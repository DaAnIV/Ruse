use ruse_object_graph::Cache;

use crate::{
    arg_iter::ArgIterator,
    bank::*,
    opcode::{ExprAst, SynthesizerExprOpcode},
    prog::SubProgram,
};
use std::{collections::HashSet, sync::Arc};

pub type OpcodesList<T> = Vec<Arc<dyn SynthesizerExprOpcode<T>>>;

#[derive(Default)]
pub struct Statistics {
    pub generated: u64,
    pub bank_size: u64,
}

pub struct Synthesizer<T: ExprAst + Default, const N: usize> {
    bank: ProgBank<T, N>,
    init_opcodes: OpcodesList<T>,
    composite_opcodes: OpcodesList<T>,
    found_contexts: HashSet<ContextArray<N>>,
    statistics: Statistics
}

impl<T: ExprAst + Default, const N: usize> Synthesizer<T, N> {
    pub fn with_context_and_opcodes(
        start_context: ContextArray<N>,
        opcodes: OpcodesList<T>,
        cache: &mut Cache,
    ) -> Self {
        let (init_opcodes, composite_opcodes) =
            opcodes.into_iter().partition(|x| x.arg_types().len() == 0);

        let mut new_obj = Self {
            bank: Default::default(),
            init_opcodes: init_opcodes,
            composite_opcodes: composite_opcodes,
            found_contexts: HashSet::new(),
            statistics: Default::default()
        };

        new_obj.init_context(&start_context, cache);
        new_obj.found_contexts.insert(start_context);
        new_obj
    }

    fn insert_program_to_type_map(&self, type_map: &mut TypeMap<T, N>, p: Arc<SubProgram<T, N>>) -> bool {
        let value_map = &mut type_map[p.out_type() as usize];
        if !value_map.contains_key(p.out_value()) && !self.bank.output_exists(&p) {
            value_map.insert(p.out_value().clone(), p);
            return true;
        }

        return false;
    }

    fn init_context(&mut self, ctx: &ContextArray<N>, cache: &mut Cache) {
        let mut type_map = new_type_map::<T, N>();
        for op in &self.init_opcodes {
            let mut p = SubProgram::<T, N>::with_opcode_and_context(op.clone(), ctx);
            p.evaluate(cache);
            self.statistics.generated += 1;
            if self.insert_program_to_type_map(&mut type_map, p.into()) {
                self.statistics.bank_size += 1;
            }
        }

        self.bank.insert(1, ctx.clone(), type_map);
    }

    pub fn synthesize_for_size(&mut self, ctx: &ContextArray<N>, n: usize, cache: &mut Cache) {
        let mut type_map = new_type_map::<T, N>();
        let mut found_contexts = HashSet::<ContextArray<N>>::new();

        for op in &self.composite_opcodes {
            if op.arg_types().len() >= n { continue; }
            for args in ArgIterator::new(&self.bank, ctx, n - 1, op.arg_types()) {
                let mut p = SubProgram::with_opcode_and_children(op.clone(), args);
                p.evaluate(cache);
                self.statistics.generated += 1;
                found_contexts.insert(p.post_ctx().clone());
                if self.insert_program_to_type_map(&mut type_map, p.into()) {
                    self.statistics.bank_size += 1;
                }
            }
        }

        self.bank.insert(n, ctx.clone(), type_map);

        for ctx in found_contexts.iter() {
            if self.found_contexts.insert(ctx.clone()) {
                self.init_context(ctx, cache)
            }
        }
    }

    #[inline]
    pub fn statistics(&self) -> &Statistics {
        &self.statistics
    }
}

impl<T: ExprAst + Default, const N: usize> Iterator for Synthesizer<T, N> {
    type Item = SubProgram<T, N>;

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}
