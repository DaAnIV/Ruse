use std::sync::Arc;

use dashmap::{DashMap, DashSet};

use crate::{
    context::Context,
    opcode::ExprAst,
    prog::SubProgram,
    value::{LocValue, ValueType},
};

pub type ContextArray<const N: usize> = Arc<[Context; N]>;
pub type ValueArray<const N: usize> = Arc<[LocValue; N]>;

pub type ProgIterator<'a, T, const N: usize> =
    Box<dyn Iterator<Item = &'a Arc<SubProgram<T, N>>> + 'a>;

// The bank is hierarchical
// iteration -> [pre_context] -> out_type -> sub_prog

pub type ValueSet<T, const N: usize> = DashSet<Arc<SubProgram<T, N>>>;

#[derive(Default)]
pub struct TypeMap<T: ExprAst, const N: usize>(pub(crate) DashMap<ValueType, ValueSet<T, N>>);

impl<T: ExprAst, const N: usize> TypeMap<T, N> {
    pub fn insert_program(&self, p: Arc<SubProgram<T, N>>) -> bool {
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

    pub fn contains(&self, p: &Arc<SubProgram<T, N>>) -> bool {
        match self.0.get(&p.out_type()) {
            None => false,
            Some(values) => values.contains(p),
        }
    }
}

#[derive(Default)]
pub struct ContextMap<T: ExprAst, const N: usize>(
    pub(crate) DashMap<ContextArray<N>, TypeMap<T, N>>,
);
impl<T: ExprAst, const N: usize> ContextMap<T, N> {
    pub fn get(
        &self,
        ctx: &ContextArray<N>,
    ) -> Option<dashmap::mapref::one::Ref<'_, ContextArray<N>, TypeMap<T, N>>> {
        self.0.get(ctx)
    }

    pub fn insert_program(&self, p: Arc<SubProgram<T, N>>) -> bool {
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

    pub fn insert(&self, ctx: &ContextArray<N>, map: TypeMap<T, N>) {
        self.0.insert(ctx.clone(), map);
    }

    pub fn contains(&self, p: &Arc<SubProgram<T, N>>) -> bool {
        match self.get(p.pre_ctx()) {
            None => false,
            Some(type_map) => type_map.contains(p),
        }
    }
}

#[derive(Default)]
pub struct ProgBank<T: ExprAst, const N: usize> {
    pub(crate) bank: Vec<Arc<ContextMap<T, N>>>,
}

impl<T: ExprAst, const N: usize> ProgBank<T, N> {
    pub fn output_exists(&self, p: &Arc<SubProgram<T, N>>) -> bool {
        (&self.bank).into_iter().any(|ctx_map| ctx_map.contains(p))
    }

    pub fn get(&self, iteration: usize) -> Option<&Arc<ContextMap<T, N>>> {
        self.bank.get(iteration)
    }

    #[inline]
    pub fn insert(&mut self, ctx_map: Arc<ContextMap<T, N>>) {
        self.bank.push(ctx_map);
    }

    #[inline]
    pub fn iteration_count(&self) -> usize {
        self.bank.len()
    }
}
