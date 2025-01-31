use crate::location::{LocValue, Location, VarLoc};
use downcast_rs::{impl_downcast, DowncastSync};
use graph_equality::equal_graphs_by_nodes;
use itertools::Itertools;
use ruse_object_graph::{
    graph_map_value::*,
    value::{ObjectValue, Value, ValueType},
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

pub type VariableName = CachedString;

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
            graph_id: graph_id.into(),
        }
    }

    pub fn get_id_for_node(&self) -> NodeIndex {
        NodeIndex(self.node_id.fetch_add(1, atomic::Ordering::Relaxed))
    }

    pub fn get_id_for_graph(&self) -> GraphIndex {
        self.graph_id.fetch_add(1, atomic::Ordering::Relaxed)
    }

    pub fn max_node_id(&self) -> NodeIndex {
        NodeIndex(self.node_id.load(atomic::Ordering::Relaxed))
    }

    pub fn max_graph_id(&self) -> GraphIndex {
        self.graph_id.load(atomic::Ordering::Relaxed)
    }
}

impl Default for GraphIdGenerator {
    fn default() -> Self {
        Self::with_initial_values(0.into(), 0)
    }
}

pub trait SynthesizerContextData: DowncastSync {}
impl_downcast!(sync SynthesizerContextData);

pub struct EmptySynthesizerData {}
impl SynthesizerContextData for EmptySynthesizerData {}

pub struct SynthesizerContext {
    all_variables: Arc<BTreeMap<VariableName, Variable>>,
    pub cache: Arc<Cache>,
    pub start_context: ContextArray,
    pub data: Box<dyn SynthesizerContextData>,
}

impl SynthesizerContext {
    pub fn from_context_array(context_array: ContextArray, cache: Arc<Cache>) -> Self {
        Self::from_context_array_with_data(context_array, Box::new(EmptySynthesizerData {}), cache)
    }
    pub fn from_context_array_with_data(
        context_array: ContextArray,
        data: Box<dyn SynthesizerContextData>,
        cache: Arc<Cache>,
    ) -> Self {
        Self {
            all_variables: context_array.get_variables(),
            cache,
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

    pub fn cached_string(&self, string: &str) -> CachedString {
        str_cached!(self.cache; string)
    }

    pub fn variables_count(&self) -> usize {
        self.all_variables.len()
    }

    pub fn output_root_name(&self) -> &VariableName {
        self.cache.output_root_name()
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
    type Item = (FieldName, Value);

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
    hashes: Arc<ValuesHashMap>,
    pub(crate) values: Arc<ValuesMap>,
    pub graphs_map: Arc<GraphsMap>,
    pub graph_id_gen: Arc<GraphIdGenerator>,
}

impl Context {
    pub fn with_values(
        values: ValuesMap,
        graphs_map: Arc<GraphsMap>,
        graph_id_gen: Arc<GraphIdGenerator>,
    ) -> Box<Self> {
        let mut instance = Box::new(Self {
            hashes: Default::default(),
            values: values.into(),
            graphs_map,
            graph_id_gen,
        });

        instance.update_hash();

        instance
    }

    fn update_hash(&mut self) {
        self.hashes = self.values.get_hashes(&self.graphs_map).into();
    }

    fn set_values(&mut self, values: ValuesMap) {
        self.values = values.into();
        self.update_hash();
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
            Location::ObjectField(l) => self.set_field(l.graph, l.node, l.field.clone(), new_val),
            Location::Temp => true,
        }
    }

    pub fn set_field(
        &mut self,
        graph: GraphIndex,
        node: NodeIndex,
        field_name: FieldName,
        value: &Value,
    ) -> bool {
        let mut new_graph = self.graphs_map[graph].as_ref().clone();
        new_graph.set_field(&node, field_name.clone(), value);
        self.update_graph(new_graph.into());
        self.update_hash();
        true
    }

    pub fn delete_field(
        &mut self,
        graph: GraphIndex,
        node: NodeIndex,
        field_name: &FieldName,
    ) -> Option<Value> {
        let mut new_graph = self.graphs_map[graph].as_ref().clone();
        let result =
            if let Some(primitive_field) = new_graph.delete_primitive_field(&node, field_name) {
                Value::Primitive(primitive_field.value)
            } else {
                let neig = new_graph.get_neighbor(&node, field_name)?;
                let obj_val = match neig {
                    EdgeEndPoint::Internal(neig_node) => ObjectValue {
                        obj_type: new_graph.obj_type(neig_node).unwrap().clone(),
                        graph_id: graph,
                        node: *neig_node,
                    },
                    EdgeEndPoint::Chain(chained_graph, neig_node) => ObjectValue {
                        obj_type: self.graphs_map[chained_graph]
                            .obj_type(neig_node)
                            .unwrap()
                            .clone(),
                        graph_id: *chained_graph,
                        node: *neig_node,
                    },
                };

                new_graph.remove_edge(&node, field_name.clone());

                Value::Object(obj_val)
            };
        self.update_graph(new_graph.into());
        self.update_hash();
        Some(result)
    }

    pub fn create_graph(&self) -> ObjectGraph {
        ObjectGraph::new(self.graph_id_gen.get_id_for_graph())
    }

    pub fn create_graph_in_map(&mut self) -> &mut ObjectGraph {
        let id = self.graph_id_gen.get_id_for_graph();
        let new_map = Arc::make_mut(&mut self.graphs_map);
        new_map.insert_graph(ObjectGraph::new(id).into());
        Arc::make_mut(new_map.get_mut(&id).unwrap())
    }

    pub fn update_graph(&mut self, new_graph: Arc<ObjectGraph>) {
        let mut new_map = self.graphs_map.as_ref().clone();
        new_map.insert_graph(new_graph);
        self.graphs_map = new_map.into();
    }

    pub fn insert_if_new(&mut self, new_graph: Arc<ObjectGraph>) {
        if !self.graphs_map.contains(new_graph.id) {
            self.update_graph(new_graph);
        }
    }

    pub fn variable_count(&self) -> usize {
        self.values.len()
    }

    pub fn variables(&self) -> impl std::iter::Iterator<Item = &VariableName> {
        self.values.keys()
    }

    pub fn variable_values(&self) -> impl std::iter::Iterator<Item = &Value> {
        self.values.values()
    }

    fn extend_graphs_map(&mut self, other: &Context) {
        let mut new_map = self.graphs_map.as_ref().clone();
        new_map.extend(&other.graphs_map);
        self.graphs_map = new_map.into();
    }

    pub(crate) fn get_partial_context<'a, I>(&self, required_variables: I) -> Option<Box<Self>>
    where
        I: IntoIterator<Item = &'a CachedString> + Copy,
    {
        let mut values = ValuesMap::default();
        let mut hashes = ValuesHashMap::default();
        for var in required_variables {
            let var_value = self.values.get(var)?.clone();
            let var_hash = unsafe { *self.hashes.get(var).unwrap_unchecked() };
            values.insert((*var).clone(), var_value);
            hashes.insert((*var).clone(), var_hash);
        }

        Some(
            Self {
                hashes: hashes.into(),
                values: values.into(),
                graphs_map: self.graphs_map.clone(),
                graph_id_gen: self.graph_id_gen.clone(),
            }
            .into(),
        )
    }
}

impl Context {
    pub fn create_output_primitive_array<I>(
        &mut self,
        elem_type: &ValueType,
        values: I,
        syn_ctx: &SynthesizerContext,
    ) -> ObjectValue
    where
        I: IntoIterator,
        I::Item: Into<PrimitiveValue>,
    {
        let out_graph_id = self.graph_id_gen.get_id_for_graph();
        let out_node_id = self.graph_id_gen.get_id_for_node();

        let mut out_graph = ObjectGraph::new(out_graph_id);

        out_graph.add_primitive_array_object(out_node_id, elem_type, values, &syn_ctx.cache);

        out_graph.set_as_root(syn_ctx.output_root_name().clone(), out_node_id);

        self.update_graph(out_graph.into());
        ObjectValue {
            obj_type: ValueType::array_obj_cached_string(elem_type, &syn_ctx.cache),
            graph_id: out_graph_id,
            node: out_node_id,
        }
    }

    pub fn create_output_primitive_array_from_fields<I>(
        &mut self,
        elem_type: &ValueType,
        values: I,
        syn_ctx: &SynthesizerContext,
    ) -> ObjectValue
    where
        I: IntoIterator,
        I::Item: Into<PrimitiveField>,
    {
        let out_graph_id = self.graph_id_gen.get_id_for_graph();
        let out_node_id = self.graph_id_gen.get_id_for_node();

        let mut out_graph = ObjectGraph::new(out_graph_id);

        out_graph.add_primitive_array_object_from_fields(
            out_node_id,
            elem_type,
            values,
            &syn_ctx.cache,
        );

        out_graph.set_as_root(syn_ctx.output_root_name().clone(), out_node_id);

        self.update_graph(out_graph.into());
        ObjectValue {
            obj_type: ValueType::array_obj_cached_string(elem_type, &syn_ctx.cache),
            graph_id: out_graph_id,
            node: out_node_id,
        }
    }

    pub fn create_output_array_object<I>(
        &mut self,
        elem_type: &ValueType,
        values: I,
        syn_ctx: &SynthesizerContext,
    ) -> ObjectValue
    where
        I: IntoIterator<Item = Value>,
    {
        let out_graph_id = self.graph_id_gen.get_id_for_graph();
        let out_node_id = self.graph_id_gen.get_id_for_node();

        let mut out_graph = ObjectGraph::new(out_graph_id);

        out_graph.add_array_object(out_node_id, elem_type, values, &syn_ctx.cache);

        out_graph.set_as_root(syn_ctx.output_root_name().clone(), out_node_id);

        self.update_graph(out_graph.into());
        ObjectValue {
            obj_type: ValueType::array_obj_cached_string(elem_type, &syn_ctx.cache),
            graph_id: out_graph_id,
            node: out_node_id,
        }
    }

    pub fn create_output_primitive_set<I>(
        &mut self,
        elem_type: &ValueType,
        values: I,
        syn_ctx: &SynthesizerContext,
    ) -> ObjectValue
    where
        I: IntoIterator,
        I::Item: Into<PrimitiveValue>,
    {
        let out_graph_id = self.graph_id_gen.get_id_for_graph();
        let out_node_id = self.graph_id_gen.get_id_for_node();

        let mut out_graph = ObjectGraph::new(out_graph_id);

        out_graph.add_primitive_set_object(out_node_id, elem_type, values, &syn_ctx.cache);

        out_graph.set_as_root(syn_ctx.output_root_name().clone(), out_node_id);

        self.update_graph(out_graph.into());
        ObjectValue {
            obj_type: ValueType::set_obj_cached_string(elem_type, &syn_ctx.cache),
            graph_id: out_graph_id,
            node: out_node_id,
        }
    }

    pub fn create_output_simple_object_from_map<I, T>(
        &mut self,
        obj_type: ObjectType,
        map: I,
        syn_ctx: &SynthesizerContext,
    ) -> ObjectValue
    where
        I: IntoIterator<Item = (FieldName, T)>,
        T: Into<PrimitiveValue>,
    {
        let out_graph_id = self.graph_id_gen.get_id_for_graph();
        let out_node_id = self.graph_id_gen.get_id_for_node();

        let mut out_graph = ObjectGraph::new(out_graph_id);

        out_graph.add_simple_object_from_map(out_node_id, obj_type.clone(), map);

        out_graph.set_as_root(syn_ctx.output_root_name().clone(), out_node_id);

        self.update_graph(out_graph.into());
        ObjectValue {
            obj_type: obj_type,
            graph_id: out_graph_id,
            node: out_node_id,
        }
    }

    pub fn create_output_object_from_map<I>(
        &mut self,
        obj_type: ObjectType,
        map: I,
        syn_ctx: &SynthesizerContext,
    ) -> ObjectValue
    where
        I: IntoIterator<Item = (FieldName, Value)>,
    {
        let out_graph_id = self.graph_id_gen.get_id_for_graph();
        let out_node_id = self.graph_id_gen.get_id_for_node();

        let mut out_graph = ObjectGraph::new(out_graph_id);

        out_graph.add_object_from_map(out_node_id, obj_type.clone(), map);

        out_graph.set_as_root(syn_ctx.output_root_name().clone(), out_node_id);

        self.update_graph(out_graph.into());
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
                match neig {
                    ruse_object_graph::EdgeEndPoint::Internal(neig_node_id) => {
                        q.push_back((cur_graph_id, *neig_node_id))
                    }
                    ruse_object_graph::EdgeEndPoint::Chain(neig_graph_id, neig_node_id) => {
                        q.push_back((*neig_graph_id, *neig_node_id))
                    }
                }
            }
        }
    }
}

impl Context {
    pub fn subset(&self, other: &Self) -> bool {
        if !self.variables().all(|v| other.values.contains_key(v)) {
            return false;
        }

        let mut equal_nodes: HashMap<(GraphIndex, NodeIndex), (GraphIndex, NodeIndex)> =
            HashMap::new();

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
                    if !graph_equality::sim_walk_equal(
                        &self.graphs_map,
                        self_object_value.graph_id,
                        self_object_value.node,
                        &other.graphs_map,
                        other_object_value.graph_id,
                        other_object_value.node,
                        &mut equal_nodes,
                    ) {
                        return false;
                    }
                }
                (_, _) => {
                    return false;
                }
            }
        }

        true
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
            write!(f, "{} -> {}", k, v.wrap(&self.graphs_map))?;
            value = iter.next();
            if value.is_some() {
                write!(f, ", ")?;
            }
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

// impl Default for Context {
//     fn default() -> Self {
//         Context::with_values(Default::default())
//     }
// }

#[derive(Clone, Debug)]
pub struct ContextArray {
    pub depth: usize,
    inner: Vec<Box<Context>>,
}

impl ContextArray {
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Box<Context>> {
        self.inner.iter()
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut Box<Context>> {
        self.inner.get_mut(index)
    }

    pub(crate) fn subset(&self, other: &ContextArray) -> bool {
        self.inner
            .iter()
            .zip_eq(other.inner.iter())
            .all(|(self_ctx, other_ctx)| self_ctx.subset(other_ctx))
    }

    pub fn get_partial_context<'a, I>(&self, required_variables: I) -> Option<Self>
    where
        I: IntoIterator<Item = &'a CachedString> + Copy,
    {
        let mut ctxs = Vec::<Box<Context>>::with_capacity(self.len());

        for ctx in self.iter() {
            ctxs.push(ctx.get_partial_context(required_variables)?);
        }

        Some(Self {
            inner: ctxs,
            depth: self.depth,
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
}

impl Default for ContextArray {
    fn default() -> Self {
        Self {
            depth: 0,
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
            depth: 0,
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
        writeln!(f, "[")?;
        for ctx in &self.inner {
            writeln!(f, "{}", ctx)?;
        }
        writeln!(f, "]")?;

        Ok(())
    }
}
