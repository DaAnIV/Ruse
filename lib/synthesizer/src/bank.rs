use std::sync::Arc;

use dashmap::{DashMap, DashSet};

use crate::{
    context::ContextArray
    prog::SubProgram,
    value::{LocValue, ValueType},
};

pub type ValueArray = Arc<Vec<LocValue>>;

// The bank is hierarchical
// iteration -> [pre_context] -> out_type -> sub_prog

pub type ValueSet = DashSet<Arc<SubProgram>>;

#[derive(Default)]
pub struct TypeMap(pub(crate) DashMap<ValueType, ValueSet>);

impl TypeMap {
    pub fn insert_program(&self, p: Arc<SubProgram>) -> bool {
        let value_set = match self.0.get_mut(&p.out_type()) {
            Some(m) => m,
            None => {
                let m = ValueSet::new();
                self.0.insert(p.out_type().clone(), m);
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
pub struct ContextMap(
    pub(crate) DashMap<ContextArray, TypeMap>,
);
impl ContextMap {
    pub fn get(
        &self,
        ctx: &ContextArray,
    ) -> Option<dashmap::mapref::one::Ref<'_, ContextArray, TypeMap>> {
        self.0.get(ctx)
    }

    pub fn insert_program(&self, p: Arc<SubProgram>) -> bool {
        let ctx = p.pre_ctx();

        let type_map = match self.0.get_mut(ctx) {
            Some(m) => m,
            None => {
                let m = Default::default();
                self.0.insert(ctx.clone(), m);
                unsafe { self.0.get_mut(ctx).unwrap_unchecked() }
            }
        };

        type_map.insert_program(p)
    }

    pub fn insert(&self, ctx: &ContextArray, map: TypeMap) {
        self.0.insert(ctx.clone(), map);
    }

    pub fn contains(&self, p: &Arc<SubProgram>) -> bool {
        match self.get(p.pre_ctx()) {
            None => false,
            Some(type_map) => type_map.contains(p),
        }
    }
}

#[derive(Default)]
pub struct ProgBank {
    pub(crate) bank: Vec<Arc<ContextMap>>,
}

impl ProgBank {
    pub fn output_exists(&self, p: &Arc<SubProgram>) -> bool {
        (&self.bank).into_iter().any(|ctx_map| ctx_map.contains(p))
    }

    pub fn get(&self, iteration: usize) -> Option<&Arc<ContextMap>> {
        self.bank.get(iteration)
    }

    #[inline]
    pub fn insert(&mut self, ctx_map: Arc<ContextMap>) {
        self.bank.push(ctx_map);
    }

    #[inline]
    pub fn iteration_count(&self) -> usize {
        self.bank.len()
    }
}
