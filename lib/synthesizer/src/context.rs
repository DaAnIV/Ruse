use crate::location::{LocValue, Location, VarLoc};
use itertools::Itertools;
use ruse_object_graph::{
    graph_map_value::*,
    value::{ObjectValue, Value, ValueType},
    *,
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt::Display,
    hash::{DefaultHasher, Hash, Hasher},
    ops::Index,
    slice::Iter,
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
    pub fn get_id_for_node(&self) -> NodeIndex {
        NodeIndex(self.node_id.fetch_add(1, atomic::Ordering::Relaxed))
    }

    pub fn get_id_for_graph(&self) -> GraphIndex {
        self.graph_id.fetch_add(1, atomic::Ordering::Relaxed)
    }
}

impl Default for GraphIdGenerator {
    fn default() -> Self {
        Self {
            node_id: 0.into(),
            graph_id: 0.into(),
        }
    }
}

pub struct SynthesizerContext {
    all_variables: Arc<HashMap<VariableName, Variable>>,
    pub cache: Arc<Cache>,
    pub start_context: ContextArray,
}

impl SynthesizerContext {
    pub fn from_context_array(context_array: ContextArray, cache: Arc<Cache>) -> Self {
        Self {
            all_variables: context_array.get_variables(),
            cache,
            start_context: context_array,
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

#[derive(Clone, Debug)]
pub struct Context {
    hash: u64,
    pub(crate) values: Arc<HashMap<VariableName, Value>>,
    pub graphs_map: Arc<GraphsMap>,
    pub graph_id_gen: Arc<GraphIdGenerator>,
}

impl Context {
    pub fn with_values(
        values: HashMap<VariableName, Value>,
        graphs_map: Arc<GraphsMap>,
        graph_id_gen: Arc<GraphIdGenerator>,
    ) -> Self {
        let mut instance = Self {
            hash: 0,
            values: values.into(),
            graphs_map,
            graph_id_gen,
        };

        instance.update_hash();

        instance
    }

    // pub fn graph(&self) -> &ObjectGraph {
    //     &self.graph
    // }

    fn get_hash_for_values(values: &HashMap<VariableName, Value>, graphs_map: &GraphsMap) -> u64 {
        let mut hasher = DefaultHasher::new();
        for (k, v) in values {
            k.hash(&mut hasher);
            match v {
                Value::Primitive(primitive_value) => primitive_value.hash(&mut hasher),
                Value::Object(object_value) => object_value.calculate_hash(&mut hasher, graphs_map),
            }
        }
        hasher.finish()
    }

    fn update_hash(&mut self) {
        let new_hash = Self::get_hash_for_values(&self.values, &self.graphs_map);
        self.hash = new_hash;
    }

    fn set_values(&mut self, values: HashMap<VariableName, Value>) {
        self.values = values.into();
        self.update_hash();
    }

    pub fn temp_value(&self, val: Value) -> LocValue {
        LocValue {
            val,
            loc: Location::Temp,
        }
    }

    pub fn get_var_loc_value(&self, var: &VariableName) -> Option<LocValue> {
        let val = match self.values.get(var) {
            None => return None,
            Some(v) => v.clone(),
        };
        Some(LocValue {
            val,
            loc: Location::Var(VarLoc { var: var.clone() }),
        })
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
                assert!(var.value_type == new_val.val_type(&self.graphs_map));

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
        match value {
            Value::Primitive(p) => {
                assert!(new_graph.get_field(&node, &field_name).is_some());
                new_graph.set_field(&node, field_name.clone(), p.clone());
            }
            Value::Object(o) => {
                if graph == o.graph_id {
                    new_graph.set_edge(&node, o.node, field_name);
                } else {
                    new_graph.set_chain_edge(&node, o.graph_id, o.node, field_name);
                }
            }
        };
        self.update_graph(new_graph.into());
        self.update_hash();
        true
    }

    pub fn create_graph(&self) -> ObjectGraph {
        ObjectGraph::new(self.graph_id_gen.get_id_for_graph())
    }

    pub fn update_graph(&mut self, new_graph: Arc<ObjectGraph>) {
        let mut new_map = self.graphs_map.as_ref().clone();
        new_map.insert_graph(new_graph);
        self.graphs_map = new_map.into();
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
        let obj_type = ValueType::array_obj_cached_string(elem_type, &syn_ctx.cache);
        let map = values
            .into_iter()
            .enumerate()
            .map(|(i, v)| (syn_ctx.cached_string(&i.to_string()), v));
        self.create_output_simple_object_from_map(obj_type, map, syn_ctx)
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
        let obj_type = ValueType::array_obj_cached_string(elem_type, &syn_ctx.cache);
        let values_map = values
            .into_iter()
            .enumerate()
            .map(|(i, v)| (syn_ctx.cached_string(&i.to_string()), v));
        self.create_output_object_from_map(obj_type, values_map, syn_ctx)
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
        let mut fields = FieldsMap::new();

        for (key, val) in map {
            fields.insert(key, val.into());
        }

        self.create_output_simple_object(obj_type, fields, syn_ctx)
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
        let mut fields = FieldsMap::new();
        let mut obj_keys = vec![];

        for (key, val) in map {
            Self::visit_value_in_map(val, &mut fields, key, &mut obj_keys);
        }

        self.create_output_object(obj_type, fields, obj_keys, syn_ctx)
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

    fn create_output_object(
        &mut self,
        obj_type: ObjectType,
        fields: FieldsMap,
        obj_keys: Vec<(FieldName, ObjectValue)>,
        syn_ctx: &SynthesizerContext,
    ) -> ObjectValue {
        let out_graph_id = self.graph_id_gen.get_id_for_graph();
        let out_node_id = self.graph_id_gen.get_id_for_node();

        let mut out_graph = ObjectGraph::new(out_graph_id);
        let node = out_graph.add_node(out_node_id, obj_type, fields);
        for (key, neig) in obj_keys {
            node.insert_chain_edge(key, neig.graph_id, neig.node);
        }
        out_graph.set_as_root(syn_ctx.output_root_name().clone(), out_node_id);

        self.update_graph(out_graph.into());
        ObjectValue {
            graph_id: out_graph_id,
            node: out_node_id,
        }
    }

    pub fn create_output_simple_object(
        &mut self,
        obj_type: ObjectType,
        fields: FieldsMap,
        syn_ctx: &SynthesizerContext,
    ) -> ObjectValue {
        self.create_output_object(obj_type, fields.into(), vec![], syn_ctx)
    }
}

impl Context {
    pub fn reachable_nodes(&self) -> HashSet<NodeIndex> {
        let mut nodes = HashSet::new();

        for var in self.variable_values() {
            if let Value::Object(obj) = var {
                self.add_nodes(obj.graph_id, obj.node, &mut nodes);
            }
        }

        nodes
    }

    fn add_nodes(&self, graph_id: GraphIndex, node_id: NodeIndex, seen: &mut HashSet<NodeIndex>) {
        let mut q = VecDeque::new();
        q.push_back((graph_id, node_id));
        while let Some((cur_graph_id, cur_node_id)) = q.pop_back() {
            if seen.contains(&cur_node_id) {
                continue;
            }

            seen.insert(cur_node_id);
            let graph = &self.graphs_map[cur_graph_id];
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

impl Default for Context {
    fn default() -> Self {
        Self::with_values(
            Default::default(),
            GraphsMap::default().into(),
            GraphIdGenerator::default().into(),
        )
    }
}

impl Hash for Context {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.hash);
    }
}

impl Eq for Context {}

impl PartialEq for Context {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
            && self.values.len() == other.values.len()
            && self.values.iter().zip(other.values.iter()).all(
                |((self_key, self_value), (other_key, other_value))| {
                    self_key == other_key
                        && self_value.wrap(&self.graphs_map) == other_value.wrap(&other.graphs_map)
                },
            )
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
        write!(f, "Graphs: {:?}", self.graphs_map.keys().collect_vec())?;
        
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
    inner: Vec<Context>,
}

impl ContextArray {
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn iter(&self) -> Iter<Context> {
        self.inner.iter()
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut Context> {
        self.inner.get_mut(index)
    }

    pub(crate) fn subset(&self, other: &ContextArray) -> bool {
        self.inner
            .iter()
            .zip_eq(other.inner.iter())
            .all(|(self_ctx, other_ctx)| self_ctx.subset(other_ctx))
    }

    pub(crate) fn get_partial_context<'a, I>(&self, required_variables: I) -> Option<Self>
    where
        I: IntoIterator<Item = &'a CachedString> + Copy,
    {
        let mut ctxs = Vec::<Context>::with_capacity(self.len());

        for ctx in self.iter() {
            let mut values = HashMap::new();
            for var in required_variables {
                let var_value = ctx.values.get(var)?.clone();
                values.insert((*var).clone(), var_value);
            }
            ctxs.push(Context::with_values(
                values,
                ctx.graphs_map.clone(),
                ctx.graph_id_gen.clone(),
            ));
        }

        Some(Self {
            inner: ctxs,
            depth: self.depth,
        })
    }

    pub fn verify_contexts_vector(&self) -> bool {
        true
    }

    pub fn get_variables(&self) -> Arc<HashMap<Arc<String>, Variable>> {
        let first = self.inner.first().unwrap();
        let mut all_vars = HashMap::<CachedString, Variable>::with_capacity(first.values.len());
        for (name, val) in first.values.iter() {
            all_vars.insert(
                name.clone(),
                Variable {
                    name: name.clone(),
                    value_type: val.val_type(&first.graphs_map),
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
            inner: vec![Context::default()],
        }
    }
}

impl From<Vec<Context>> for ContextArray {
    fn from(value: Vec<Context>) -> Self {
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
