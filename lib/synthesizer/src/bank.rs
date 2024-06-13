use std::sync::Arc;

use dashmap::{DashMap, DashSet, Map, SharedValue};

use crate::{
    prog::SubProgram,
    value::{LocValue, ValueType},
};

pub type ValueArray = Arc<Vec<LocValue>>;

// The bank is hierarchical
// iteration -> out_type -> sub_prog

pub(crate) type ValueMap = DashSet<Arc<SubProgram>>;

#[derive(Default, Debug)]
pub struct TypeMap(pub(crate) DashMap<ValueType, Arc<ValueMap>>);

impl TypeMap {
    pub fn insert_program(&self, p: Arc<SubProgram>) -> bool {
        let value_map = self.get_or_insert_value_map(&p.out_type());
        value_map.insert(p)
    }

    fn get_or_insert_value_map(&self, value_type: &ValueType) -> Arc<ValueMap> {
        let idx = self.0.determine_map(value_type);
        let mut shard = unsafe { self.0._yield_write_shard(idx) };
        if let Some((_, vptr)) = shard.get_key_value(value_type) {
            vptr.get().clone()
        } else {
            let m = Arc::new(ValueMap::new());
            shard.insert(value_type.clone(), SharedValue::new(m.clone()));
            m
        }
    }

    pub fn contains(&self, p: &Arc<SubProgram>) -> bool {
        match self.0.get(&p.out_type()) {
            None => false,
            Some(values) => values.contains(p),
        }
    }
}

#[derive(Default)]
pub struct ProgBank {
    pub(crate) bank: Vec<Arc<TypeMap>>,
}

impl ProgBank {
    pub fn output_exists(&self, p: &Arc<SubProgram>) -> bool {
        (&self.bank)
            .into_iter()
            .any(|type_map| type_map.contains(p))
    }

    pub fn get(&self, iteration: usize) -> Option<&Arc<TypeMap>> {
        self.bank.get(iteration)
    }

    #[inline]
    pub fn insert(&mut self, type_map: Arc<TypeMap>) {
        self.bank.push(type_map);
    }

    #[inline]
    pub fn iteration_count(&self) -> usize {
        self.bank.len()
    }

    pub fn print_all_programs(&self) {
        for (i, type_map) in self.bank.iter().enumerate() {
            println!("Iteration {}", i);
            for values in type_map.0.iter() {
                for p in values.value().iter() {
                    println!("{}", p)
                }
            }
        }
    }
}
