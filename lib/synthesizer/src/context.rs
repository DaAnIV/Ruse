use std::{collections::HashMap, hash::{DefaultHasher, Hash, Hasher}, sync::Arc};

use crate::value::{Location, Value};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Context {
    hash: u64,
    values: Arc<HashMap<Arc<String>, Value>>,
}

impl Context {
    pub fn new(values: HashMap<Arc<String>, Value>) -> Self {
        let mut hasher = DefaultHasher::new();
        for (k, v) in &values {
            k.hash(&mut hasher);
            v.hash(&mut hasher);
        }
        Self {
            values: values.into(),
            hash: hasher.finish()
        }
    }

    pub fn update_value(&self, loc: &Location, new_val: Value) -> Context {
        if loc.is_temp() {
            return self.clone();
        }

        let mut new_values = (*self.values).clone();

        match &loc {
            Location::Var(l) => {
                new_values.insert(l.var.clone(), new_val);
            }
            Location::ObjectField(l) => {
                let obj_val = new_values[&l.var].obj().unwrap();
                let mut new_graph = match new_val {
                    Value::Primitive(p) => {
                        let mut new_graph = (*obj_val.graph).clone();
                        new_graph.set_field(l.node, l.field.clone(), p);
                        new_graph
                    }
                    Value::Object(_o) => todo!(),
                };
                new_graph
                    .generate_serialized_data()
                    .expect("Failed to serialize new graph");
            }
            Location::Temp => unreachable!(),
        };

        Context::new(new_values)
    }

    pub fn get_var_value(&self, var: &Arc<String>) -> Value {
        self.values[var].clone()
    }

    pub fn get_keys<'a>(&'a self) -> Box<dyn Iterator<Item = &Arc<String>>+'a> {
        Box::new(self.values.keys())
    }
}

impl Hash for Context {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.hash);
    }
}