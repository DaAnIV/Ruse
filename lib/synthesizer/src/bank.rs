use std::{collections::{HashMap, HashSet}, iter, sync::Arc};

use crate::{
    context::Context,
    opcode::ExprAst,
    prog::SubProgram,
    value::{LocValue, ValueType},
};

pub type ContextArray<const N: usize> =  Arc<[Context; N]>;
pub type ValueArray<const N: usize> =  Arc<[LocValue; N]>;

pub type ProgIterator<'a, T, const N: usize> = Box<dyn Iterator<Item = &'a Arc<SubProgram<T, N>>> + 'a>;

// The bank is hierarchical
// size-> [pre_context] -> out_type -> [out_value] -> sub_prog

pub type ValueMap<T, const N: usize> = HashMap<ValueArray<N>, Arc<SubProgram<T, N>>>;

pub type TypeMap<T, const N: usize> = [ValueMap<T, N>; ValueType::range()];

type ContextMap<T, const N: usize> = HashMap<ContextArray<N>, TypeMap<T, N>>;
type SizeMap<T, const N: usize> = HashMap<usize, ContextMap<T, N>>;

pub type OutputsMap<const N: usize> = [HashSet<(ContextArray<N>, ValueArray<N>)>; ValueType::range()];

pub fn new_type_map<T: ExprAst, const N: usize>() -> TypeMap<T, N> {
    TypeMap::<T, N>::default()
}

#[derive(Default)]
pub struct ProgBank<T: ExprAst + Default, const N: usize> {
    bank: SizeMap<T, N>,
    existing_outputs: OutputsMap<N>,
}

impl<T: ExprAst + Default, const N: usize> ProgBank<T, N> {
    pub fn progs<'a>(
        &'a self,
        size: usize,
        value_type: ValueType,
        ctx: &ContextArray<N>,
    ) -> ProgIterator<'a, T,N> {
        let ctx_map = self.bank.get(&size);
        if ctx_map.is_none() {
            return Box::new(iter::empty());
        }
        let type_map = ctx_map.unwrap().get(ctx);
        if type_map.is_none() {
            return Box::new(iter::empty());
        }
        let value_map = &type_map.unwrap()[value_type as usize];

        Box::new(value_map.values())
    }

    pub fn output_exists(&self, p: &Arc<SubProgram<T, N>>) -> bool {
        self.existing_outputs[p.out_type() as usize].contains(&(p.post_ctx().clone(), p.out_value().clone()))
    }

    #[inline]
    pub fn insert(&mut self, size: usize, ctx: ContextArray<N>, type_map: TypeMap<T, N>) {
        let ctx_map = match self.bank.get_mut(&size) {
            Some(m) => m,
            None => {
                let m = ContextMap::new();
                self.bank.insert(size, m);
                unsafe { self.bank.get_mut(&size).unwrap_unchecked() }
            }
        };
        for i in 0..ValueType::range() {
            type_map[i].values().for_each(|v| {
                self.existing_outputs[i].insert((v.post_ctx().clone(), v.out_value().clone()));
            });
        }
        ctx_map.insert(ctx, type_map);
    }
}
