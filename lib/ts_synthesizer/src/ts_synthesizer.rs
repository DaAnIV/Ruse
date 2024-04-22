use ruse_object_graph::Cache;
use ruse_synthesizer::{
    bank::ContextArray,
    prog::SubProgram,
    synthesizer::{OpcodesList, Statistics, Synthesizer},
};
use ruse_ts_interpreter::opcode::TsExprAst;

use std::sync::Arc;

pub struct TsSynthesizer<const N: usize>(Synthesizer<TsExprAst, N>);

impl<const N: usize> TsSynthesizer<N> {
    pub fn with_context_and_opcodes(
        start_context: ContextArray<N>,
        opcodes: OpcodesList<TsExprAst>,
    ) -> Self {
        Self {
            0: Synthesizer::with_context_and_opcodes(start_context, opcodes),
        }
    }

    #[inline]
    pub fn run_iteration<F, V>(
        &mut self,
        cache: &Cache,
        predicate: F,
        valid: V,
    ) -> Option<Arc<SubProgram<TsExprAst, N>>>
    where
        F: Fn(&Arc<SubProgram<TsExprAst, N>>) -> bool,
        V: Fn(&Arc<SubProgram<TsExprAst, N>>) -> bool,
    {
        self.0.run_iteration(cache, predicate, valid)
    }

    #[inline]
    pub fn statistics(&self) -> &Statistics {
        &self.0.statistics()
    }
}
