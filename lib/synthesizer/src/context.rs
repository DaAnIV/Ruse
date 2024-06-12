use crate::value::{LocValue, Location, Value, ValueType, VarLoc};
use ruse_object_graph::{str_cached, Cache, CachedString, NodeIndex, ObjectGraph};
use std::{
    collections::HashMap,
    fmt::Display,
    hash::{DefaultHasher, Hash, Hasher},
    sync::Arc,
};

#[derive(PartialEq, Eq, Debug, Clone, Hash)]
pub struct Variable {
    pub name: CachedString,
    pub value_type: ValueType,
    pub immutable: bool,
}

pub struct SynthesizerContext {
    all_variables: Arc<HashMap<CachedString, Variable>>,
    pub cache: Arc<Cache>,
    pub start_context: ContextArray,
}

impl SynthesizerContext {
    pub fn from_context_array(context_array: ContextArray, cache: Arc<Cache>) -> Self {
        Self {
            all_variables: context_array.get_variables(),
            cache: cache,
            start_context: context_array,
        }
    }
    pub fn get_variable(&self, name: &CachedString) -> Option<&Variable> {
        self.all_variables.get(name)
    }

    pub fn set_immutable(&mut self, var: &CachedString) {
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
}

#[derive(Clone, Debug)]
pub struct Context {
    hash: u64,
    values: Arc<HashMap<CachedString, Value>>,
    number_of_changes: usize,
}

impl Context {
    pub fn with_values(values: HashMap<CachedString, Value>) -> Self {
        Self {
            hash: Self::get_hash_for_values(&values),
            values: values.into(),
            number_of_changes: 0,
        }
    }

    pub fn number_of_changes(&self) -> usize {
        self.number_of_changes
    }

    fn get_hash_for_values(values: &HashMap<CachedString, Value>) -> u64 {
        let mut hasher = DefaultHasher::new();
        for (k, v) in values {
            k.hash(&mut hasher);
            v.hash(&mut hasher);
        }
        hasher.finish()
    }

    fn set_values(&mut self, values: HashMap<CachedString, Value>) {
        let new_hash = Self::get_hash_for_values(&values);

        self.hash = new_hash;
        self.values = values.into();
        self.number_of_changes += 1;
    }

    pub fn temp_value(&self, val: Value) -> LocValue {
        LocValue {
            val: val,
            loc: Location::Temp,
        }
    }

    pub fn get_var_loc_value(&self, var: &CachedString) -> LocValue {
        LocValue {
            val: self.values[var].clone(),
            loc: Location::Var(VarLoc { var: var.clone() }),
        }
    }

    pub fn get_loc_value(&self, val: Value, loc: Location) -> LocValue {
        return LocValue { val: val, loc: loc };
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
                assert!(new_val.is_primitive());
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
                let var = syn_ctx.all_variables.get(&l.var).unwrap();

                let obj_val = self.values[&l.var].obj().unwrap();
                let (new_graph, node) = self.set_field(&obj_val.graph, l.node, &l.field, new_val);
                l.node = node;

                let mut new_values = (*self.values).clone();
                for (root_name, root_idx) in new_graph.roots() {
                    if let Some(root_var) = new_values.get_mut(root_name) {
                        if var.immutable { // This is not exactly true
                            return false;
                        }
                        let root_obj_val = root_var.mut_obj().unwrap();
                        root_obj_val.node = *root_idx;
                        root_obj_val.graph = new_graph.clone();
                    }
                }

                self.set_values(new_values);

                true
            }
            Location::Temp => return true,
        }
    }

    pub fn get_keys<'a>(&'a self) -> Box<dyn Iterator<Item = &CachedString> + 'a> {
        Box::new(self.values.keys())
    }

    pub fn set_field(
        &self,
        graph: &Arc<ObjectGraph>,
        node: NodeIndex,
        field_name: &CachedString,
        value: &Value,
    ) -> (Arc<ObjectGraph>, NodeIndex) {
        let (mut new_graph, node) = match value {
            Value::Primitive(p) => {
                assert!(graph.get_field(node, &field_name).is_some());
                let mut new_graph = graph.as_ref().clone();
                new_graph.set_field(node, field_name.clone(), p.clone());
                (new_graph, node)
            }
            Value::Object(o) => {
                let (mut new_graph, nodes_map) =
                    ObjectGraph::union(&[graph.clone(), o.graph.clone()]);
                new_graph.add_edge(
                    nodes_map[&(Arc::as_ptr(&graph) as u64, node)],
                    nodes_map[&(Arc::as_ptr(&o.graph) as u64, o.node)],
                    field_name,
                );
                (new_graph, nodes_map[&(Arc::as_ptr(&graph) as u64, node)])
            }
        };
        new_graph.generate_serialized_data();

        (Arc::new(new_graph), node)
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
        self.hash == other.hash && self.values == other.values
    }
}

impl Display for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut iter = self.values.iter();
        let mut value = iter.next();
        while let Some((k, v)) = value {
            write!(f, "{} -> {}", k, v).expect("Failed to write");
            value = iter.next();
            if value.is_some() {
                write!(f, ", ").expect("Failed to write");
            }
        }
        Ok(())
    }
}

impl Default for Context {
    fn default() -> Self {
        Context::with_values(Default::default())
    }
}

pub type ContextArray = Arc<Vec<Context>>;

#[macro_export]
macro_rules! context_array {
    ($($x:expr),+ $(,)?) => {
        Arc::new(vec![$(
            $crate::context::Context::with_values($x.into()),
        )+])
    };
}
