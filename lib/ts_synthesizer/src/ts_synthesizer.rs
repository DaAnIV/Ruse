use ruse_synthesizer::{
    bank::ProgBank,
    context::VariableName,
    opcode::OpcodesList,
    prog::SubProgram,
    synthesizer::{CurrentStatistics, Synthesizer, SynthesizerPredicate, WorkerContextCreator},
    synthesizer_context::{SynthesizerContext, SynthesizerWorkerContext},
};
use ruse_ts_interpreter::js_worker_context::create_js_worker_context;
use tokio_util::sync::CancellationToken;

use std::{fmt::Display, sync::Arc};

pub struct TsWorkerContextCreator {}
impl WorkerContextCreator for TsWorkerContextCreator {
    fn create_worker_ctx(&self, index: usize) -> SynthesizerWorkerContext {
        create_js_worker_context(index)
    }
}

pub struct TsSynthesizer<P: ProgBank> {
    inner: Arc<Synthesizer<P, TsWorkerContextCreator>>,
}

impl<P: ProgBank + 'static> TsSynthesizer<P> {
    pub fn new(
        bank: P,
        syn_ctx: SynthesizerContext,
        opcodes: OpcodesList,
        predicate: SynthesizerPredicate,
        valid: SynthesizerPredicate,
        max_mutations: u32,
        iteration_workers_count: usize,
    ) -> TsSynthesizer<P> {
        Self {
            inner: Arc::new(Synthesizer::new(
                bank,
                syn_ctx,
                opcodes,
                predicate,
                valid,
                max_mutations,
                iteration_workers_count,
                TsWorkerContextCreator {},
            )),
        }
    }

    #[inline]
    pub async fn run_iteration(&mut self) -> anyhow::Result<Option<Arc<SubProgram>>> {
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

    pub fn json_display(&self) -> impl Display + '_ {
        self.inner.json_display()
    }

    pub fn create_worker_ctx(&self, index: usize) -> SynthesizerWorkerContext {
        create_js_worker_context(index)
    }
}
