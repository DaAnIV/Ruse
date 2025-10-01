use ruse_synthesizer::{
    bank::ProgBank,
    opcode::OpcodesList,
    synthesizer::{Synthesizer, SynthesizerOptions, SynthesizerPredicate, WorkerContextCreator},
    synthesizer_context::{SynthesizerContext, SynthesizerWorkerContext},
};
use ruse_ts_interpreter::js_worker_context::create_js_worker_context;

pub struct TsWorkerContextCreator {}
impl WorkerContextCreator for TsWorkerContextCreator {
    fn create_worker_ctx(index: usize) -> SynthesizerWorkerContext {
        create_js_worker_context(index)
    }
}

pub type TsSynthesizer<P> = Synthesizer<P, TsWorkerContextCreator>;

pub fn create_ts_synthesizer<P: ProgBank + 'static>(
    bank: P,
    syn_ctx: SynthesizerContext,
    opcodes: OpcodesList,
    predicate: SynthesizerPredicate,
    valid: SynthesizerPredicate,
    options: SynthesizerOptions,
) -> TsSynthesizer<P> {
    Synthesizer::<P, TsWorkerContextCreator>::new(bank, syn_ctx, opcodes, predicate, valid, options)
}
