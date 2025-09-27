use std::{
    collections::{HashMap, HashSet},
    fmt,
    ops::{AddAssign, SubAssign},
    sync::Arc,
};

use crate::{
    graph_map_value::{GraphMapValue, GraphMapWrap},
    graph_node::*,
    FieldName, GraphIndex, GraphsMap, NodeIndex, ObjectType, PrimitiveValue,
};

type NodesMap = HashMap<NodeIndex, Arc<ObjectGraphNode>>;

#[derive(Clone)]
pub struct ObjectGraph {
    pub id: GraphIndex,
    is_static: bool,
    pub(crate) nodes: NodesMap,
    pub(crate) chained_graphs: HashMap<GraphIndex, usize>,
}

impl ObjectGraph {
    pub(crate) fn new(id: GraphIndex) -> Self {
        Self {
            id,
            nodes: Default::default(),
            chained_graphs: Default::default(),
            is_static: false,
        }
    }

    pub(crate) fn new_static(id: GraphIndex) -> Self {
        Self {
            id,
            nodes: Default::default(),
            chained_graphs: Default::default(),
            is_static: true,
        }
    }

    pub fn is_static(&self) -> bool {
        self.is_static
    }

    pub fn get_node(&self, id: &NodeIndex) -> Option<&Arc<ObjectGraphNode>> {
        self.nodes.get(id)
    }

    pub(crate) fn get_mut_node(&mut self, id: &NodeIndex) -> Option<&mut ObjectGraphNode> {
        Some(Arc::make_mut(self.nodes.get_mut(id)?))
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn contains_node(&self, id: &NodeIndex) -> bool {
        self.nodes.contains_key(id)
    }

    pub fn contains_internal_edge(&self, a: &NodeIndex, b: &NodeIndex) -> bool {
        for (_, neig) in self.neighbors(a) {
            if neig.graph.is_none() && neig.node == *b {
                return true;
            }
        }
        false
    }

    pub fn obj_type(&self, node: &NodeIndex) -> Option<&ObjectType> {
        Some(&self.get_node(node)?.obj_type())
    }

    pub fn get_primitive_field(
        &self,
        node: &NodeIndex,
        field: &FieldName,
    ) -> Option<&PrimitiveField> {
        self.get_node(node)?.get_field(field).clone()
    }

    pub fn primitive_fields_count(&self, id: &NodeIndex) -> usize {
        let node = self.get_node(id).unwrap();
        node.fields_len()
    }

    pub fn primitive_fields(
        &self,
        id: &NodeIndex,
    ) -> impl std::iter::Iterator<Item = (&FieldName, &PrimitiveField)> {
        let node = self.get_node(id).unwrap();
        node.fields_iter()
    }

    pub fn get_neighbor(&self, node: &NodeIndex, field: &FieldName) -> Option<&EdgeEndPoint> {
        self.get_node(node)?.pointers_get(field)
    }

    pub fn get_neighbor_id(&self, node: &NodeIndex, field: &FieldName) -> Option<(GraphIndex, NodeIndex)> {
        self.get_neighbor(node, field).map(|e| (e.graph.unwrap_or(self.id), e.node))
    }

    pub fn neighbors_count(&self, id: &NodeIndex) -> usize {
        let node = self.get_node(id).unwrap();
        node.pointers_len()
    }

    pub fn neighbors(
        &self,
        id: &NodeIndex,
    ) -> impl std::iter::Iterator<Item = (&FieldName, &EdgeEndPoint)> {
        let node = self.get_node(id).unwrap();
        node.pointers_iter()
    }

    pub fn node_ids(&self) -> impl std::iter::Iterator<Item = &NodeIndex> {
        self.nodes.keys()
    }

    pub fn nodes(&self) -> impl std::iter::Iterator<Item = (&NodeIndex, &Arc<ObjectGraphNode>)> {
        self.nodes.iter()
    }

    pub fn chained_graphs(&self) -> impl std::iter::Iterator<Item = &GraphIndex> {
        self.chained_graphs.keys()
    }

    pub(crate) fn node_attributes(&self, node: NodeIndex) -> Option<Attributes> {
        self.get_node(&node).map(|x| x.attributes.clone())
    }
}

impl ObjectGraph {
    pub(crate) fn add_node(
        &mut self,
        id: NodeIndex,
        node: ObjectGraphNode,
    ) -> &mut ObjectGraphNode {
        assert!(!self.nodes.contains_key(&id), "Node {} already exists", id);

        self.nodes.insert(id, node.into());
        unsafe { Arc::get_mut(self.nodes.get_mut(&id).unwrap_unchecked()).unwrap_unchecked() }
    }

    pub(crate) fn set_internal_edge(&mut self, a: NodeIndex, b: NodeIndex, name: FieldName) {
        assert!(self.nodes.contains_key(&b));
        let node_a = self.get_mut_node(&a).unwrap();
        node_a.insert_internal_edge(name.clone(), b);
    }

    fn inc_chained_graph_ref(&mut self, graph: GraphIndex) {
        if let Some(ref_count) = self.chained_graphs.get_mut(&graph) {
            ref_count.add_assign(1);
        } else {
            self.chained_graphs.insert(graph, 1);
        }
    }

    fn dec_chained_graph_ref(&mut self, graph: GraphIndex) {
        debug_assert!(self.chained_graphs.contains_key(&graph));
        let delete: bool;
        {
            let ref_count = self.chained_graphs.get_mut(&graph).unwrap();
            ref_count.sub_assign(1);
            delete = *ref_count == 0;
        }
        if delete {
            self.chained_graphs.remove(&graph);
        }
    }

    pub(crate) fn set_chain_edge(
        &mut self,
        a: NodeIndex,
        b_graph: GraphIndex,
        b: NodeIndex,
        name: FieldName,
    ) {
        debug_assert!(b_graph != self.id);

        let node_a = self.get_mut_node(&a).unwrap();
        node_a.insert_chain_edge(name.clone(), b_graph, b);
        self.inc_chained_graph_ref(b_graph);
    }

    pub(crate) fn remove_edge(&mut self, a: NodeIndex, name: &FieldName) -> Option<EdgeEndPoint> {
        let node_a = self.get_mut_node(&a).unwrap();
        let edge = node_a.pointers_remove(name)?;
        if let Some(graph) = &edge.graph {
            self.dec_chained_graph_ref(*graph);
        }

        Some(edge)
    }

    pub(crate) fn set_primitive_field(
        &mut self,
        node: NodeIndex,
        field: FieldName,
        value: PrimitiveValue,
    ) {
        self.set_primitive_field_with_attributes(node, field, value, Attributes::default());
    }

    pub(crate) fn set_primitive_field_with_attributes(
        &mut self,
        node: NodeIndex,
        field: FieldName,
        value: PrimitiveValue,
        attributes: Attributes,
    ) {
        if let Some(mut_node) = self.get_mut_node(&node) {
            mut_node.insert_primitive_field_with_attributes(field, value, attributes);
        } else {
            assert!(false, "Graph {} doesn't contain node {}", &self.id, node)
        }
    }

    pub(crate) fn delete_primitive_field(
        &mut self,
        node: NodeIndex,
        field: &FieldName,
    ) -> Option<PrimitiveField> {
        self.get_mut_node(&node)
            .unwrap()
            .remove_primitive_field(field)
    }
}

impl ObjectGraph {
    pub fn get_node_from_edge_end_point<'a>(
        &'a self,
        end_point: &EdgeEndPoint,
        graphs_map: &'a GraphsMap,
    ) -> &'a ObjectGraphNode {
        if let Some(graph) = end_point.graph {
            &graphs_map[graph].nodes[&end_point.node]
        } else {
            &self.nodes[&end_point.node]
        }
    }
}

impl ObjectGraph {
    pub fn fmt_node(
        &self,
        f: &mut fmt::Formatter<'_>,
        graphs_map: &GraphsMap,
        node: &NodeIndex,
    ) -> fmt::Result {
        let mut seen = HashSet::new();
        self.fmt_node_with_indentation(f, graphs_map, node, 0, &mut seen)
    }

    fn fmt_node_with_indentation(
        &self,
        f: &mut fmt::Formatter<'_>,
        graphs_map: &GraphsMap,
        node: &NodeIndex,
        indentation: usize,
        seen: &mut HashSet<NodeIndex>,
    ) -> fmt::Result {
        self.fmt_node_with_indentation_and_name(f, graphs_map, node, "", indentation, seen)
    }

    pub(crate) fn fmt_node_with_indentation_and_name(
        &self,
        f: &mut fmt::Formatter<'_>,
        graphs_map: &GraphsMap,
        node: &NodeIndex,
        name: &str,
        indentation: usize,
        seen: &mut HashSet<NodeIndex>,
    ) -> fmt::Result {
        let indent_str = Self::indent_str(indentation);

        let weight = self.get_node(node).unwrap();

        write!(
            f,
            "{}[{},{}] {} ",
            indent_str,
            self.id,
            node,
            weight.obj_type()
        )?;
        if !name.is_empty() {
            write!(f, "{} ", name)?;
        }
        if !seen.insert(*node) {
            return Ok(());
        }
        writeln!(f, "{{")?;
        for (field_name, field) in weight.fields_iter() {
            writeln!(f, " {}{}: {},", indent_str, field_name, field.value)?;
        }
        for (field_name, val) in weight.pointers_iter() {
            let graph = &graphs_map[val.graph.unwrap_or(self.id)];
            graph.fmt_node_with_indentation_and_name(
                f,
                graphs_map,
                &val.node,
                field_name.as_str(),
                indentation + 1,
                seen,
            )?;
        }

        writeln!(f, "{}}}", indent_str)
    }

    fn indent_str(indentation: usize) -> String {
        let mut indent_str = String::with_capacity(indentation);
        for _ in 0..indentation {
            indent_str.push(' ');
        }
        indent_str
    }
}

impl fmt::Debug for ObjectGraph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObjectGraph")
            .field("id", &self.id)
            .field("is_static", &self.is_static)
            .field("nodes", &self.nodes)
            .finish()
    }
}

impl GraphMapWrap<Self> for ObjectGraph {
    fn wrap<'a>(&'a self, graphs_map: &'a GraphsMap) -> GraphMapValue<'a, Self> {
        GraphMapValue::from(&self, graphs_map)
    }
}
