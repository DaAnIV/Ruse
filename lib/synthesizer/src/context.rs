use crate::value::{LocValue, Location, Value, VarLoc};
use ruse_object_graph::{CachedString, ObjectGraph};
use std::{
    collections::HashMap, fmt::Display, hash::{DefaultHasher, Hash, Hasher}, sync::Arc
};

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
            number_of_changes: 0
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
        let arc_values = values.into();

        self.hash = new_hash;
        self.values = arc_values;
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

    pub fn update_value(&mut self, new_val: &Value, loc: &Location) {
        match loc {
            Location::Var(l) => {
                debug_assert!(self.values.contains_key(&l.var));
                assert!(new_val.is_primitive());
                assert!(self.values[&l.var].val_type() == new_val.val_type());

                let mut new_values = (*self.values).clone();
                new_values.insert(l.var.clone(), new_val.clone());

                self.set_values(new_values);
            }
            Location::ObjectField(l) => {
                debug_assert!(self.values.contains_key(&l.var));

                let obj_val = self.values[&l.var].obj().unwrap();
                let mut new_graph = match new_val {
                    Value::Primitive(p) => {
                        assert!(obj_val.graph.get_field(l.node, &l.field).is_some());
                        let mut new_graph = (*obj_val.graph).clone();
                        new_graph.set_field(l.node, l.field.clone(), p.clone());
                        new_graph
                    }
                    Value::Object(o) => {
                        let (mut new_graph, nodes_map) =
                            ObjectGraph::union(&[obj_val.graph.clone(), o.graph.clone()]);
                        new_graph.add_edge(
                            nodes_map[&(Arc::as_ptr(&obj_val.graph) as u64, l.node)],
                            nodes_map[&(Arc::as_ptr(&o.graph) as u64, o.node)],
                            &l.field,
                        );
                        new_graph
                    }
                };
                new_graph
                    .generate_serialized_data()
                    .expect("Failed to serialize new graph");

                let graph_ptr = Arc::new(new_graph);

                let mut new_values = (*self.values).clone();
                for (root_name, root_idx) in graph_ptr.roots() {
                    if let Some(root_var) = new_values.get_mut(root_name) {
                        let root_obj_val = root_var.mut_obj().unwrap();
                        root_obj_val.node = *root_idx;
                        root_obj_val.graph = graph_ptr.clone();
                    }
                }

                self.set_values(new_values);
            }
            Location::Temp => return,
        }
    }

    pub fn get_keys<'a>(&'a self) -> Box<dyn Iterator<Item = &CachedString> + 'a> {
        Box::new(self.values.keys())
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
