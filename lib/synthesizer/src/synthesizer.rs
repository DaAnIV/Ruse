use ruse_object_graph::Cache;

use crate::{
    arg_iter::ArgIterator, bank::*, context::Context, opcode::{ExprAst, SynthesizerExprOpcode}, prog::SubProgram
};
use std::sync::Arc;

pub type OpcodesList<T> = Vec<Arc<dyn SynthesizerExprOpcode<T>>>;

pub struct Synthesizer<T: ExprAst + Default, const N: usize> {
    bank: ProgBank<T, N>,
    init_opcodes: OpcodesList<T>,
    composite_opcodes: OpcodesList<T>,
}

impl<T: ExprAst + Default, const N: usize> Synthesizer<T, N> {
    pub fn with_context_and_opcodes(
        start_context: [Context; N],
        opcodes: OpcodesList<T>,
        cache: &mut Cache
    ) -> Self {
        let (init_opcodes, composite_opcodes) = opcodes.into_iter().partition(|x| x.arg_types().len() == 0);

        let mut new_obj = Self {
            bank: Default::default(),
            init_opcodes: init_opcodes,
            composite_opcodes: composite_opcodes,
        };

        new_obj.init_context(&start_context, cache);
        new_obj
    }

    fn insert_program_to_type_map(type_map: &mut TypeMap<T, N>, p: Arc<SubProgram<T, N>>) -> bool {
        let value_map = type_map.get_mut(&p.out_type()).unwrap();
        if !value_map.contains_key(p.out_value()) {
            value_map.insert(p.out_value().clone(), p);
            return true;
        }

        return false;
    }

    fn init_context(&mut self, ctx: &[Context; N], cache: &mut Cache) {
        let mut type_map = new_type_map::<T, N>();
        for op in &self.init_opcodes {
            let mut p = SubProgram::<T, N>::with_opcode_and_children(op.clone(), vec![]);
            p.evaluate(cache);
            Self::insert_program_to_type_map(&mut type_map, p.into());
        }
        let mut context_map = ContextMap::<T, N>::new();
        context_map.insert(ctx.clone(), type_map);
        self.bank.insert(1, context_map);
    }

    pub fn synthesize_for_size(&mut self, ctx: &[Context; N], n: usize, cache: &mut Cache) {
        let mut type_map = new_type_map::<T, N>();

        for op in &self.composite_opcodes {
            for args in ArgIterator::new(&self.bank, ctx, n - 1, op.arg_types()) {
                let mut p = SubProgram::with_opcode_and_children(op.clone(), args);
                p.evaluate(cache);
                Self::insert_program_to_type_map(&mut type_map, p.into());
            }
        }

        let mut context_map = ContextMap::<T, N>::new();
        context_map.insert(ctx.clone(), type_map);
        self.bank.insert(n, context_map);
    }
}

impl<T: ExprAst + Default, const N: usize> Iterator for Synthesizer<T, N> {
    type Item = SubProgram<T, N>;

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}
