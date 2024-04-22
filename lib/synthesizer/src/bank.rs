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
// iteration -> [pre_context] -> out_type -> sub_prog

pub type ValueMap<T, const N: usize> = HashSet<Arc<SubProgram<T, N>>>;

pub type TypeMap<T, const N: usize> = [ValueMap<T, N>; ValueType::range()];

#[derive(Default)]
pub struct ContextMap<T: ExprAst + Default, const N: usize>(HashMap<ContextArray<N>, TypeMap<T, N>>);

pub fn new_type_map<T: ExprAst, const N: usize>() -> TypeMap<T, N> {
    TypeMap::<T, N>::default()
}

impl<T: ExprAst + Default, const N: usize> ContextMap<T, N> {
    pub fn get(&self, ctx: &ContextArray<N>) -> Option<&TypeMap<T, N>> {
        self.0.get(ctx)
    }

    pub fn insert_program(&mut self, p: Arc<SubProgram<T, N>>) -> bool {
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

    pub fn iter(&self) -> std::collections::hash_map::Iter<'_, ContextArray<N>, TypeMap<T, N>> {
        self.0.iter()
    }

    pub fn insert(&mut self, ctx: &ContextArray<N>, map: TypeMap<T, N>) {
        self.0.insert(ctx.clone(), map);
    }

    pub fn keys(&self) -> std::collections::hash_map::Keys<'_, ContextArray<N>, TypeMap<T, N>> {
        self.0.keys()
    }
}

#[derive(Default)]
pub struct ProgBank<T: ExprAst + Default, const N: usize> {
    bank: Vec<ContextMap<T, N>>,
    existing_outputs: ContextMap<T, N>,
}

impl<T: ExprAst + Default, const N: usize> ProgBank<T, N> {
    pub fn progs<'a>(
        &'a self,
        value_type: ValueType,
        ctx: &ContextArray<N>,
    ) -> ProgIterator<'a, T,N> {        
        let type_map = self.existing_outputs.get(ctx);
        if type_map.is_none() {
            return Box::new(iter::empty());
        }
        let value_map = &type_map.unwrap()[value_type as usize];

        Box::new(value_map.iter())
    }

    pub fn progs_for_iteration<'a>(
        &'a self,
        iteration: usize,
        value_type: ValueType,
        ctx: &ContextArray<N>,
    ) -> ProgIterator<'a, T,N> {
        let ctx_map = self.bank.get(iteration);
        if ctx_map.is_none() {
            return Box::new(iter::empty());
        }
        let type_map = &unsafe { ctx_map.unwrap_unchecked() }.get(ctx);
        if type_map.is_none() {
            return Box::new(iter::empty());
        }
        let value_map = &unsafe { type_map.unwrap_unchecked() }[value_type as usize];

        Box::new(value_map.iter())
    }

    pub fn output_exists(&self, p: &Arc<SubProgram<T, N>>) -> bool {
        let type_map = self.existing_outputs.get(p.pre_ctx());
        if type_map.is_none() {
            return false;
        }
        let value_map = &type_map.unwrap()[p.out_type() as usize];

        return value_map.contains(p);
    }

    pub fn get(&self, iteration: usize) -> Option<&ContextMap<T, N>> {
        self.bank.get(iteration)
    }

    #[inline]
    pub fn insert(&mut self, ctx_map: ContextMap<T, N>) {
        for (ctx, type_map) in ctx_map.iter() {
            let outputs_type_map = match self.existing_outputs.0.get_mut(ctx) {
                Some(m) => m,
                None => {
                    let m = new_type_map();
                    self.existing_outputs.0.insert(ctx.clone(), m);
                    unsafe { self.existing_outputs.0.get_mut(ctx).unwrap_unchecked() }
                }
            };
            for i in 0..ValueType::range() {
                type_map[i].iter().for_each(|v| {
                    outputs_type_map[v.out_type() as usize].insert(v.clone());
                });
            }
        }
        self.bank.push(ctx_map);
    }

    #[inline]
    pub fn iteration_count(&self) -> usize {
        self.bank.len()
    }
}
