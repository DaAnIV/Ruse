use std::{
    collections::{hash_map, BTreeMap, HashMap, HashSet},
    fmt,
    hash::{BuildHasherDefault, DefaultHasher},
    ops::{self, AddAssign, SubAssign},
    sync::Arc,
};

use itertools::Itertools;

use crate::{
    graph_equality,
    graph_map_value::{GraphMapValue, GraphMapWrap},
    graph_node::*,
    node_index::{DefaultIx, NodeIndex},
    scached,
    value::{ObjectValue, Value, ValueType},
    Cache, FieldsMap, PrimitiveValue,
};
pub type GraphIndex = DefaultIx;

#[derive(Debug, Clone, Copy)]
pub struct GraphRoot {
    pub graph: GraphIndex,
    pub node: NodeIndex,
}

type RootsMap = BTreeMap<RootName, GraphRoot>;
type NodeRootNamesMap = HashMap<NodeIndex, HashSet<RootName>>;

#[derive(Clone)]
pub struct GraphsMap {
    graphs: HashMap<GraphIndex, Arc<ObjectGraph>, BuildHasherDefault<DefaultHasher>>,
    roots: RootsMap,
    node_roots_names: NodeRootNamesMap,
}

impl GraphsMap {
    pub fn new() -> Self {
        return Self {
            graphs: Default::default(),
            roots: Default::default(),
            node_roots_names: Default::default(),
        };
    }

    pub fn insert_graph(&mut self, graph: Arc<ObjectGraph>) -> Option<Arc<ObjectGraph>> {
        self.graphs.insert(graph.id, graph)
    }

    pub fn remove(&mut self, id: GraphIndex) -> Option<Arc<ObjectGraph>> {
        self.graphs.remove(&id)
    }

    pub fn insert_graph_if_new(&mut self, graph: Arc<ObjectGraph>) -> bool {
        match self.graphs.entry(graph.id) {
            std::collections::hash_map::Entry::Occupied(_) => false,
            std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                vacant_entry.insert(graph);
                true
            }
        }
    }

    pub fn get(&self, index: &GraphIndex) -> Option<&Arc<ObjectGraph>> {
        self.graphs.get(&index)
    }

    pub fn get_mut(&mut self, index: &GraphIndex) -> Option<&mut Arc<ObjectGraph>> {
        self.graphs.get_mut(&index)
    }

    pub fn get_or_create_mut(&mut self, graph_id: GraphIndex) -> &mut Arc<ObjectGraph> {
        match self.graphs.entry(graph_id) {
            std::collections::hash_map::Entry::Occupied(g) => g.into_mut(),
            std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                vacant_entry.insert(ObjectGraph::new(graph_id).into())
            }
        }
    }

    pub fn contains(&self, index: GraphIndex) -> bool {
        self.graphs.contains_key(&index)
    }

    pub fn graph_indices(&self) -> impl std::iter::Iterator<Item = &GraphIndex> {
        self.graphs.keys()
    }

    pub fn graphs(&self) -> impl std::iter::Iterator<Item = &Arc<ObjectGraph>> {
        self.graphs.values()
    }

    pub fn extend(&mut self, other: &GraphsMap) {
        for (key, value) in &other.graphs {
            self.graphs.entry(*key).or_insert(value.clone());
        }
    }

    pub fn keys(&self) -> impl std::iter::Iterator<Item = &usize> {
        self.graphs.keys()
    }

    pub fn add_static_graphs(&mut self, from: &GraphsMap) {
        for g in from.graphs.values() {
            if g.is_static() {
                self.insert_graph_if_new(g.clone());
            }
        }
    }

    pub fn node_count(&self) -> usize {
        self.graphs().fold(0, |acc, g| acc + g.node_count())
    }
}

impl GraphsMap {
    pub fn get_root(&self, name: &RootName) -> Option<&GraphRoot> {
        self.roots.get(name)
    }

    pub fn set_as_root(&mut self, name: RootName, graph: GraphIndex, node: NodeIndex) {
        let graph_root = GraphRoot { graph, node };
        match self.node_roots_names.entry(node) {
            hash_map::Entry::Vacant(vacant_entry) => {
                vacant_entry.insert(HashSet::from_iter([name.clone()]));
            }
            hash_map::Entry::Occupied(mut occupied_entry) => {
                occupied_entry.get_mut().insert(name.clone());
            }
        };
        if let Some(old_root) = self.roots.insert(name.clone(), graph_root) {
            if old_root.node != node {
                let remove_entry = {
                    let names = self.node_roots_names.get_mut(&old_root.node).unwrap();
                    names.remove(&name);
                    names.is_empty()
                };
                if remove_entry {
                    self.node_roots_names.remove(&old_root.node);
                }
            }
        }
    }

    pub fn root_names(&self) -> impl std::iter::Iterator<Item = &RootName> {
        self.roots.keys()
    }

    pub fn roots(&self) -> impl std::iter::Iterator<Item = (&RootName, &GraphRoot)> {
        self.roots.iter()
    }

    pub fn is_root(&self, node_id: &NodeIndex) -> bool {
        self.node_roots_names.contains_key(node_id)
    }

    pub fn node_root_names(&self, node_id: &NodeIndex) -> Option<impl Iterator<Item = &RootName>> {
        let root_names = self.node_roots_names.get(node_id)?;
        Some(root_names.iter())
    }
}

impl Default for GraphsMap {
    fn default() -> Self {
        Self::new()
    }
}

impl ops::Index<&GraphIndex> for GraphsMap {
    type Output = Arc<ObjectGraph>;

    fn index(&self, index: &GraphIndex) -> &Self::Output {
        self.get(index)
            .expect(&format!("No entry found for key {}", index))
    }
}

impl ops::Index<GraphIndex> for GraphsMap {
    type Output = Arc<ObjectGraph>;

    fn index(&self, index: GraphIndex) -> &Self::Output {
        self.get(&index)
            .expect(&format!("No entry found for key {}", index))
    }
}

impl fmt::Display for GraphsMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut seen = HashSet::new();
        write!(f, "{{")?;
        for (_, names) in self.node_roots_names.iter() {
            let root = &self.roots[names.iter().next().unwrap()];
            self.graphs[&root.graph].fmt_node_with_indentation_and_name(
                f,
                self,
                &root.node,
                &names.iter().join(","),
                1,
                &mut seen,
            )?;
        }
        write!(f, "}}")?;

        Ok(())
    }
}

impl fmt::Debug for GraphsMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (graph_id, g) in self.graphs.iter() {
            write!(f, "[{}]: ", graph_id)?;
            fmt::Debug::fmt(g, f)?;
        }

        Ok(())
    }
}

impl Eq for GraphsMap {}
impl PartialEq for GraphsMap {
    fn eq(&self, other: &Self) -> bool {
        if !self.roots.keys().eq(other.roots.keys()) {
            return false;
        }
        graph_equality::equal_graphs_by_root_names(self, other, self.roots.keys())
    }
}

type NodesMap = HashMap<NodeIndex, Arc<ObjectGraphNode>>;

#[derive(Clone)]
pub struct ObjectGraph {
    pub id: GraphIndex,
    is_static: bool,
    pub(crate) nodes: NodesMap,
    pub(crate) chained_graphs: HashMap<GraphIndex, usize>,
}

impl ObjectGraph {
    pub fn new(id: GraphIndex) -> Self {
        Self {
            id,
            nodes: Default::default(),
            chained_graphs: Default::default(),
            is_static: false,
        }
    }

    pub fn new_static(id: GraphIndex) -> Self {
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

    pub fn add_node(&mut self, id: NodeIndex, node: ObjectGraphNode) -> &mut ObjectGraphNode {
        assert!(!self.nodes.contains_key(&id));

        self.nodes.insert(id, node.into());
        Arc::get_mut(self.nodes.get_mut(&id).unwrap()).unwrap()
    }

    pub fn construct_node(
        &mut self,
        id: NodeIndex,
        obj_type: ObjectType,
        fields: FieldsMap,
    ) -> &mut ObjectGraphNode {
        let node = ObjectGraphNode::new(obj_type, fields, Default::default());

        self.add_node(id, node)
    }

    fn add_node_with_pointers(
        &mut self,
        id: NodeIndex,
        obj_type: ObjectType,
        fields: FieldsMap,
        pointers: PointersMap,
    ) -> &mut ObjectGraphNode {
        let node = ObjectGraphNode::new(obj_type, fields, pointers);

        self.add_node(id, node)
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

    pub fn set_edge(&mut self, a: &NodeIndex, b: NodeIndex, name: FieldName) {
        assert!(self.nodes.contains_key(&b));
        let node_a = self.get_mut_node(a).unwrap();
        node_a.insert_internal_edge(name.clone(), b);
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
        if let Some(EdgeEndPoint::Chain(graph, _)) = node_a.pointers_remove(&name) {
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
        Some(&self.get_node(node)?.obj_type())
    }

    pub fn set_field(&mut self, node: &NodeIndex, field: FieldName, value: &Value) {
        match value {
            Value::Primitive(primitive_value) => {
                self.set_primitive_field(node, field, primitive_value.clone())
            }
            Value::Object(object_value) => {
                if object_value.graph_id == self.id {
                    self.set_edge(node, object_value.node, field);
                } else {
                    self.set_chain_edge(node, object_value.graph_id, object_value.node, field);
                }
            }
            Value::Null => {
                self.remove_edge(node, field);
            }
        }
    }

    pub fn get_primitive_field(
        &self,
        node: &NodeIndex,
        field: &FieldName,
    ) -> Option<&PrimitiveField> {
        self.get_node(node)?.get_field(field).clone()
    }

    pub fn set_primitive_field(
        &mut self,
        node: &NodeIndex,
        field: FieldName,
        value: PrimitiveValue,
    ) {
        self.set_primitive_field_with_attributes(node, field, value, Attributes::default());
    }

    pub fn set_primitive_field_with_attributes(
        &mut self,
        node: &NodeIndex,
        field: FieldName,
        value: PrimitiveValue,
        attributes: Attributes,
    ) {
        if let Some(mut_node) = self.get_mut_node(node) {
            mut_node.insert_field_with_attributes(field, value, attributes);
        } else {
            assert!(false, "Graph {} doesn't contain node {}", &self.id, node)
        }
    }

    pub fn delete_primitive_field(
        &mut self,
        node: &NodeIndex,
        field: &FieldName,
    ) -> Option<PrimitiveField> {
        self.get_mut_node(node).unwrap().remove_field(field)
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

    fn fmt_node_with_indentation_and_name(
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

        write!(f, "{}[{}] {} ", indent_str, node, weight.obj_type())?;
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
            match val {
                EdgeEndPoint::Internal(index) => {
                    self.fmt_node_with_indentation_and_name(
                        f,
                        graphs_map,
                        index,
                        field_name.as_str(),
                        indentation + 1,
                        seen,
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
                        seen,
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

    pub fn add_primitive_array_object_from_fields<I>(
        &mut self,
        id: NodeIndex,
        elem_type: &ValueType,
        values: I,
        cache: &Cache,
    ) -> NodeIndex
    where
        I: IntoIterator,
        I::Item: Into<PrimitiveField>,
    {
        let obj_type = ValueType::array_obj_cached_string(elem_type, cache);
        let map = values
            .into_iter()
            .enumerate()
            .map(|(i, f)| (scached!(cache; i.to_string()), f));
        self.add_simple_object_from_fields_map(id, obj_type, map)
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

    pub fn add_primitive_set_object<I>(
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
        let obj_type = ValueType::set_obj_cached_string(elem_type, cache);
        let map = values.into_iter().map(|v| {
            let pv: PrimitiveValue = v.into();
            (scached!(cache; pv.to_string()), pv)
        });
        self.add_simple_object_from_map(id, obj_type, map)
    }

    pub fn add_primitive_set_object_from_fields<I>(
        &mut self,
        id: NodeIndex,
        elem_type: &ValueType,
        values: I,
        cache: &Cache,
    ) -> NodeIndex
    where
        I: IntoIterator,
        I::Item: Into<PrimitiveField>,
    {
        let obj_type = ValueType::set_obj_cached_string(elem_type, cache);
        let map = values.into_iter().map(|f| {
            let pf: PrimitiveField = f.into();
            (scached!(cache; pf.value.to_string()), pf)
        });
        self.add_simple_object_from_fields_map(id, obj_type, map)
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

        for (key, value) in map {
            let pv: PrimitiveValue = value.into();
            fields.insert(key, pv.into());
        }

        self.add_simple_object(id, obj_type, fields)
    }

    pub fn add_simple_object_from_fields_map<I, T>(
        &mut self,
        id: NodeIndex,
        obj_type: ObjectType,
        map: I,
    ) -> NodeIndex
    where
        I: IntoIterator<Item = (FieldName, T)>,
        T: Into<PrimitiveField>,
    {
        let mut fields = FieldsMap::new();

        for (key, field) in map {
            fields.insert(key, field.into());
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

    pub fn add_object_from_fields_map<I>(
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
                fields.insert(key, p.into());
            }
            Value::Object(o) => {
                obj_keys.push((key, o));
            }
            Value::Null => (),
        };
    }

    pub fn add_object(
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
        self.construct_node(id, obj_type, fields.into());
        id
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
