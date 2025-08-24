use ruse_synthesizer::{
    bank::ProgBank,
    context::{SynthesizerContext, VariableName},
    opcode::OpcodesList,
    prog::SubProgram,
    synthesizer::{CurrentStatistics, Synthesizer, SynthesizerPredicate},
};
use tokio_util::sync::CancellationToken;

use std::{fmt::Display, sync::Arc};

pub struct TsSynthesizer<P: ProgBank> {
    inner: Arc<Synthesizer<P>>,
}

impl<P: ProgBank + 'static> TsSynthesizer<P> {
    pub fn new(
        bank: P,
        syn_ctx: SynthesizerContext,
        opcodes: OpcodesList,
        predicate: SynthesizerPredicate,
        valid: SynthesizerPredicate,
        max_context_depth: usize,
        iteration_workers_count: usize,
    ) -> TsSynthesizer<P> {
        Self {
            inner: Arc::new(Synthesizer::new(
                bank,
                syn_ctx,
                opcodes,
                predicate,
                valid,
                max_context_depth,
                iteration_workers_count,
            )),
        }
    }

    #[inline]
    pub async fn run_iteration(&mut self) -> Option<Arc<SubProgram>> {
        Synthesizer::run_iteration(&mut self.inner).await
    }

    #[inline]
    pub fn statistics(&self) -> CurrentStatistics {
        self.inner.statistics()
    }

    pub fn get_cancel_token(&self) -> CancellationToken {
        self.inner.get_cancel_token()
    }

    pub fn set_immutable(&mut self, var: &VariableName) {
        Arc::get_mut(&mut self.inner).unwrap().set_immutable(var);
    }

    pub fn print_all_programs(&self) {
        self.inner.print_all_programs()
    }

    pub fn json_display(&self) -> impl Display + '_ {
        self.inner.json_display()
    }
}
