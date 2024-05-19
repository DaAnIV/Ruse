use std::sync::Arc;

use dashmap::{DashMap, DashSet};

use crate::{
    context::Context,
    opcode::ExprAst,
    prog::SubProgram,
    value::{LocValue, ValueType},
};

pub type ContextArray = Arc<Vec<Context>>;
pub type ValueArray = Arc<Vec<LocValue>>;

pub type ProgIterator<'a, T> =
    Box<dyn Iterator<Item = &'a Arc<SubProgram<T>>> + 'a>;

// The bank is hierarchical
// iteration -> [pre_context] -> out_type -> sub_prog

pub type ValueSet<T> = DashSet<Arc<SubProgram<T>>>;

#[derive(Default)]
pub struct TypeMap<T: ExprAst>(pub(crate) DashMap<ValueType, ValueSet<T>>);

impl<T: ExprAst> TypeMap<T> {
    pub fn insert_program(&self, p: Arc<SubProgram<T>>) -> bool {
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

    pub fn contains(&self, p: &Arc<SubProgram<T>>) -> bool {
        match self.0.get(&p.out_type()) {
            None => false,
            Some(values) => values.contains(p),
        }
    }
}

#[derive(Default)]
pub struct ContextMap<T: ExprAst>(
    pub(crate) DashMap<ContextArray, TypeMap<T>>,
);
impl<T: ExprAst> ContextMap<T> {
    pub fn get(
        &self,
        ctx: &ContextArray,
    ) -> Option<dashmap::mapref::one::Ref<'_, ContextArray, TypeMap<T>>> {
        self.0.get(ctx)
    }

    pub fn insert_program(&self, p: Arc<SubProgram<T>>) -> bool {
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

    pub fn insert(&self, ctx: &ContextArray, map: TypeMap<T>) {
        self.0.insert(ctx.clone(), map);
    }

    pub fn contains(&self, p: &Arc<SubProgram<T>>) -> bool {
        match self.get(p.pre_ctx()) {
            None => false,
            Some(type_map) => type_map.contains(p),
        }
    }
}

#[derive(Default)]
pub struct ProgBank<T: ExprAst> {
    pub(crate) bank: Vec<Arc<ContextMap<T>>>,
}

impl<T: ExprAst> ProgBank<T> {
    pub fn output_exists(&self, p: &Arc<SubProgram<T>>) -> bool {
        (&self.bank).into_iter().any(|ctx_map| ctx_map.contains(p))
    }

    pub fn get(&self, iteration: usize) -> Option<&Arc<ContextMap<T>>> {
        self.bank.get(iteration)
    }

    #[inline]
    pub fn insert(&mut self, ctx_map: Arc<ContextMap<T>>) {
        self.bank.push(ctx_map);
    }

    #[inline]
    pub fn iteration_count(&self) -> usize {
        self.bank.len()
    }
}
