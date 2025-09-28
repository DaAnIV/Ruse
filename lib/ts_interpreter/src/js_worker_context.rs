use ruse_synthesizer::synthesizer_context::*;

use crate::{engine_context::EngineContext, ts_classes::TsClasses};

#[derive(Debug, Default)]
pub struct JsWorkerContextData {
    pub engine_ctx: Option<EngineContext>,
}
impl SynthesizerWorkerContextData for JsWorkerContextData {
    fn gc(&self) {
        if let Some(engine_ctx) = &self.engine_ctx {
            engine_ctx.gc();
        }
    }
}

impl JsWorkerContextData {
    pub fn get_engine_ctx(&mut self, classes: &TsClasses) -> &mut EngineContext {
        self.engine_ctx
            .get_or_insert_with(|| EngineContext::create_engine_ctx(classes))
    }
}

pub fn create_js_worker_context(index: usize) -> SynthesizerWorkerContext {
    SynthesizerWorkerContext {
        index,
        data: Box::new(JsWorkerContextData { engine_ctx: None }),
    }
}
