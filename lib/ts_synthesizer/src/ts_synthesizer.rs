use ruse_object_graph::Cache;
use ruse_synthesizer::{
    bank::ContextArray,
    prog::SubProgram,
    synthesizer::{CurrentStatistics, OpcodesList, Synthesizer, SynthesizerPredicate},
};
use ruse_ts_interpreter::opcode::TsExprAst;

use std::sync::Arc;

pub struct TsSynthesizer {
    inner: Arc<Synthesizer<TsExprAst>>,
}

impl TsSynthesizer {
    pub fn new(
        start_context: ContextArray,
        opcodes: OpcodesList<TsExprAst>,
        predicate: SynthesizerPredicate<TsExprAst>,
        valid: SynthesizerPredicate<TsExprAst>,
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
    ) -> Option<Arc<SubProgram<TsExprAst>>>
    {
        Synthesizer::run_iteration(&mut self.inner, cache).await
    }

    #[inline]
    pub fn statistics(&self) -> CurrentStatistics {
        self.inner.statistics()
    }
}
