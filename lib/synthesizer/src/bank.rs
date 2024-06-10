use std::sync::Arc;

use dashmap::{DashMap, DashSet};

use crate::{
    prog::SubProgram,
    value::{LocValue, ValueType},
};

pub type ValueArray = Arc<Vec<LocValue>>;

// The bank is hierarchical
// iteration -> out_type -> sub_prog

pub(crate) type ValueMap = DashSet<Arc<SubProgram>>;

#[derive(Default)]
pub struct TypeMap(pub(crate) DashMap<ValueType, Arc<ValueMap>>);

impl TypeMap {
    pub fn insert_program(&self, p: Arc<SubProgram>) -> bool {
        let value_set = match self.0.get_mut(&p.out_type()) {
            Some(m) => m,
            None => {
                let m = ValueMap::new();
                self.0.insert(p.out_type().clone(), m.into());
                unsafe { self.0.get_mut(&p.out_type()).unwrap_unchecked() }
            }
        };

        value_set.insert(p)
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
    pub fn insert(&mut self, ctx_map: Arc<TypeMap>) {
        self.bank.push(ctx_map);
    }

    #[inline]
    pub fn iteration_count(&self) -> usize {
        self.bank.len()
    }
}
