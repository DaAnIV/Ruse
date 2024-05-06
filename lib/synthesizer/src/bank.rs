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

pub type ValueMap<T, const N: usize> = DashSet<Arc<SubProgram<T, N>>>;

pub type TypeMap<T, const N: usize> = [ValueMap<T, N>; ValueType::range()];

#[derive(Default)]
pub struct ContextMap<T: ExprAst + Default, const N: usize>(
    pub(crate) DashMap<ContextArray<N>, TypeMap<T, N>>,
);

pub fn new_type_map<T: ExprAst, const N: usize>() -> TypeMap<T, N> {
    TypeMap::<T, N>::default()
}

impl<T: ExprAst + Default, const N: usize> ContextMap<T, N> {
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
                let m = new_type_map();
                self.0.insert(ctx.clone(), m);
                unsafe { self.0.get_mut(ctx).unwrap_unchecked() }
            }
        };

        type_map[p.out_type() as usize].insert(p)
    }

    pub fn insert(&self, ctx: &ContextArray<N>, map: TypeMap<T, N>) {
        self.0.insert(ctx.clone(), map);
    }
}

#[derive(Default)]
pub struct ProgBank<T: ExprAst + Default, const N: usize> {
    pub(crate) bank: Vec<Arc<ContextMap<T, N>>>,
}

impl<T: ExprAst + Default, const N: usize> ProgBank<T, N> {
    pub fn output_exists(&self, p: &Arc<SubProgram<T, N>>) -> bool {
        (&self.bank).into_iter().any(|ctx_map| {
            let type_map = ctx_map.get(p.pre_ctx());
            if type_map.is_none() {
                return false;
            }
            let value_map = &type_map.unwrap()[p.out_type() as usize];

            return value_map.contains(p);
        })
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
