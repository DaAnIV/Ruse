use core::fmt;
use itertools::Itertools;
use std::{
    collections::{HashMap, HashSet},
    fmt::Write,
};

use crate::{
    graph_node::ObjectGraphNode, graph_walk::ObjectGraphWalker, FieldName, FieldsMap, GraphIndex,
    GraphsMap, NodeIndex, PrimitiveField, ValueType,
};

static MERMAID_GRAPH_TYPE: &str = "flowchart LR";
static MERMAID_SUBGRAPH: &str = "subgraph";
static INDENT: &str = "    ";

#[derive(Debug, Clone)]
pub struct SubgraphConfig {
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct MermaidConfig {
    pub prefix: Option<String>,
    pub subgraph: Option<SubgraphConfig>,
    pub exclude_fields: HashSet<String>,
    pub print_only_content: bool,
    pub override_root_name: HashMap<NodeIndex, String>,
    pub write_chain_on_edge: bool,
}

impl Default for MermaidConfig {
    fn default() -> Self {
        Self {
            prefix: None,
            subgraph: None,
            print_only_content: false,
            exclude_fields: Default::default(),
            override_root_name: Default::default(),
            write_chain_on_edge: false,
        }
    }
}

impl MermaidConfig {
    pub fn subgraph_config(name: &str) -> Self {
        Self {
            prefix: Some(name.to_owned()),
            subgraph: Some(SubgraphConfig {
                name: name.to_owned(),
            }),
            print_only_content: false,
            exclude_fields: Default::default(),
            override_root_name: Default::default(),
            write_chain_on_edge: false,
        }
    }

    pub fn subgraph_config_with_prefix(name: &str, prefix: &str) -> Self {
        Self {
            prefix: Some(prefix.to_owned()),
            subgraph: Some(SubgraphConfig {
                name: name.to_owned(),
            }),
            print_only_content: false,
            exclude_fields: Default::default(),
            override_root_name: Default::default(),
            write_chain_on_edge: false,
        }
    }
}

pub struct Mermaid<'a> {
    graphs_map: &'a GraphsMap,
    nodes: Vec<(GraphIndex, NodeIndex)>,
    config: MermaidConfig,
}

/// Escape for Mermaid
struct Escaper<W>(W);

impl<W> fmt::Write for Escaper<W>
where
    W: fmt::Write,
{
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            self.write_char(c)?;
        }
        Ok(())
    }

    fn write_char(&mut self, c: char) -> fmt::Result {
        match c {
            // Mermaid uses different escaping rules
            '"' => self.0.write_str("&quot;")?,
            '<' => self.0.write_str("&lt;")?,
            '>' => self.0.write_str("&gt;")?,
            '&' => self.0.write_str("&amp;")?,
            '\n' => return self.0.write_str("<br/>"),
            _ => return self.0.write_char(c),
        }
        Ok(())
    }
}
struct NameEscaper<W>(W);

impl<W> fmt::Write for NameEscaper<W>
where
    W: fmt::Write,
{
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            self.write_char(c)?;
        }
        Ok(())
    }

    fn write_char(&mut self, c: char) -> fmt::Result {
        match c {
            // Mermaid class names can contain more characters, but we'll be conservative
            '.' | '(' | ')' | '[' | ']' | '{' | '}' | ' ' | '-' | '+' => self.0.write_str("_"),
            _ => self.0.write_char(c),
        }
    }
}

pub struct Escaped<T>(pub T);

impl<T> fmt::Display for Escaped<T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if f.alternate() {
            writeln!(&mut Escaper(f), "{:#}", &self.0)
        } else {
            write!(&mut Escaper(f), "{}", &self.0)
        }
    }
}
pub struct EscapedName<T>(pub T);

impl<T> fmt::Display for EscapedName<T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if f.alternate() {
            writeln!(&mut NameEscaper(f), "{:#}", &self.0)
        } else {
            write!(&mut NameEscaper(f), "{}", &self.0)
        }
    }
}

struct ObjectGraphNodeMermaidDisplay<'a> {
    node: &'a ObjectGraphNode,
    mermaid_config: &'a MermaidConfig,
}

impl<'a> ObjectGraphNodeMermaidDisplay<'a> {
    fn fields_iter(&self) -> impl std::iter::Iterator<Item = (&FieldName, &PrimitiveField)> {
        self.node
            .fields_iter()
            .filter(|(n, _v)| !self.mermaid_config.exclude_fields.contains(n.as_str()))
    }
}

impl<'a> fmt::Display for ObjectGraphNodeMermaidDisplay<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Flowchart format: compact field representation
        let fields: Vec<String> = self
            .fields_iter()
            .map(|(field_name, field)| format!("{}:{}", field_name, field.value))
            .collect();

        if !fields.is_empty() {
            write!(f, "{}", fields.join(", "))?;
        }
        Ok(())
    }
}

impl<'a> fmt::Debug for ObjectGraphNodeMermaidDisplay<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fields = FieldsMap::from_iter(self.fields_iter().map(|(k, v)| (k.clone(), v.clone())));
        f.debug_struct("ObjectGraphNode")
            .field("obj_type", self.node.obj_type())
            .field("fields", &fields)
            .finish()
    }
}

impl<'a> Mermaid<'a> {
    pub fn from_graphs_map(graphs_map: &'a GraphsMap) -> Self {
        Self::from_graphs_map_with_config(graphs_map, MermaidConfig::default())
    }

    pub fn from_graphs_map_with_config(graphs_map: &'a GraphsMap, config: MermaidConfig) -> Self {
        let nodes: Vec<(GraphIndex, NodeIndex)> =
            graphs_map.roots().map(|(_, r)| (r.graph, r.node)).collect();
        Self::from_nodes_with_config(graphs_map, nodes, config)
    }

    pub fn from_nodes(graphs_map: &'a GraphsMap, nodes: Vec<(GraphIndex, NodeIndex)>) -> Self {
        Self::from_nodes_with_config(graphs_map, nodes, MermaidConfig::default())
    }

    pub fn from_nodes_with_config(
        graphs_map: &'a GraphsMap,
        nodes: Vec<(GraphIndex, NodeIndex)>,
        config: MermaidConfig,
    ) -> Self {
        Self {
            graphs_map,
            nodes,
            config,
        }
    }

    fn write_subgraph_header(f: &mut fmt::Formatter, name: &str) -> fmt::Result {
        writeln!(f, "{} {}", MERMAID_SUBGRAPH, name)?;

        Ok(())
    }

    fn write_subgraph_footer(f: &mut fmt::Formatter, _name: &str) -> fmt::Result {
        writeln!(f, "end")?;
        Ok(())
    }

    fn write_header(f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "{}", MERMAID_GRAPH_TYPE)
    }

    pub fn write_header_with_config(f: &mut fmt::Formatter, config: &MermaidConfig) -> fmt::Result {
        if !config.print_only_content {
            match &config.subgraph {
                Some(subgraph) => Self::write_subgraph_header(f, &subgraph.name),
                None => Self::write_header(f),
            }?;
        }

        Ok(())
    }

    fn write_footer(_f: &mut fmt::Formatter) -> fmt::Result {
        Ok(())
    }

    pub fn write_footer_with_config(f: &mut fmt::Formatter, config: &MermaidConfig) -> fmt::Result {
        if !config.print_only_content {
            match &config.subgraph {
                Some(subgraph) => Self::write_subgraph_footer(f, &subgraph.name),
                None => Self::write_footer(f),
            }?;
        }

        Ok(())
    }

    pub fn graph_header_string() -> String {
        format!("{}", MERMAID_GRAPH_TYPE)
    }

    pub fn graph_footer_string() -> String {
        String::new() // Mermaid doesn't need footer
    }

    fn get_mermaid_id(id: &NodeIndex, config: &MermaidConfig) -> String {
        match &config.prefix {
            Some(prefix) => format!("{}{}", EscapedName(prefix), id),
            None => format!("{}", EscapedName(id)),
        }
    }

    fn node_content(&self, node: &ObjectGraphNode) -> String {
        let node_mermaid_display = ObjectGraphNodeMermaidDisplay {
            node,
            mermaid_config: &self.config,
        };
        Escaped(&node_mermaid_display).to_string()
    }

    pub fn write_css_classes(f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(
            f,
            "{}classDef rootNode fill:#e1f5fe,stroke:#01579b,stroke-width:3px,color:#000",
            INDENT
        )?;
        writeln!(
            f,
            "{}classDef regularNode fill:#f3e5f5,stroke:#4a148c,stroke-width:2px,color:#000",
            INDENT
        )?;
        Ok(())
    }

    pub fn apply_node_class(f: &mut fmt::Formatter, node_id: &str, is_root: bool) -> fmt::Result {
        let class_name = if is_root { "rootNode" } else { "regularNode" };
        writeln!(f, "{}class {} {}", INDENT, node_id, class_name)?;
        Ok(())
    }

    pub fn write_node(
        f: &mut fmt::Formatter,
        node_id: &str,
        node_coords: Option<(GraphIndex, NodeIndex)>,
        val_type: &ValueType,
        content: &str,
        root_name: Option<String>,
    ) -> fmt::Result {
        // Mermaid flowchart node format with Markdown: NodeId["Markdown content"]
        let mut label = String::new();

        // Root names separated at the top
        if let Some(root_name) = root_name {
            label.push_str(&root_name);
            label.push_str("\\<hr>"); // Separator line
        }

        // Small coordinates at top left
        // Bold object type as first main line
        if let Some((graph_index, node_index)) = node_coords {
            label.push_str(&format!(
                "\\<p style='text-align:left;'><sup>[{},{}]</sup></p>",
                graph_index, node_index
            ));
        }
        label.push_str(&format!("**{}**\n", val_type));
        label.push_str("<hr style=\"border-top: dotted;\">");

        // Content after object type
        if !content.trim().is_empty() {
            label.push_str(&content);
            label.push_str("\n");
        }

        writeln!(f, "{}[\"`{}`\"]", node_id, label)?;

        Ok(())
    }

    fn get_graph_root_names(&self, node_id: &NodeIndex) -> Option<String> {
        if let Some(override_root_name) = self.config.override_root_name.get(node_id) {
            Some(override_root_name.clone())
        } else {
            self.graphs_map
                .node_root_names(&node_id)
                .map(|mut names| names.join(", "))
        }
    }

    fn graph_fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Self::write_header_with_config(f, &self.config)?;

        let mut edges = Vec::new();
        let mut node_styles = Vec::new();

        for (cur_graph, cur_node_id, cur_node) in
            ObjectGraphWalker::from_nodes(&self.graphs_map, self.nodes.iter().copied())
        {
            write!(f, "{}", INDENT)?;
            let root_names = self.get_graph_root_names(&cur_node_id);
            let node_id = Self::get_mermaid_id(&cur_node_id, &self.config);
            let is_root = root_names.is_some();

            Self::write_node(
                f,
                &node_id,
                Some((cur_graph.id, cur_node_id)),
                &cur_node.val_type(),
                &self.node_content(cur_node),
                root_names,
            )?;

            // Collect styling information
            node_styles.push((node_id, is_root));

            edges.extend(
                cur_node
                    .pointers_iter()
                    .map(|(edge_name, neig)| (cur_node_id, edge_name.clone(), *neig)),
            );
        }

        // output all relationships
        for (node_id, edge_name, neig) in edges {
            if self.config.exclude_fields.contains(edge_name.as_str()) {
                continue;
            }

            // Mermaid flowchart relationship format: NodeA --|label|--> NodeB
            if self.config.write_chain_on_edge && neig.graph.is_some() {
                writeln!(
                    f,
                    "{}{} --|{} (chain -> {})|--> {}",
                    INDENT,
                    Self::get_mermaid_id(&node_id, &self.config),
                    edge_name,
                    neig.graph.unwrap(),
                    Self::get_mermaid_id(&neig.node, &self.config)
                )?;
            } else {
                writeln!(
                    f,
                    "{}{} --|{}|--> {}",
                    INDENT,
                    Self::get_mermaid_id(&node_id, &self.config),
                    edge_name,
                    Self::get_mermaid_id(&neig.node, &self.config)
                )?;
            }
        }

        // Add CSS class definitions
        writeln!(f)?; // Empty line for readability
        Self::write_css_classes(f)?;

        // Apply styling to nodes
        writeln!(f)?; // Empty line for readability
        for (node_id, is_root) in node_styles {
            Self::apply_node_class(f, &node_id, is_root)?;
        }

        Self::write_footer_with_config(f, &self.config)?;
        Ok(())
    }
}

impl<'a> fmt::Display for Mermaid<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.graph_fmt(f)
    }
}
