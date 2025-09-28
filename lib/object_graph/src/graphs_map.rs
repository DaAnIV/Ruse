use std::{
    collections::{hash_map, BTreeMap, HashMap, HashSet},
    fmt,
    hash::{BuildHasherDefault, DefaultHasher},
    ops,
    sync::Arc,
};

use itertools::Itertools;

use crate::{
    field_name, graph_equality,
    graph_walk::ObjectGraphWalker,
    value::{ObjectValue, Value},
    vobj, EdgeEndPoint, FieldName, FieldsMap, GraphIndex, NodeIndex, ObjectGraph, ObjectGraphNode,
    ObjectType, PointersMap, PrimitiveField, PrimitiveValue, RootName, ValueType,
};

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
        let old = self.graphs.insert(graph.id, graph);

        old
    }

    pub fn remove(&mut self, id: GraphIndex) -> Option<Arc<ObjectGraph>> {
        let old = self.graphs.remove(&id);

        old
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

    fn get_mut(&mut self, index: &GraphIndex) -> Option<&mut Arc<ObjectGraph>> {
        self.graphs.get_mut(&index)
    }

    pub fn ensure_graph(&mut self, graph_id: GraphIndex) {
        match self.graphs.entry(graph_id) {
            std::collections::hash_map::Entry::Occupied(_) => (),
            std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                vacant_entry.insert(ObjectGraph::new(graph_id).into());
            }
        };
    }

    pub fn new_static_graph(&mut self, graph_id: GraphIndex) {
        let old = self.insert_graph(Arc::new(ObjectGraph::new_static(graph_id)));
        assert!(old.is_none());
    }

    pub fn contains(&self, index: GraphIndex) -> bool {
        self.graphs.contains_key(&index)
    }

    pub fn contains_node(&self, graph_id: &GraphIndex, node_id: &NodeIndex) -> bool {
        self.graphs
            .get(graph_id)
            .map(|g| g.contains_node(node_id))
            .unwrap_or(false)
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

    pub fn keys(&self) -> impl std::iter::Iterator<Item = &GraphIndex> {
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

    pub fn node_ids(&self) -> impl Iterator<Item = &NodeIndex> {
        self.graphs().flat_map(|g| g.node_ids())
    }

    pub fn graph_node_ids<'a>(&'a self) -> impl Iterator<Item = (GraphIndex, NodeIndex)> + 'a {
        self.graphs().flat_map(|g| g.node_ids().map(|n| (g.id, *n)))
    }

    pub fn neighbors<'a>(
        &'a self,
        graph: GraphIndex,
        node: NodeIndex,
    ) -> impl Iterator<Item = (GraphIndex, NodeIndex)> + 'a {
        self.get(&graph)
            .unwrap()
            .neighbors(&node)
            .map(move |(_, edge)| (edge.graph.unwrap_or(graph), edge.node))
    }

    pub fn set_as_immutable(&mut self, root: &RootName) {
        let nodes = if let Some(graph_root) = self.roots.get(root) {
            ObjectGraphWalker::from_node(&self, graph_root.graph, graph_root.node)
                .map(|(graph, node, _)| (graph.id, node))
                .collect_vec()
        } else {
            vec![]
        };

        for (graph_id, node_id) in nodes {
            let graph = Arc::make_mut(self.get_mut(&graph_id).unwrap());
            let node = graph.get_mut_node(&node_id).unwrap();
            node.attributes.readonly = true;
        }
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

    pub(crate) fn common_roots<'a>(
        &'a self,
        other_graphs_map: &'a GraphsMap,
    ) -> impl Iterator<Item = &'a RootName> {
        self.roots
            .keys()
            .filter(|root| other_graphs_map.roots.contains_key(*root))
    }
}

impl GraphsMap {
    pub fn obj_type(&self, graph_id: GraphIndex, node_id: NodeIndex) -> Option<&ObjectType> {
        self.get(&graph_id).and_then(|g| g.obj_type(&node_id))
    }
}


impl GraphsMap {
    pub fn construct_node(
        &mut self,
        graph: GraphIndex,
        id: NodeIndex,
        obj_type: ObjectType,
        fields: FieldsMap,
    ) {
        self.add_node_with_pointers(graph, id, obj_type, fields, Default::default());
    }

    pub fn add_node_with_pointers(
        &mut self,
        graph: GraphIndex,
        id: NodeIndex,
        obj_type: ObjectType,
        fields: FieldsMap,
        pointers: PointersMap,
    ) {
        let graph = Arc::make_mut(self.get_mut(&graph).unwrap());
        let node = ObjectGraphNode::new(obj_type, fields, pointers);
        graph.add_node(id, node);
    }

    pub fn set_field(
        &mut self,
        field: FieldName,
        graph: GraphIndex,
        node: NodeIndex,
        value: &Value,
    ) -> bool {
        if self[graph].get_node(&node).unwrap().attributes.readonly {
            return false;
        }

        match value {
            Value::Primitive(primitive_value) => {
                self.set_primitive_field(field, graph, node, primitive_value.clone())
            }
            Value::Object(object_value) => {
                self.set_edge(field, graph, node, object_value.graph_id, object_value.node);
            }
            Value::Null => {
                self.remove_edge(&field, graph, node);
            }
        }

        true
    }

    pub fn set_primitive_field(
        &mut self,
        field: FieldName,
        graph: GraphIndex,
        node: NodeIndex,
        value: PrimitiveValue,
    ) {
        let graph = Arc::make_mut(self.get_mut(&graph).unwrap());
        graph.set_primitive_field(node, field, value);
    }

    pub fn set_edge(
        &mut self,
        field: FieldName,
        graph_a: GraphIndex,
        node_a: NodeIndex,
        graph_b: GraphIndex,
        node_b: NodeIndex,
    ) {
        let graph = Arc::make_mut(self.get_mut(&graph_a).unwrap());
        if graph_a == graph_b {
            graph.set_internal_edge(node_a, node_b, field);
        } else {
            graph.set_chain_edge(node_a, graph_b, node_b, field);
        }
    }

    pub fn delete_field(
        &mut self,
        field: &FieldName,
        graph: GraphIndex,
        node: NodeIndex,
    ) -> Option<Value> {
        if let Some(primitive_field) = self.delete_primitive_field(field, graph, node) {
            Some(Value::Primitive(primitive_field.value))
        } else if let Some((neig_graph_index, neig_node_index)) =
            self.remove_edge(field, graph, node)
        {
            let obj_type = self[neig_graph_index]
                .obj_type(&neig_node_index)
                .unwrap()
                .clone();
            Some(vobj!(obj_type, neig_graph_index, neig_node_index))
        } else {
            None
        }
    }

    pub fn delete_primitive_field(
        &mut self,
        field: &FieldName,
        graph: GraphIndex,
        node: NodeIndex,
    ) -> Option<PrimitiveField> {
        let graph = Arc::make_mut(self.get_mut(&graph).unwrap());
        graph.delete_primitive_field(node, field)
    }

    pub fn remove_edge(
        &mut self,
        field: &FieldName,
        graph_id: GraphIndex,
        node: NodeIndex,
    ) -> Option<(GraphIndex, NodeIndex)> {
        let graph = Arc::make_mut(self.get_mut(&graph_id).unwrap());
        let old = graph.remove_edge(node, field)?;

        Some((old.graph.unwrap_or(graph_id), old.node))
    }
}

impl GraphsMap {
    pub fn add_primitive_array_object<I>(
        &mut self,
        graph: GraphIndex,
        id: NodeIndex,
        elem_type: &ValueType,
        values: I,
    ) -> NodeIndex
    where
        I: IntoIterator,
        I::Item: Into<PrimitiveValue>,
    {
        let obj_type = ObjectType::array_obj_type(elem_type);
        let map = values
            .into_iter()
            .enumerate()
            .map(|(i, v)| (field_name!(i.to_string()), v));
        self.add_simple_object_from_map(graph, id, obj_type, map)
    }

    pub fn add_primitive_array_object_from_fields<I>(
        &mut self,
        graph: GraphIndex,
        id: NodeIndex,
        elem_type: &ValueType,
        values: I,
    ) -> NodeIndex
    where
        I: IntoIterator,
        I::Item: Into<PrimitiveField>,
    {
        let obj_type = ObjectType::array_obj_type(elem_type);
        let map = values
            .into_iter()
            .enumerate()
            .map(|(i, f)| (field_name!(i.to_string()), f));
        self.add_simple_object_from_fields_map(graph, id, obj_type, map)
    }

    pub fn add_array_object<I>(
        &mut self,
        graph: GraphIndex,
        id: NodeIndex,
        elem_type: &ValueType,
        values: I,
    ) -> NodeIndex
    where
        I: IntoIterator<Item = Value>,
    {
        let obj_type = ObjectType::array_obj_type(elem_type);
        let values_map = values
            .into_iter()
            .enumerate()
            .map(|(i, v)| (field_name!(i.to_string()), v));
        self.add_object_from_map(graph, id, obj_type, values_map)
    }

    pub fn add_primitive_set_object<I>(
        &mut self,
        graph: GraphIndex,
        id: NodeIndex,
        elem_type: &ValueType,
        values: I,
    ) -> NodeIndex
    where
        I: IntoIterator,
        I::Item: Into<PrimitiveValue>,
    {
        let obj_type = ObjectType::array_obj_type(elem_type);
        let map = values.into_iter().map(|v| {
            let pv: PrimitiveValue = v.into();
            (field_name!(pv.to_string()), pv)
        });
        self.add_simple_object_from_map(graph, id, obj_type, map)
    }

    pub fn add_primitive_set_object_from_fields<I>(
        &mut self,
        graph: GraphIndex,
        id: NodeIndex,
        elem_type: &ValueType,
        values: I,
    ) -> NodeIndex
    where
        I: IntoIterator,
        I::Item: Into<PrimitiveField>,
    {
        let obj_type = ObjectType::set_obj_type(elem_type);
        let map = values.into_iter().map(|f| {
            let pf: PrimitiveField = f.into();
            (field_name!(pf.value.to_string()), pf)
        });
        self.add_simple_object_from_fields_map(graph, id, obj_type, map)
    }

    pub fn add_primitive_map_object<I, V>(
        &mut self,
        graph: GraphIndex,
        id: NodeIndex,
        key_type: &ValueType,
        value_type: &ValueType,
        values: I,
    ) -> NodeIndex
    where
        I: IntoIterator<Item = (PrimitiveValue, V)>,
        V: Into<PrimitiveValue>,
    {
        let obj_type = ObjectType::map_obj_type(key_type, value_type);
        let map = values
            .into_iter()
            .map(|(k, v)| (field_name!(k.to_string()), v.into()));
        self.add_simple_object_from_map(graph, id, obj_type, map)
    }

    pub fn add_map_object<I>(
        &mut self,
        graph: GraphIndex,
        id: NodeIndex,
        key_type: &ValueType,
        value_type: &ValueType,
        values: I,
    ) -> NodeIndex
    where
        I: IntoIterator<Item = (PrimitiveValue, Value)>,
    {
        let obj_type = ObjectType::map_obj_type(key_type, value_type);
        let values_map = values
            .into_iter()
            .map(|(k, v)| (field_name!(k.to_string()), v));
        self.add_object_from_map(graph, id, obj_type, values_map)
    }

    pub fn add_simple_object_from_map<I, T>(
        &mut self,
        graph: GraphIndex,
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

        self.add_simple_object(graph, id, obj_type, fields)
    }

    pub fn add_simple_object_from_fields_map<I, T>(
        &mut self,
        graph: GraphIndex,
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

        self.add_simple_object(graph, id, obj_type, fields)
    }

    pub fn add_object_from_map<I>(
        &mut self,
        graph: GraphIndex,
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
            self.add_simple_object(graph, id, obj_type, fields)
        } else {
            self.add_object(graph, id, obj_type, fields, obj_keys)
        }
    }

    pub fn add_object_from_fields_map<I>(
        &mut self,
        graph: GraphIndex,
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
            self.add_simple_object(graph, id, obj_type, fields)
        } else {
            self.add_object(graph, id, obj_type, fields, obj_keys)
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
        graph_id: GraphIndex,
        id: NodeIndex,
        obj_type: ObjectType,
        fields: FieldsMap,
        obj_keys: Vec<(FieldName, ObjectValue)>,
    ) -> NodeIndex {
        let graph = Arc::make_mut(self.get_mut(&graph_id).unwrap());
        let pointers = PointersMap::from_iter(obj_keys.into_iter().map(|(key, neig)| {
            if graph.contains_node(&neig.node) {
                (key, EdgeEndPoint::internal(neig.node, Default::default()))
            } else {
                (
                    key,
                    EdgeEndPoint::chain(neig.graph_id, neig.node, Default::default()),
                )
            }
        }));

        self.add_node_with_pointers(graph_id, id, obj_type, fields, pointers);
        id
    }

    pub fn add_simple_object(
        &mut self,
        graph: GraphIndex,
        id: NodeIndex,
        obj_type: ObjectType,
        fields: FieldsMap,
    ) -> NodeIndex {
        self.construct_node(graph, id, obj_type, fields);
        id
    }
}

impl GraphsMap {
    pub fn get_graphs_for_roots<'a>(
        &self,
        variables: impl Iterator<Item = &'a RootName>,
    ) -> GraphsMap {
        let mut pruned_graphs_map = GraphsMap::new();
        for v in variables {
            if let Some(root) = self.get_root(v) {
                pruned_graphs_map.set_as_root(v.clone(), root.graph, root.node);
                pruned_graphs_map
                    .insert_graph_and_chained_graphs(self.graphs[&root.graph].clone(), &self);
            }
        }
        pruned_graphs_map
    }

    fn insert_graph_and_chained_graphs(&mut self, graph: Arc<ObjectGraph>, graphs_map: &GraphsMap) {
        if self.graphs.insert(graph.id, graph.clone()).is_none() {
            for g in graph.chained_graphs() {
                self.insert_graph_and_chained_graphs(graphs_map.graphs[g].clone(), graphs_map);
            }
        }
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
            .expect(&format!("No graph found for Graph ID {}", index))
    }
}

impl ops::Index<GraphIndex> for GraphsMap {
    type Output = Arc<ObjectGraph>;

    fn index(&self, index: GraphIndex) -> &Self::Output {
        self.get(&index)
            .expect(&format!("No graph found for Graph ID {}", index))
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
