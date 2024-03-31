use std::{collections::HashMap, iter, sync::Arc};

use crate::{
    context::Context,
    opcode::ExprAst,
    prog::SubProgram,
    value::{LocValue, ValueType},
};

pub type ProgIterator<'a, T, const N: usize> = Box<dyn Iterator<Item = Arc<SubProgram<T, N>>> + 'a>;

// The bank is hierarchical
// size-> [pre_context] -> out_type -> [out_value] -> sub_prog

pub type ValueMap<T, const N: usize> = HashMap<[LocValue; N], Arc<SubProgram<T, N>>>;

pub type TypeMap<T, const N: usize> = HashMap<ValueType, ValueMap<T, N>>;

pub type ContextMap<T, const N: usize> = HashMap<[Context; N], TypeMap<T, N>>;
type SizeMap<T, const N: usize> = HashMap<usize, ContextMap<T, N>>;

pub fn new_type_map<T: ExprAst, const N: usize>() -> TypeMap<T, N> {
    let mut new_obj = TypeMap::<T, N>::with_capacity(ValueType::range());

    for i in 0..ValueType::range() {
        new_obj.insert(i.try_into().unwrap(), Default::default());
    }

    new_obj
}

#[derive(Default)]
pub struct ProgBank<T: ExprAst + Default, const N: usize> {
    bank: SizeMap<T, N>,
}

impl<T: ExprAst + Default, const N: usize> ProgBank<T, N> {
    pub fn progs<'a>(
        &'a self,
        size: usize,
        value_type: ValueType,
        ctx: &[Context; N],
    ) -> ProgIterator<'a, T,N> {
        let ctx_map = self.bank.get(&size);
        if ctx_map.is_none() {
            return Box::new(iter::empty());
        }
        let type_map = ctx_map.unwrap().get(ctx);
        if type_map.is_none() {
            return Box::new(iter::empty());
        }
        let value_map = type_map.unwrap().get(&value_type);
        if value_map.is_none() {
            return Box::new(iter::empty());
        }

        Box::new(value_map.unwrap().values().map(|x| x.clone()))
    }

    #[inline]
    pub fn insert(&mut self, size: usize, ctx_map: ContextMap<T, N>) {
        self.bank.insert(size, ctx_map);
    }
}
