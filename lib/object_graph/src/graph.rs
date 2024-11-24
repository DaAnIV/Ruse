use std::{
    collections::{BTreeMap, HashMap},
    fmt,
    hash::{Hash, Hasher},
    ops::{self, AddAssign, SubAssign},
    sync::Arc,
};

use crate::{
    graph_equality, graph_map_value::{GraphMapEq, GraphMapHash, GraphMapValue, GraphMapWrap}, graph_node::*, graph_walk::ObjectGraphWalker, node_index::{DefaultIx, NodeIndex}, scached, value::{ObjectValue, Value, ValueType}, Cache, CachedString, FieldsMap, PrimitiveValue
};
pub type GraphIndex = DefaultIx;

#[derive(Clone, Default, Debug)]
pub struct GraphsMap(HashMap<GraphIndex, Arc<ObjectGraph>>);

impl GraphsMap {
    pub fn insert_graph(&mut self, graph: Arc<ObjectGraph>) -> Option<Arc<ObjectGraph>> {
        self.0.insert(graph.id, graph)
    }

    pub fn get(&self, index: GraphIndex) -> &Arc<ObjectGraph> {
        self.0.get(&index).unwrap()
    }

    pub fn contains(&self, index: GraphIndex) -> bool {
        self.0.contains_key(&index)
    }

    pub fn graphs(&self) -> impl std::iter::Iterator<Item = &GraphIndex> {
        self.0.keys()
    }
    
    pub fn extend(&mut self, other: &GraphsMap) {
        for (key, value) in &other.0 {
            self.0.entry(*key).or_insert(value.clone());
        }
    }

    pub fn keys(&self) -> impl std::iter::Iterator<Item = &usize> {
        self.0.keys()
    }
}

impl ops::Index<&GraphIndex> for GraphsMap {
    type Output = Arc<ObjectGraph>;

    fn index(&self, index: &GraphIndex) -> &Self::Output {
        self.0.get(index).expect(&format!("No entry found for key {}", index))
    }
}

impl ops::Index<GraphIndex> for GraphsMap {
    type Output = Arc<ObjectGraph>;

    fn index(&self, index: GraphIndex) -> &Self::Output {
        self.0.get(&index).expect(&format!("No entry found for key {}", index))
    }
}

type NodesMap = HashMap<NodeIndex, Arc<ObjectGraphNode>>;
pub type RootName = CachedString;
type RootsMap = BTreeMap<RootName, NodeIndex>;

#[derive(Clone, Debug)]
pub struct ObjectGraph {
    pub id: GraphIndex,
    pub(crate) nodes: NodesMap,
    pub(crate) roots: RootsMap,
    pub(crate) chained_graphs: HashMap<GraphIndex, usize>,
}

impl ObjectGraph {
    pub fn new(id: GraphIndex) -> Self {
        Self {
            id,
            nodes: Default::default(),
            roots: Default::default(),
            chained_graphs: Default::default(),
        }
    }

    pub fn add_node(
        &mut self,
        id: NodeIndex,
        obj_type: ObjectType,
        fields: FieldsMap,
    ) -> &mut ObjectGraphNode {
        assert!(!self.nodes.contains_key(&id));
        let node = ObjectGraphNode {
            id,
            obj_type,
            fields,
            pointers: Default::default(),
        };

        // self.zero_hash();
        self.nodes.insert(id, node.into());
        Arc::get_mut(self.nodes.get_mut(&id).unwrap()).unwrap()
    }

    fn add_node_with_pointers(
        &mut self,
        id: NodeIndex,
        obj_type: ObjectType,
        fields: FieldsMap,
        pointers: PointersMap,
    ) -> &mut ObjectGraphNode {
        assert!(!self.nodes.contains_key(&id));
        let node = ObjectGraphNode {
            id,
            obj_type,
            fields,
            pointers: pointers,
        };

        self.nodes.insert(id, node.into());
        Arc::get_mut(self.nodes.get_mut(&id).unwrap()).unwrap()
    }

    pub fn get_node(&self, id: &NodeIndex) -> Option<&Arc<ObjectGraphNode>> {
        self.nodes.get(id)
    }

    fn get_mut_node(&mut self, id: &NodeIndex) -> Option<&mut ObjectGraphNode> {
        let old_node = self.nodes.get(id)?;
        let node_clone = (**old_node).clone();
        self.nodes.insert(node_clone.id, node_clone.into());

        Arc::get_mut(self.nodes.get_mut(id).unwrap())
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn contains_node(&self, id: &NodeIndex) -> bool {
        self.nodes.contains_key(id)
    }

    pub fn set_edge(&mut self, a: &NodeIndex, b: NodeIndex, name: FieldName) {
        assert!(self.nodes.contains_key(&b));
        let node_a = self.get_mut_node(a).unwrap();
        node_a
            .pointers
            .insert(name.clone(), EdgeEndPoint::Internal(b));
    }

    pub fn inc_chained_graph_ref(&mut self, graph: GraphIndex) {
        if let Some(ref_count) = self.chained_graphs.get_mut(&graph) {
            ref_count.add_assign(1);
        } else {
            self.chained_graphs.insert(graph, 1);
        }
    }

    pub fn dec_chained_graph_ref(&mut self, graph: GraphIndex) {
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

    pub fn set_chain_edge(
        &mut self,
        a: &NodeIndex,
        b_graph: GraphIndex,
        b: NodeIndex,
        name: FieldName,
    ) {
        debug_assert!(b_graph != self.id);

        let node_a = self.get_mut_node(a).unwrap();
        node_a.insert_chain_edge(name.clone(), b_graph, b);
        self.inc_chained_graph_ref(b_graph);
    }

    pub fn remove_edge(&mut self, a: &NodeIndex, name: FieldName) {
        let node_a = self.get_mut_node(a).unwrap();
        if let Some(EdgeEndPoint::Chain(graph, _)) = node_a.pointers.remove(&name) {
            self.dec_chained_graph_ref(graph);
        }
    }

    pub fn contains_internal_edge(&mut self, a: &NodeIndex, b: &NodeIndex) -> bool {
        for (_, neig) in self.neighbors(a) {
            if let EdgeEndPoint::Internal(neig_id) = neig {
                if neig_id == b {
                    return true;
                }
            }
        }
        false
    }

    pub fn obj_type(&self, node: &NodeIndex) -> Option<&ObjectType> {
        Some(&self.get_node(node)?.obj_type)
    }

    pub fn get_field(&self, node: &NodeIndex, field: &FieldName) -> Option<&PrimitiveValue> {
        self.get_node(node)?.fields.get(field).clone()
    }

    pub fn set_field(&mut self, node: &NodeIndex, field: FieldName, value: PrimitiveValue) {
        self.get_mut_node(node).unwrap().fields.insert(field, value);
    }

    pub fn delete_field(&mut self, node: &NodeIndex, field: &FieldName) -> Option<PrimitiveValue> {
        self.get_mut_node(node).unwrap().fields.remove(field)
    }

    pub fn fields_count(&self, id: &NodeIndex) -> usize {
        let node = self.get_node(id).unwrap();
        node.fields.len()
    }

    pub fn fields(
        &self,
        id: &NodeIndex,
    ) -> impl std::iter::Iterator<Item = (&FieldName, &PrimitiveValue)> {
        let node = self.get_node(id).unwrap();
        node.fields.iter()
    }

    pub fn get_neighbor(&self, node: &NodeIndex, field: &FieldName) -> Option<&EdgeEndPoint> {
        self.get_node(node)?.pointers.get(field)
    }

    pub fn neighbors_count(&self, id: &NodeIndex) -> usize {
        let node = self.get_node(id).unwrap();
        node.pointers.len()
    }

    pub fn neighbors(
        &self,
        id: &NodeIndex,
    ) -> impl std::iter::Iterator<Item = (&FieldName, &EdgeEndPoint)> {
        let node = self.get_node(id).unwrap();
        node.pointers.iter()
    }

    pub fn get_root(&self, name: &RootName) -> Option<&NodeIndex> {
        self.roots.get(name)
    }

    pub fn set_as_root(&mut self, name: RootName, id: NodeIndex) {
        assert!(self.nodes.contains_key(&id));
        self.roots.insert(name, id);
    }

    pub fn root_names(&self) -> impl std::iter::Iterator<Item = &RootName> {
        self.roots.keys()
    }

    pub fn node_ids(&self) -> impl std::iter::Iterator<Item = &NodeIndex> {
        self.nodes.keys()
    }

    pub fn nodes(&self) -> impl std::iter::Iterator<Item = &Arc<ObjectGraphNode>> {
        self.nodes.values()
    }

    pub fn chained_graphs(&self) -> impl std::iter::Iterator<Item = &GraphIndex> {
        self.chained_graphs.keys()
    }
}

impl ObjectGraph {
    pub fn get_node_from_edge_end_point<'a>(
        &'a self,
        end_point: &EdgeEndPoint,
        graphs_map: &'a GraphsMap,
    ) -> &'a ObjectGraphNode {
        match end_point {
            EdgeEndPoint::Internal(node_index) => &self.nodes[node_index],
            EdgeEndPoint::Chain(graph_index, node_index) => {
                &graphs_map[graph_index].nodes[node_index]
            }
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
        self.fmt_node_with_indentation(f, graphs_map, node, 0)
    }

    pub fn fmt_graph(&self, f: &mut fmt::Formatter<'_>, graphs_map: &GraphsMap) -> fmt::Result {
        writeln!(f, "{{")?;
        for (name, node) in self.roots.iter() {
            self.fmt_node_with_indentation_and_name(f, graphs_map, node, name, 1)?;
        }
        writeln!(f, "}}")?;

        Ok(())
    }

    fn fmt_node_with_indentation(
        &self,
        f: &mut fmt::Formatter<'_>,
        graphs_map: &GraphsMap,
        node: &NodeIndex,
        indentation: usize,
    ) -> fmt::Result {
        self.fmt_node_with_indentation_and_name(f, graphs_map, node, "", indentation)
    }

    fn fmt_node_with_indentation_and_name(
        &self,
        f: &mut fmt::Formatter<'_>,
        graphs_map: &GraphsMap,
        node: &NodeIndex,
        name: &str,
        indentation: usize,
    ) -> fmt::Result {
        let indent_str = Self::indent_str(indentation);

        let weight = self.get_node(node).unwrap();

        write!(f, "{}{} ", indent_str, weight.obj_type)?;
        if !name.is_empty() {
            write!(f, "{} ", name)?;
        }
        writeln!(f, "{{")?;
        for (field_name, val) in weight.fields.iter() {
            writeln!(f, " {}{}: {},", indent_str, field_name, val)?;
        }
        for (field_name, val) in weight.pointers.iter() {
            match val {
                EdgeEndPoint::Internal(index) => {
                    self.fmt_node_with_indentation_and_name(
                        f,
                        graphs_map,
                        index,
                        field_name.as_str(),
                        indentation + 1,
                    )?;
                }
                EdgeEndPoint::Chain(graph_id, index) => {
                    let graph = &graphs_map[graph_id];
                    graph.fmt_node_with_indentation_and_name(
                        f,
                        graphs_map,
                        index,
                        field_name.as_str(),
                        indentation + 1,
                    )?;
                }
            }
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

impl ObjectGraph {
    pub fn add_primitive_array_object<I>(
        &mut self,
        id: NodeIndex,
        elem_type: &ValueType,
        values: I,
        cache: &Cache,
    ) -> NodeIndex
    where
        I: IntoIterator,
        I::Item: Into<PrimitiveValue>,
    {
        let obj_type = ValueType::array_obj_cached_string(elem_type, cache);
        let map = values
            .into_iter()
            .enumerate()
            .map(|(i, v)| (scached!(cache; i.to_string()), v));
        self.add_simple_object_from_map(id, obj_type, map)
    }

    pub fn add_array_object<I>(
        &mut self,
        id: NodeIndex,
        elem_type: &ValueType,
        values: I,
        cache: &Cache,
    ) -> NodeIndex
    where
        I: IntoIterator<Item = Value>,
    {
        let obj_type = ValueType::array_obj_cached_string(elem_type, cache);
        let values_map = values
            .into_iter()
            .enumerate()
            .map(|(i, v)| (scached!(cache; i.to_string()), v));
        self.add_object_from_map(id, obj_type, values_map)
    }

    pub fn add_simple_object_from_map<I, T>(
        &mut self,
        id: NodeIndex,
        obj_type: ObjectType,
        map: I,
    ) -> NodeIndex
    where
        I: IntoIterator<Item = (FieldName, T)>,
        T: Into<PrimitiveValue>,
    {
        let mut fields = FieldsMap::new();

        for (key, val) in map {
            fields.insert(key, val.into());
        }

        self.add_simple_object(id, obj_type, fields)
    }

    pub fn add_object_from_map<I>(
        &mut self,
        id: NodeIndex,
        obj_type: ObjectType,
        map: I,
    ) -> NodeIndex
    where
        I: IntoIterator<Item = (FieldName, Value)>,
    {
        let mut fields = FieldsMap::new();
        let mut obj_keys = vec![];

        for (key, val) in map {
            Self::visit_value_in_map(val, &mut fields, key, &mut obj_keys);
        }

        if obj_keys.is_empty() {
            self.add_simple_object(id, obj_type, fields)
        } else {
            self.add_object(id, obj_type, fields, obj_keys)
        }
    }

    fn visit_value_in_map(
        val: Value,
        fields: &mut FieldsMap,
        key: FieldName,
        obj_keys: &mut Vec<(FieldName, ObjectValue)>,
    ) {
        match val {
            Value::Primitive(p) => {
                fields.insert(key, p);
            }
            Value::Object(o) => {
                obj_keys.push((key, o));
            }
        };
    }

    fn add_object(
        &mut self,
        id: NodeIndex,
        obj_type: ObjectType,
        fields: FieldsMap,
        obj_keys: Vec<(FieldName, ObjectValue)>,
    ) -> NodeIndex {
        let pointers = PointersMap::from_iter(obj_keys.into_iter().map(|(key, neig)| {
            if self.contains_node(&neig.node) {
                (key, EdgeEndPoint::Internal(neig.node))
            } else {
                (key, EdgeEndPoint::Chain(neig.graph_id, neig.node))
            }
        }));

        let _ = self.add_node_with_pointers(id, obj_type, fields, pointers);
        id
    }

    pub fn add_simple_object(
        &mut self,
        id: NodeIndex,
        obj_type: ObjectType,
        fields: FieldsMap,
    ) -> NodeIndex {
        self.add_node(id, obj_type, fields.into()).id
    }
}

impl GraphMapWrap<Self> for ObjectGraph {
    fn wrap<'a>(&'a self, graphs_map: &'a GraphsMap) -> GraphMapValue<'a, Self> {
        GraphMapValue::from(&self, graphs_map)
    }
}

impl GraphMapEq for ObjectGraph {
    fn eq(&self, self_graphs_map: &GraphsMap, other: &Self, other_graphs_map: &GraphsMap) -> bool {
        graph_equality::equal_graphs(self_graphs_map, other_graphs_map, self, other)
    }
}

impl GraphMapHash for ObjectGraph {
    fn calculate_hash<H: Hasher>(&self, state: &mut H, graphs_map: &GraphsMap) {
        for (root_name, _) in &self.roots {
            root_name.hash(state);
        }

        for (_, node) in ObjectGraphWalker::from_graph(graphs_map, self.id) {
            node.hash(state);
        }
    }
}
