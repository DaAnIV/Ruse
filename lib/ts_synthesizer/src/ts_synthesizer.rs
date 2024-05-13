use ruse_object_graph::Cache;
use ruse_synthesizer::{
    bank::ContextArray,
    prog::SubProgram,
    synthesizer::{CurrentStatistics, OpcodesList, Synthesizer, SynthesizerPredicate},
};
use ruse_ts_interpreter::opcode::TsExprAst;

use std::sync::Arc;

pub struct TsSynthesizer<const N: usize> {
    inner: Arc<Synthesizer<TsExprAst, N>>,
}

impl<const N: usize> TsSynthesizer<N> {
    pub fn new(
        start_context: ContextArray<N>,
        opcodes: OpcodesList<TsExprAst>,
        predicate: SynthesizerPredicate<TsExprAst, N>,
        valid: SynthesizerPredicate<TsExprAst, N>,
        max_context_depth: usize
    ) -> Self {
        Self {
            inner: Arc::new(Synthesizer::new(
                start_context,
                opcodes,
                predicate,
                valid,
                max_context_depth
            )),
        }
    }

    #[inline]
    pub async fn run_iteration(
        &mut self,
        cache: &Arc<Cache>,
    ) -> Option<Arc<SubProgram<TsExprAst, N>>>
    {
        Synthesizer::run_iteration(&mut self.inner, cache).await
    }

    #[inline]
    pub fn statistics(&self) -> CurrentStatistics {
        self.inner.statistics()
    }
}
