use core::fmt;
use std::fmt::{Display, Write};

use crate::{
    graph_node::ObjectGraphNode, graph_walk::ObjectGraphWalker, GraphIndex, GraphsMap, NodeIndex,
    ObjectGraph, RootName,
};

static DOT_GRAPH_TYPE: &str = "digraph";
static DOT_SUBGRAPH: &str = "subgraph";
static EDGE: &str = "->";
static INDENT: &str = "    ";

pub struct SubgraphConfig {
    pub name: String,
}

pub struct DotConfig {
    pub prefix: Option<String>,
    pub subgraph: Option<SubgraphConfig>,
    pub print_only_content: bool,
}

impl Default for DotConfig {
    fn default() -> Self {
        Self {
            prefix: None,
            subgraph: None,
            print_only_content: false,
        }
    }
}

pub struct Dot<'a> {
    graphs_map: &'a GraphsMap,
    graph_ids: Vec<GraphIndex>,
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

struct Escaped<T>(T);

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

/// Format data using a specific format function
struct FnFmt<'a, T, F>(&'a T, F);

impl<'a, T, F> fmt::Display for FnFmt<'a, T, F>
where
    F: Fn(&'a T, &mut fmt::Formatter<'_>) -> fmt::Result,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.1(self.0, f)
    }
}

impl<'a> Dot<'a> {
    pub fn from_graph(graphs_map: &'a GraphsMap, graph: GraphIndex) -> Self {
        Self::from_graph_with_config(graphs_map, graph, DotConfig::default())
    }

    pub fn from_graph_with_config(
        graphs_map: &'a GraphsMap,
        graph: GraphIndex,
        config: DotConfig,
    ) -> Self {
        Self::from_graphs_with_config(graphs_map, vec![graph], config)
    }

    pub fn from_graphs_map(graphs_map: &'a GraphsMap) -> Self {
        Self::from_graphs_map_with_config(graphs_map, DotConfig::default())
    }

    pub fn from_graphs_map_with_config(graphs_map: &'a GraphsMap, config: DotConfig) -> Self {
        Self::from_graphs_with_config(
            graphs_map,
            Vec::from_iter(graphs_map.graphs().map(|x| *x)),
            config,
        )
    }

    pub fn from_graphs(graphs_map: &'a GraphsMap, graphs: Vec<GraphIndex>) -> Self {
        Self::from_graphs_with_config(graphs_map, graphs, DotConfig::default())
    }

    pub fn from_graphs_with_config(
        graphs_map: &'a GraphsMap,
        graphs: Vec<GraphIndex>,
        config: DotConfig,
    ) -> Self {
        Self {
            graphs_map,
            graph_ids: graphs,
            config,
        }
    }

    fn write_subgraph_header(f: &mut fmt::Formatter, name: &str) -> fmt::Result {
        writeln!(f, "{} cluster_{} {{", DOT_SUBGRAPH, name)?;
        writeln!(f, "{}label=\"{}\"", INDENT, name)?;
        writeln!(f, "{}graph[style=dotted];", INDENT)?;
        writeln!(f, "{}margin=20;", INDENT)?;

        Ok(())
    }

    fn write_header(f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "{} {{", DOT_GRAPH_TYPE)
    }

    fn write_footer(f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "}}")
    }

    pub fn print_header() {
        println!("{} {{", DOT_GRAPH_TYPE);
    }

    pub fn print_footer() {
        println!("}}");
    }

    fn is_root(&self, graph: &ObjectGraph, id: &NodeIndex) -> Option<RootName> {
        for (root_name, root_id) in &graph.roots {
            if id == root_id {
                return Some(root_name.clone());
            }
        }
        None
    }

    fn get_dot_id(&self, id: &NodeIndex) -> String {
        match &self.config.prefix {
            Some(prefix) => format!("{}{}", prefix, id),
            None => id.to_string(),
        }
    }

    fn graph_fmt<NF>(&self, f: &mut fmt::Formatter, node_fmt: NF) -> fmt::Result
    where
        NF: Fn(&ObjectGraphNode, &mut fmt::Formatter) -> fmt::Result,
    {
        if !self.config.print_only_content {
            match &self.config.subgraph {
                Some(subgraph) => Self::write_subgraph_header(f, &subgraph.name),
                None => Self::write_header(f),
            }?;
        }

        let mut edges = Vec::new();

        for (cur_graph, cur_node_id, cur_node) in
            ObjectGraphWalker::from_graphs(&self.graphs_map, self.graph_ids.clone())
        {
            write!(f, "{}{} [ ", INDENT, self.get_dot_id(&cur_node_id))?;
            write!(f, "label = \"")?;
            Escaped(FnFmt(cur_node, &node_fmt)).fmt(f)?;
            write!(f, "\"")?;
            if let Some(root_name) = self.is_root(cur_graph, &cur_node_id) {
                write!(f, ", xlabel = \"{}:\"", root_name)?;
            }
            writeln!(f, "]")?;
            edges.extend(
                cur_node
                    .pointers_iter()
                    .map(|(edge_name, neig)| (cur_node_id, edge_name.clone(), *neig)),
            );
        }

        // output all edges
        for (node_id, edge_name, neig) in edges {
            write!(
                f,
                "{}{} {} {} [ ",
                INDENT,
                self.get_dot_id(&node_id),
                EDGE,
                self.get_dot_id(neig.index())
            )?;
            match neig {
                crate::graph_node::EdgeEndPoint::Internal(_) => {
                    write!(f, "label = \"{}\"", edge_name)
                }
                crate::graph_node::EdgeEndPoint::Chain(next_graph_id, _) => {
                    write!(f, "label = \"{} (chain -> {})\"", edge_name, next_graph_id)
                }
            }?;
            writeln!(f, "]")?;
        }

        if !self.config.print_only_content {
            Self::write_footer(f)?;
        }
        Ok(())
    }
}

impl<'a> fmt::Display for Dot<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.graph_fmt(f, fmt::Display::fmt)
    }
}

impl<'a> fmt::Debug for Dot<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.graph_fmt(f, fmt::Debug::fmt)
    }
}
