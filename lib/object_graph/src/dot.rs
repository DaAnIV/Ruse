use core::fmt;
use itertools::Itertools;
use std::{collections::{HashMap, HashSet}, fmt::Write};

use crate::{
    graph_node::ObjectGraphNode, graph_walk::ObjectGraphWalker, FieldName, FieldsMap, GraphIndex,
    GraphsMap, NodeIndex, PrimitiveField,
};

static DOT_GRAPH_TYPE: &str = "digraph";
static DOT_SUBGRAPH: &str = "subgraph";
static EDGE: &str = "->";
static INDENT: &str = "    ";

#[derive(Debug, Clone)]
pub struct SubgraphConfig {
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct DotConfig {
    pub prefix: Option<String>,
    pub subgraph: Option<SubgraphConfig>,
    pub exclude_fields: HashSet<String>,
    pub print_only_content: bool,
    pub override_root_name: HashMap<NodeIndex, String>,
}

impl Default for DotConfig {
    fn default() -> Self {
        Self {
            prefix: None,
            subgraph: None,
            print_only_content: false,
            exclude_fields: Default::default(),
            override_root_name: Default::default(),
        }
    }
}

impl DotConfig {
    pub fn subgraph_config(name: &str) -> Self {
        Self {
            prefix: Some(name.to_owned()),
            subgraph: Some(SubgraphConfig {
                name: name.to_owned(),
            }),
            print_only_content: false,
            exclude_fields: Default::default(),
            override_root_name: Default::default(),
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
        }
    }
}

pub struct Dot<'a> {
    graphs_map: &'a GraphsMap,
    nodes: Vec<(GraphIndex, NodeIndex)>,
    config: DotConfig,
}

/// Escape for Graphviz
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
            '"' | '\\' => self.0.write_char('\\')?,
            // \l is for left justified linebreak
            '\n' => return self.0.write_str("\\l"),
            _ => {}
        }
        self.0.write_char(c)
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
            '.' | '(' | ')' | '[' | ']' | '{' | '}' | ' ' => return self.0.write_str("_"),
            _ => {}
        }
        self.0.write_char(c)
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

struct ObjectGraphNodeDotDisplay<'a> {
    node: &'a ObjectGraphNode,
    dot_config: &'a DotConfig,
}

impl<'a> ObjectGraphNodeDotDisplay<'a> {
    fn fields_iter(&self) -> impl std::iter::Iterator<Item = (&FieldName, &PrimitiveField)> {
        self.node
            .fields_iter()
            .filter(|(n, _v)| !self.dot_config.exclude_fields.contains(n.as_str()))
    }
}

impl<'a> fmt::Display for ObjectGraphNodeDotDisplay<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}: {{", self.node.obj_type())?;
        for (field_name, field) in self.fields_iter() {
            writeln!(f, "  {}: {}", field_name, field.value)?;
        }
        writeln!(f, "}}")?;

        Ok(())
    }
}

impl<'a> fmt::Debug for ObjectGraphNodeDotDisplay<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fields = FieldsMap::from_iter(self.fields_iter().map(|(k, v)| (k.clone(), v.clone())));
        f.debug_struct("ObjectGraphNode")
            .field("obj_type", self.node.obj_type())
            .field("fields", &fields)
            .finish()
    }
}

impl<'a> Dot<'a> {
    pub fn from_graphs_map(graphs_map: &'a GraphsMap) -> Self {
        Self::from_graphs_map_with_config(graphs_map, DotConfig::default())
    }

    pub fn from_graphs_map_with_config(graphs_map: &'a GraphsMap, config: DotConfig) -> Self {
        let nodes: Vec<(GraphIndex, NodeIndex)> = graphs_map
            .roots()
            .map(|(_, r)| (r.graph, r.node))
            .collect();
        Self::from_nodes_with_config(graphs_map, nodes, config)
    }

    pub fn from_nodes(graphs_map: &'a GraphsMap, nodes: Vec<(GraphIndex, NodeIndex)>) -> Self {
        Self::from_nodes_with_config(graphs_map, nodes, DotConfig::default())
    }

    pub fn from_nodes_with_config(
        graphs_map: &'a GraphsMap,
        nodes: Vec<(GraphIndex, NodeIndex)>,
        config: DotConfig,
    ) -> Self {
        Self {
            graphs_map,
            nodes,
            config,
        }
    }

    fn write_subgraph_header(f: &mut fmt::Formatter, name: &str) -> fmt::Result {
        writeln!(f, "{} cluster_{} {{", DOT_SUBGRAPH, EscapedName(name))?;
        writeln!(f, "{}label=\"{}\"", INDENT, name)?;
        writeln!(f, "{}graph[style=dotted];", INDENT)?;
        writeln!(f, "{}margin=20;", INDENT)?;

        Ok(())
    }

    fn write_header(f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "{} {{", DOT_GRAPH_TYPE)
    }

    pub fn write_header_with_config(f: &mut fmt::Formatter, config: &DotConfig) -> fmt::Result {
        if !config.print_only_content {
            match &config.subgraph {
                Some(subgraph) => Self::write_subgraph_header(f, &subgraph.name),
                None => Self::write_header(f),
            }?;
        }

        Ok(())
    }

    fn write_footer(f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "}}")
    }

    pub fn write_footer_with_config(f: &mut fmt::Formatter, config: &DotConfig) -> fmt::Result {
        if !config.print_only_content {
            Self::write_footer(f)?;
        }

        Ok(())
    }

    pub fn graph_header_string() -> String {
        format!("{} {{", DOT_GRAPH_TYPE)
    }

    pub fn graph_footer_string() -> String {
        format!("}}")
    }

    fn get_dot_id(id: &NodeIndex, config: &DotConfig) -> String {
        match &config.prefix {
            Some(prefix) => format!("{}{}", EscapedName(prefix), id),
            None => id.to_string(),
        }
    }

    fn node_label(&self, node: &ObjectGraphNode) -> String {
        let node_dot_display = ObjectGraphNodeDotDisplay {
            node,
            dot_config: &self.config,
        };
        Escaped(&node_dot_display).to_string()
    }

    pub fn write_node(
        f: &mut fmt::Formatter,
        node_id: &str,
        label: &str,
        root_name: Option<String>,
    ) -> fmt::Result {
        write!(f, "{} [ ", node_id)?;
        write!(f, "label = \"{}\" ", label)?;
        if let Some(root_name) = root_name {
            write!(f, ", xlabel = \"{}:\"", root_name)?;
        }
        writeln!(f, "]")?;

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

        for (cur_graph, cur_node_id, cur_node) in
            ObjectGraphWalker::from_nodes(&self.graphs_map, self.nodes.iter().copied())
        {
            write!(f, "{}", INDENT)?;
            let root_names = self.get_graph_root_names(&cur_node_id);
            Self::write_node(
                f,
                &Self::get_dot_id(&cur_node_id, &self.config),
                &format!(
                    "[{},{}] {}",
                    cur_graph.id,
                    cur_node_id,
                    self.node_label(cur_node)
                ),
                root_names,
            )?;
            edges.extend(
                cur_node
                    .pointers_iter()
                    .map(|(edge_name, neig)| (cur_node_id, edge_name.clone(), *neig)),
            );
        }

        // output all edges
        for (node_id, edge_name, neig) in edges {
            if self.config.exclude_fields.contains(edge_name.as_str()) {
                continue;
            }

            write!(
                f,
                "{}{} {} {} [ ",
                INDENT,
                Self::get_dot_id(&node_id, &self.config),
                EDGE,
                Self::get_dot_id(&neig.node, &self.config)
            )?;
            if let Some(neig_graph) = &neig.graph {
                write!(f, "label = \"{} (chain -> {})\"", edge_name, neig_graph)
            } else {
                write!(f, "label = \"{}\"", edge_name)
            }?;
            writeln!(f, "]")?;
        }

        Self::write_footer_with_config(f, &self.config)?;
        Ok(())
    }
}

impl<'a> fmt::Display for Dot<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.graph_fmt(f)
    }
}
