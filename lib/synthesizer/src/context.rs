use crate::location::{LocValue, Location, VarLoc};
use downcast_rs::{impl_downcast, DowncastSync};
use graph_equality::equal_graphs_by_nodes;
use itertools::Itertools;
use ruse_object_graph::{
    dot::{Dot, DotConfig},
    graph_map_value::*,
    mermaid::{Mermaid, MermaidConfig},
    value::{ObjectValue, Value},
    *,
};
use std::{
    collections::{btree_map, BTreeMap, HashMap, HashSet, VecDeque},
    fmt::Display,
    hash::{DefaultHasher, Hash, Hasher},
    ops::{Deref, DerefMut, Index},
    sync::{
        atomic::{self, AtomicUsize},
        Arc,
    },
};

pub type VariableName = RootName;

#[derive(PartialEq, Eq, Debug, Clone, Hash)]
pub struct Variable {
    pub name: VariableName,
    pub value_type: ValueType,
    pub immutable: bool,
}

#[derive(Debug)]
pub struct GraphIdGenerator {
    node_id: AtomicUsize,
    graph_id: AtomicUsize,
}

impl GraphIdGenerator {
    pub fn with_initial_values(node_id: NodeIndex, graph_id: GraphIndex) -> Self {
        Self {
            node_id: node_id.0.into(),
            graph_id: graph_id.0.into(),
        }
    }

    pub fn get_id_for_node(&self) -> NodeIndex {
        NodeIndex(self.node_id.fetch_add(1, atomic::Ordering::Relaxed))
    }

    pub fn get_id_for_graph(&self) -> GraphIndex {
        GraphIndex(self.graph_id.fetch_add(1, atomic::Ordering::Relaxed))
    }

    pub fn max_node_id(&self) -> NodeIndex {
        NodeIndex(self.node_id.load(atomic::Ordering::Relaxed))
    }

    pub fn max_graph_id(&self) -> GraphIndex {
        GraphIndex(self.graph_id.load(atomic::Ordering::Relaxed))
    }
}

impl Default for GraphIdGenerator {
    fn default() -> Self {
        Self::with_initial_values(0.into(), 0.into())
    }
}

pub trait SynthesizerContextData: DowncastSync {}
impl_downcast!(sync SynthesizerContextData);

pub struct SynthesizerContext {
    all_variables: Arc<BTreeMap<VariableName, Variable>>,
    pub start_context: ContextArray,
    pub data: Box<dyn SynthesizerContextData>,
}

pub trait SynthesizerWorkerContextData: DowncastSync {}
impl_downcast!(sync SynthesizerWorkerContextData);

pub struct EmptySynthesizerData {}
impl SynthesizerContextData for EmptySynthesizerData {}
impl SynthesizerWorkerContextData for EmptySynthesizerData {}

pub struct SynthesizerWorkerContext {
    pub index: usize,
    pub data: Box<dyn SynthesizerWorkerContextData>,
}
impl Default for SynthesizerWorkerContext {
    fn default() -> Self {
        Self {
            index: 0,
            data: Box::new(EmptySynthesizerData {}),
        }
    }
}

impl SynthesizerContext {
    pub fn from_context_array(context_array: ContextArray) -> Self {
        Self::from_context_array_with_data(context_array, Box::new(EmptySynthesizerData {}))
    }
    pub fn from_context_array_with_data(
        context_array: ContextArray,
        data: Box<dyn SynthesizerContextData>,
    ) -> Self {
        Self {
            all_variables: context_array.get_variables(),
            start_context: context_array,
            data,
        }
    }
    pub fn get_variable(&self, name: &VariableName) -> Option<&Variable> {
        self.all_variables.get(name)
    }

    pub fn set_immutable(&mut self, var: &VariableName) {
        let all_variables = Arc::get_mut(&mut self.all_variables).unwrap();
        let var = all_variables.get_mut(var).unwrap();
        var.immutable = true;
    }

    pub fn variables_count(&self) -> usize {
        self.all_variables.len()
    }
}

#[derive(serde::Serialize)]
pub struct SynthesizerContextJsonDisplay {
    all_variables: HashMap<String, String>,
    start_context: Vec<ContextJsonDisplay>,
}

impl SynthesizerContext {
    pub fn json_display(&self) -> impl Display {
        self.json_display_struct()
    }

    pub fn json_display_struct(&self) -> SynthesizerContextJsonDisplay {
        SynthesizerContextJsonDisplay {
            all_variables: self
                .all_variables
                .iter()
                .map(|(k, v)| (k.to_string(), v.value_type.to_string()))
                .collect(),
            start_context: self.start_context.json_display_struct().contexts,
        }
    }
}

impl Display for SynthesizerContextJsonDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = serde_json::to_string_pretty(self).unwrap();
        write!(f, "{}", value)
    }
}

type ValuesHashMap = BTreeMap<VariableName, u64>;

#[derive(Debug, Clone, Default)]
pub struct ValuesMap(BTreeMap<VariableName, Value>);

impl ValuesMap {
    fn get_hashes(&self, graphs_map: &GraphsMap) -> ValuesHashMap {
        let mut map = ValuesHashMap::default();
        for (k, v) in &self.0 {
            let mut state = DefaultHasher::default();
            v.calculate_hash(&mut state, graphs_map);
            map.insert(k.clone(), state.finish());
        }

        map
    }
}

impl Deref for ValuesMap {
    type Target = BTreeMap<VariableName, Value>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ValuesMap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<const N: usize> From<[(VariableName, Value); N]> for ValuesMap {
    fn from(items: [(VariableName, Value); N]) -> Self {
        ValuesMap(BTreeMap::from(items))
    }
}

impl IntoIterator for ValuesMap {
    type Item = (VariableName, Value);

    type IntoIter = btree_map::IntoIter<VariableName, Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl GraphMapWrap<Self> for ValuesMap {
    fn wrap<'a>(&'a self, graphs_map: &'a GraphsMap) -> GraphMapValue<'a, Self> {
        GraphMapValue::from(&self, graphs_map)
    }
}

impl GraphMapHash for ValuesMap {
    fn calculate_hash<H: std::hash::Hasher>(&self, state: &mut H, graphs_map: &GraphsMap) {
        for (k, v) in &self.0 {
            k.hash(state);
            v.calculate_hash(state, graphs_map);
        }
    }
}

#[derive(Clone, Debug)]
pub struct Context {
    pub(crate) hashes: Arc<ValuesHashMap>,
    pub(crate) values: Arc<ValuesMap>,
    pub(crate) outputs: Arc<Vec<Value>>,
    pub graphs_map: Arc<GraphsMap>,
    pub graph_id_gen: Arc<GraphIdGenerator>,
}

impl Context {
    pub fn with_values(
        values: ValuesMap,
        graphs_map: Arc<GraphsMap>,
        graph_id_gen: Arc<GraphIdGenerator>,
    ) -> Box<Self> {
        Self::with_values_and_outputs(values, vec![], graphs_map, graph_id_gen)
    }

    pub fn with_values_and_outputs(
        values: ValuesMap,
        outputs: Vec<Value>,
        graphs_map: Arc<GraphsMap>,
        graph_id_gen: Arc<GraphIdGenerator>,
    ) -> Box<Self> {
        let mut instance = Box::new(Self {
            hashes: Default::default(),
            values: values.into(),
            graphs_map,
            graph_id_gen,
            outputs: outputs.into(),
        });

        instance.update_hash();

        #[cfg(debug_assertions)]
        instance.verify_values();

        instance
    }

    fn update_hash(&mut self) {
        self.hashes = self.values.get_hashes(&self.graphs_map).into();
    }

    fn set_values(&mut self, values: ValuesMap) {
        self.values = values.into();
        self.update_hash();

        #[cfg(debug_assertions)]
        self.verify_values();
    }

    #[cfg(debug_assertions)]
    fn verify_values(&self) -> bool {
        for (name, value) in self.values.iter() {
            if let Some(obj_val) = value.obj() {
                assert!(
                    self.graphs_map.get_root(name).is_some(),
                    "Variable {} is not a root in the graphs map",
                    name
                );
                let root = self.graphs_map.get_root(name).unwrap();
                assert!(
                    root.graph == obj_val.graph_id && root.node == obj_val.node,
                    "Variable {} does not match the root in the graphs map",
                    name
                );
                let graph = &self.graphs_map[obj_val.graph_id];
                assert!(
                    graph.obj_type(&obj_val.node) == Some(&obj_val.obj_type),
                    "Variable {} type {} does not match the object type {} in the graphs map",
                    name,
                    &graph.obj_type(&obj_val.node).unwrap(),
                    &obj_val.obj_type
                );
            }
        }

        true
    }

    pub fn temp_value(&self, val: Value) -> LocValue {
        LocValue {
            val,
            loc: Location::Temp,
        }
    }

    pub fn get_var_loc_value(
        &self,
        var: &VariableName,
        syn_ctx: &SynthesizerContext,
    ) -> Option<LocValue> {
        let val = self.values.get(var)?.clone();
        let readonly = syn_ctx.get_variable(var)?.immutable;
        Some(LocValue {
            val,
            loc: Location::Var(VarLoc {
                var: var.clone(),
                attrs: Attributes { readonly },
            }),
        })
    }

    pub fn get_var_value(&self, var: &VariableName) -> Option<Value> {
        self.values.get(var).map(|x| x.clone())
    }

    pub fn get_loc_value(&self, val: Value, loc: Location) -> LocValue {
        LocValue { val, loc }
    }

    pub fn update_value(
        &mut self,
        new_val: &Value,
        loc: &mut Location,
        syn_ctx: &SynthesizerContext,
    ) -> bool {
        match loc {
            Location::Var(l) => {
                let var = syn_ctx.all_variables.get(&l.var).unwrap();
                assert!(var.value_type == new_val.val_type());

                if var.immutable {
                    return false;
                }

                let mut new_values = (*self.values).clone();
                new_values.insert(l.var.clone(), new_val.clone());

                self.set_values(new_values);

                true
            }
            Location::ObjectField(l) => {
                if l.attrs.readonly {
                    return false;
                }
                self.set_field(l.graph, l.node, l.field.clone(), new_val)
            }
            Location::Temp => true,
        }
    }

    pub fn set_field(
        &mut self,
        graph_id: GraphIndex,
        node: NodeIndex,
        field_name: FieldName,
        value: &Value,
    ) -> bool {
        let graphs_map = Arc::make_mut(&mut self.graphs_map);
        graphs_map.set_field(field_name.clone(), graph_id, node, value);
        self.update_hash();
        true
    }

    pub fn delete_field(
        &mut self,
        graph: GraphIndex,
        node: NodeIndex,
        field_name: &FieldName,
    ) -> Option<Value> {
        let graphs_map = Arc::make_mut(&mut self.graphs_map);
        let result = graphs_map.delete_field(field_name, graph, node);
        self.update_hash();
        result
    }

    pub fn create_graph_in_map(&mut self) -> GraphIndex {
        let id = self.graph_id_gen.get_id_for_graph();
        let graphs_map = Arc::make_mut(&mut self.graphs_map);
        graphs_map.ensure_graph(id);

        id
    }

    pub fn insert_if_new(&mut self, new_graph: Arc<ObjectGraph>) {
        if !self.graphs_map.contains(new_graph.id) {
            let graphs_map = Arc::make_mut(&mut self.graphs_map);
            graphs_map.insert_graph_if_new(new_graph);
        }
    }

    pub fn variable_count(&self) -> usize {
        self.values.len()
    }

    pub fn variables(&self) -> impl std::iter::Iterator<Item = (&VariableName, &Value)> {
        self.values.iter()
    }

    pub fn variable_names(&self) -> impl std::iter::Iterator<Item = &VariableName> {
        self.values.keys()
    }

    pub fn variable_values(&self) -> impl std::iter::Iterator<Item = &Value> {
        self.values.values()
    }

    fn extend_graphs_map(&mut self, other: &Context) {
        let graphs_map = Arc::make_mut(&mut self.graphs_map);
        graphs_map.extend(&other.graphs_map);
    }

    pub(crate) fn get_partial_context<'a, I>(&self, required_variables: I) -> Option<Box<Self>>
    where
        I: IntoIterator<Item = &'a VariableName>,
    {
        let mut values = ValuesMap::default();
        let mut hashes = ValuesHashMap::default();

        let mut connected_variables = HashSet::new();
        for var in required_variables {
            connected_variables.insert(var);
            if let Value::Object(obj_val) = self.values.get(var)? {
                for (name, other_obj_val) in self
                    .values
                    .iter()
                    .filter_map(|(name, val)| Some((name, val.obj()?)))
                {
                    if self
                        .graphs_map
                        .nodes_connected(obj_val.node, other_obj_val.node)
                    {
                        connected_variables.insert(name);
                    }
                }
            }
        }

        for var in connected_variables.iter().map(|var| *var) {
            let var_value = self.values.get(var)?.clone();
            let var_hash = unsafe { *self.hashes.get(var).unwrap_unchecked() };
            values.insert(var.clone(), var_value);
            hashes.insert(var.clone(), var_hash);
        }

        #[cfg(feature = "prune_graph_map")]
        let partial_graphs_map = self
            .graphs_map
            .get_graphs_for_roots(connected_variables.into_iter());
        #[cfg(not(feature = "prune_graph_map"))]
        let partial_graphs_map = self.graphs_map.clone();

        Some(
            Self {
                hashes: hashes.into(),
                values: values.into(),
                graphs_map: partial_graphs_map.into(),
                graph_id_gen: self.graph_id_gen.clone(),
                outputs: Vec::new().into(),
            }
            .into(),
        )
    }
}

impl Context {
    pub fn add_output(&mut self, output: Value) {
        Arc::make_mut(&mut self.outputs).push(output);
    }

    pub fn clear_outputs(&mut self) {
        Arc::make_mut(&mut self.outputs).clear();
    }

    pub fn outputs(&self) -> impl Iterator<Item = &Value> {
        self.outputs.iter()
    }
}

impl Context {
    pub fn create_output_primitive_array<I>(
        &mut self,
        elem_type: &ValueType,
        values: I,
    ) -> ObjectValue
    where
        I: IntoIterator,
        I::Item: Into<PrimitiveValue>,
    {
        let out_graph_id = self.graph_id_gen.get_id_for_graph();
        let out_node_id = self.graph_id_gen.get_id_for_node();

        let graphs_map = Arc::make_mut(&mut self.graphs_map);
        graphs_map.ensure_graph(out_graph_id);
        graphs_map.add_primitive_array_object(out_graph_id, out_node_id, elem_type, values);

        ObjectValue {
            obj_type: ObjectType::array_obj_type(elem_type),
            graph_id: out_graph_id,
            node: out_node_id,
        }
    }

    pub fn create_output_primitive_array_from_fields<I>(
        &mut self,
        elem_type: &ValueType,
        values: I,
    ) -> ObjectValue
    where
        I: IntoIterator,
        I::Item: Into<PrimitiveField>,
    {
        let out_graph_id = self.graph_id_gen.get_id_for_graph();
        let out_node_id = self.graph_id_gen.get_id_for_node();

        let graphs_map = Arc::make_mut(&mut self.graphs_map);
        graphs_map.ensure_graph(out_graph_id);
        graphs_map.add_primitive_array_object_from_fields(
            out_graph_id,
            out_node_id,
            elem_type,
            values,
        );

        ObjectValue {
            obj_type: ObjectType::array_obj_type(elem_type),
            graph_id: out_graph_id,
            node: out_node_id,
        }
    }

    pub fn create_output_array_object<I>(&mut self, elem_type: &ValueType, values: I) -> ObjectValue
    where
        I: IntoIterator<Item = Value>,
    {
        let out_graph_id = self.graph_id_gen.get_id_for_graph();
        let out_node_id = self.graph_id_gen.get_id_for_node();

        let graphs_map = Arc::make_mut(&mut self.graphs_map);
        graphs_map.ensure_graph(out_graph_id);
        graphs_map.add_array_object(out_graph_id, out_node_id, elem_type, values);

        ObjectValue {
            obj_type: ObjectType::array_obj_type(elem_type),
            graph_id: out_graph_id,
            node: out_node_id,
        }
    }

    pub fn create_output_primitive_set<I>(
        &mut self,
        elem_type: &ValueType,
        values: I,
    ) -> ObjectValue
    where
        I: IntoIterator,
        I::Item: Into<PrimitiveValue>,
    {
        let out_graph_id = self.graph_id_gen.get_id_for_graph();
        let out_node_id = self.graph_id_gen.get_id_for_node();

        let graphs_map = Arc::make_mut(&mut self.graphs_map);
        graphs_map.ensure_graph(out_graph_id);
        graphs_map.add_primitive_set_object(out_graph_id, out_node_id, elem_type, values);

        ObjectValue {
            obj_type: ObjectType::set_obj_type(elem_type),
            graph_id: out_graph_id,
            node: out_node_id,
        }
    }

    pub fn create_output_simple_object_from_map<I, T>(
        &mut self,
        obj_type: ObjectType,
        map: I,
    ) -> ObjectValue
    where
        I: IntoIterator<Item = (FieldName, T)>,
        T: Into<PrimitiveValue>,
    {
        let out_graph_id = self.graph_id_gen.get_id_for_graph();
        let out_node_id = self.graph_id_gen.get_id_for_node();

        let graphs_map = Arc::make_mut(&mut self.graphs_map);
        graphs_map.ensure_graph(out_graph_id);
        graphs_map.add_simple_object_from_map(out_graph_id, out_node_id, obj_type.clone(), map);

        ObjectValue {
            obj_type: obj_type,
            graph_id: out_graph_id,
            node: out_node_id,
        }
    }

    pub fn create_output_object_from_map<I>(&mut self, obj_type: ObjectType, map: I) -> ObjectValue
    where
        I: IntoIterator<Item = (FieldName, Value)>,
    {
        let out_graph_id = self.graph_id_gen.get_id_for_graph();
        let out_node_id = self.graph_id_gen.get_id_for_node();

        let graphs_map = Arc::make_mut(&mut self.graphs_map);
        graphs_map.ensure_graph(out_graph_id);
        graphs_map.add_object_from_map(out_graph_id, out_node_id, obj_type.clone(), map);

        ObjectValue {
            obj_type: obj_type,
            graph_id: out_graph_id,
            node: out_node_id,
        }
    }
}

impl Context {
    pub fn reachable_nodes(&self) -> HashSet<(GraphIndex, NodeIndex)> {
        Self::reachable_nodes_from_iter(self.variable_values(), &self.graphs_map)
    }

    fn reachable_nodes_from_iter<'a, I>(
        values: I,
        graphs_map: &GraphsMap,
    ) -> HashSet<(GraphIndex, NodeIndex)>
    where
        I: Iterator<Item = &'a Value>,
    {
        let mut nodes = HashSet::new();

        for var in values {
            if let Value::Object(obj) = var {
                Self::add_reachable_nodes(graphs_map, obj.graph_id, obj.node, &mut nodes);
            }
        }

        nodes
    }

    fn add_reachable_nodes(
        graphs_map: &GraphsMap,
        graph_id: GraphIndex,
        node_id: NodeIndex,
        seen: &mut HashSet<(GraphIndex, NodeIndex)>,
    ) {
        let mut q = VecDeque::new();
        q.push_back((graph_id, node_id));
        while let Some((cur_graph_id, cur_node_id)) = q.pop_back() {
            if seen.contains(&(cur_graph_id, cur_node_id)) {
                continue;
            }

            seen.insert((cur_graph_id, cur_node_id));
            let graph = &graphs_map[cur_graph_id];
            for (_, neig) in graph.neighbors(&cur_node_id) {
                q.push_back((neig.graph.unwrap_or(cur_graph_id), neig.node));
            }
        }
    }
}

impl Context {
    pub fn subset(&self, other: &Self) -> bool {
        if !self.variable_names().all(|v| other.values.contains_key(v)) {
            return false;
        }

        let mut self_object_nodes = vec![];
        let mut other_object_nodes = vec![];

        for (key, self_value) in self.values.iter() {
            let other_value = &other.values[key];
            match (self_value, other_value) {
                (
                    Value::Primitive(self_primitive_value),
                    Value::Primitive(other_primitive_value),
                ) => {
                    if self_primitive_value != other_primitive_value {
                        return false;
                    }
                }
                (Value::Object(self_object_value), Value::Object(other_object_value)) => {
                    self_object_nodes.push((self_object_value.graph_id, self_object_value.node));
                    other_object_nodes.push((other_object_value.graph_id, other_object_value.node));
                }
                (_, _) => {
                    return false;
                }
            }
        }

        equal_graphs_by_nodes(
            &self.graphs_map,
            &other.graphs_map,
            self_object_nodes,
            other_object_nodes,
        )
    }
}

impl Hash for Context {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        for (k, hash) in self.hashes.iter() {
            k.hash(state);
            hash.hash(state);
        }
    }
}

impl Eq for Context {}

impl PartialEq for Context {
    fn eq(&self, other: &Self) -> bool {
        if self.hashes != other.hashes || self.values.len() != other.values.len() {
            return false;
        }

        if self.values.keys().ne(other.values.keys()) {
            return false;
        }

        if !self
            .values
            .iter()
            .filter(|(_, v)| v.is_primitive())
            .all(|(key, self_value)| {
                let other_value = &other.values[key];
                self_value.wrap(&self.graphs_map) == other_value.wrap(&other.graphs_map)
            })
        {
            return false;
        }

        let nodes_a = self.values.values().filter(|v| v.is_obj()).map(|v| {
            let obj = v.obj().unwrap();
            (obj.graph_id, obj.node)
        });
        let nodes_b = other.values.values().filter(|v| v.is_obj()).map(|v| {
            let obj = v.obj().unwrap();
            (obj.graph_id, obj.node)
        });

        equal_graphs_by_nodes(&self.graphs_map, &other.graphs_map, nodes_a, nodes_b)
    }
}

impl Display for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut iter = self.values.iter();
        let mut value = iter.next();
        while let Some((k, v)) = value {
            write!(
                f,
                "{} -> {}",
                k,
                v.mermaid_display_with_config(&self.graphs_map, MermaidConfig::subgraph_config(&k))
            )?;
            value = iter.next();
            if value.is_some() {
                write!(f, ", ")?;
            }
        }
        for (i, output) in self.outputs().enumerate() {
            write!(
                f,
                "; Output: {}",
                output.mermaid_display_with_config(
                    &self.graphs_map,
                    MermaidConfig::subgraph_config(&format!("output_{}", i))
                )
            )?;
        }

        write!(f, "; Hashes: ")?;
        let mut hashes_iter = self.hashes.iter();
        let mut hash_value = hashes_iter.next();
        while let Some((k, v)) = hash_value {
            write!(f, "{} -> {}", k, *v)?;
            hash_value = hashes_iter.next();
            if hash_value.is_some() {
                write!(f, ", ")?;
            }
        }
        write!(f, "; Graphs: {:?}", self.graphs_map.keys().collect_vec())?;

        Ok(())
    }
}

impl Context {
    pub fn dot_display(&self) -> Dot<'_> {
        let mut root_names = HashMap::from_iter(
            self.values
                .iter()
                .filter_map(|(k, v)| v.obj().map(|o| (o.node, k.to_string()))),
        );

        for (i, output) in self.outputs.iter().enumerate() {
            if let Some(obj) = output.obj() {
                if let Some(name) = root_names.get(&obj.node) {
                    root_names.insert(obj.node, format!("output_{}, {}", i, name));
                } else {
                    root_names.insert(obj.node, format!("output_{}", i));
                }
            }
        }

        Dot::from_nodes_with_config(
            &self.graphs_map,
            self.values
                .values()
                .filter_map(|v| v.obj().map(|o| (o.graph_id, o.node)))
                .chain(
                    self.outputs
                        .iter()
                        .filter_map(|v| v.obj().map(|o| (o.graph_id, o.node))),
                )
                .collect(),
            DotConfig {
                override_root_name: root_names,
                ..Default::default()
            },
        )
    }

    pub fn mermaid_display(&self) -> Mermaid<'_> {
        let mut root_names = HashMap::from_iter(
            self.values
                .iter()
                .filter_map(|(k, v)| v.obj().map(|o| (o.node, k.to_string()))),
        );

        for (i, output) in self.outputs.iter().enumerate() {
            if let Some(obj) = output.obj() {
                if let Some(name) = root_names.get(&obj.node) {
                    root_names.insert(obj.node, format!("output_{}, {}", i, name));
                } else {
                    root_names.insert(obj.node, format!("output_{}", i));
                }
            }
        }

        Mermaid::from_nodes_with_config(
            &self.graphs_map,
            self.values
                .values()
                .filter_map(|v| v.obj().map(|o| (o.graph_id, o.node)))
                .chain(
                    self.outputs
                        .iter()
                        .filter_map(|v| v.obj().map(|o| (o.graph_id, o.node))),
                )
                .collect(),
            MermaidConfig {
                override_root_name: root_names,
                ..Default::default()
            },
        )
    }
}

#[derive(serde::Serialize)]
struct ContextJsonDisplay {
    values: HashMap<String, String>,
    outputs: HashMap<String, String>,
    #[serde(rename(serialize = "ctx.mermaid"))]
    ctx_mermaid: String,
    graphs: String,
    hashes: String,
}

impl Context {
    pub fn json_display(&self) -> impl Display + '_ {
        self.get_json_wrapper()
    }

    fn get_json_wrapper(&self) -> ContextJsonDisplay {
        ContextJsonDisplay {
            values: self
                .variables()
                .map(|(k, v)| {
                    (
                        format!("{}.mermaid", k),
                        v.mermaid_display_with_name(&self.graphs_map, &format!("{}", k))
                            .to_string(),
                    )
                })
                .collect(),
            outputs: self
                .outputs()
                .enumerate()
                .map(|(i, v)| {
                    (
                        format!("output_{}.mermaid", i),
                        v.mermaid_display_with_name(&self.graphs_map, &format!("output_{}", i))
                            .to_string(),
                    )
                })
                .collect(),
            ctx_mermaid: self.mermaid_display().to_string(),
            graphs: format!("{:?}", self.graphs_map.keys().collect_vec()),
            hashes: format!("{:?}", self.hashes),
        }
    }
}

impl Display for ContextJsonDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = serde_json::to_string_pretty(self).unwrap();
        write!(f, "{}", value)
    }
}

// impl Default for Context {
//     fn default() -> Self {
//         Context::with_values(Default::default())
//     }
// }

// #[macro_export]
// macro_rules! trace_context {
//     (target: $target:expr, $ctx:expr, $message:expr, $($arg:tt)*) => (
//         if tracing::enabled!(tracing::Level::TRACE) {
//             tracing::trace!(target: $target, {
//                 .json = %$ctx.json_display()
//             },  $message, $($arg)*);
//         }
//     );
//     (target: $target:expr, $ctx:expr, $message:expr) => (
//         if tracing::enabled!(tracing::Level::TRACE) {
//             tracing::trace!(target: $target, {
//                 .json = %$ctx.json_display()
//             },  $message);
//         }
//     );
// }

#[derive(Clone, Debug)]
pub struct ContextArray {
    inner: Vec<Box<Context>>,
}

impl ContextArray {
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut Box<Context>> {
        self.inner.get_mut(index)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Box<Context>> {
        self.inner.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Box<Context>> {
        self.inner.iter_mut()
    }

    pub fn subset(&self, other: &ContextArray) -> bool {
        self.inner
            .iter()
            .zip_eq(other.inner.iter())
            .all(|(self_ctx, other_ctx)| self_ctx.subset(other_ctx))
    }

    pub fn get_partial_context<'a, I>(&self, required_variables: I) -> Option<Self>
    where
        I: IntoIterator<Item = &'a VariableName> + Copy,
    {
        let mut ctxs = Vec::<Box<Context>>::with_capacity(self.len());

        for ctx in self.iter() {
            ctxs.push(ctx.get_partial_context(required_variables)?);
        }

        Some(Self {
            inner: ctxs,
        })
    }

    pub fn verify_contexts_vector(&self) -> bool {
        true
    }

    pub fn get_variables(&self) -> Arc<BTreeMap<VariableName, Variable>> {
        let first = self.inner.first().unwrap();
        let mut all_vars = BTreeMap::<VariableName, Variable>::default();
        for (name, val) in first.values.iter() {
            all_vars.insert(
                name.clone(),
                Variable {
                    name: name.clone(),
                    value_type: val.val_type(),
                    immutable: false,
                },
            );
        }

        Arc::new(all_vars)
    }

    pub fn extend_graphs_map(&mut self, other: &ContextArray) {
        for (cur, other) in self.inner.iter_mut().zip(other.iter()) {
            cur.extend_graphs_map(other);
        }
    }

    pub fn get(&self, index: usize) -> Option<&Box<Context>> {
        self.inner.get(index)
    }
}

#[macro_export]
macro_rules! trace_context_array {
    (target: $target:expr, $ctx_array:expr, $message:expr) => (
        if tracing::enabled!(tracing::Level::TRACE) {
            tracing::trace!(target: $target, {
                size = %$ctx_array.len(),
                contexts.json = %$ctx_array.json_display()
            },  $message);
        }
    );
    (target: $target:expr, $ctx_array:expr, $message:expr, $($arg:tt)*) => (
        if tracing::enabled!(tracing::Level::TRACE) {
            tracing::trace!(target: $target, {
                depth = %$ctx_array.depth,
                size = %$ctx_array.len(),
                contexts.json = %$ctx_array.json_display()
            },  $message, $($arg)*);
        }
    );
}

impl Default for ContextArray {
    fn default() -> Self {
        Self {
            inner: vec![Context::with_values(
                Default::default(),
                GraphsMap::default().into(),
                GraphIdGenerator::default().into(),
            )],
        }
    }
}

impl From<Vec<Box<Context>>> for ContextArray {
    fn from(value: Vec<Box<Context>>) -> Self {
        assert!(!value.is_empty(), "Must have at least one example");
        let obj = ContextArray {
            inner: value,
        };
        debug_assert!(obj.verify_contexts_vector());
        obj
    }
}

impl Index<usize> for ContextArray {
    type Output = Context;

    fn index(&self, index: usize) -> &Self::Output {
        &self.inner[index]
    }
}

impl Hash for ContextArray {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.hash(state);
    }
}

impl Eq for ContextArray {}

impl PartialEq for ContextArray {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Display for ContextArray {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{{")?;
        writeln!(f, "contexts: [")?;
        for ctx in &self.inner {
            writeln!(f, "{}", ctx)?;
        }
        writeln!(f, "]")?;
        writeln!(f, "}}")?;

        Ok(())
    }
}

impl ContextArray {
    pub fn json_display(&self) -> impl Display + '_ {
        self.json_display_struct()
    }

    pub fn json_display_struct(&self) -> ContextArrayJsonDisplay {
        ContextArrayJsonDisplay {
            contexts: self
                .inner
                .iter()
                .map(|ctx| ctx.get_json_wrapper())
                .collect(),
        }
    }
}

#[derive(serde::Serialize)]
pub struct ContextArrayJsonDisplay {
    contexts: Vec<ContextJsonDisplay>,
}

impl Display for ContextArrayJsonDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = serde_json::to_string_pretty(&self.contexts).unwrap();
        write!(f, "{}", value)
    }
}
