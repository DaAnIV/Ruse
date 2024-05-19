use ruse_object_graph::Cache;
use ruse_synthesizer::{
    bank::ContextArray,
    prog::SubProgram,
    synthesizer::{CurrentStatistics, OpcodesList, Synthesizer, SynthesizerPredicate},
};

use std::sync::Arc;

pub struct TsSynthesizer {
    inner: Arc<Synthesizer>,
}

impl TsSynthesizer {
    pub fn new(
        start_context: ContextArray,
        opcodes: OpcodesList,
        predicate: SynthesizerPredicate,
        valid: SynthesizerPredicate,
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
    ) -> Option<Arc<SubProgram>>
    {
        Synthesizer::run_iteration(&mut self.inner, cache).await
    }

    #[inline]
    pub fn statistics(&self) -> CurrentStatistics {
        self.inner.statistics()
    }
}
