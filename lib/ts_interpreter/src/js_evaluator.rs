use boa_engine::JsResult;
use ruse_object_graph::value::Value;
use ruse_synthesizer::context::{Context, SynthesizerContext, SynthesizerWorkerContext};

use crate::{
    engine_context::EngineContext,
    js_value::{TryFromJs, TryIntoJs},
    js_worker_context::JsWorkerContextData,
    ts_classes::TsClasses,
};

pub struct JsEvaluator {}

impl JsEvaluator {
    pub fn evaluate_get_js_value(
        js: &str,
        ctx: &Context,
        output: &Value,
        syn_ctx: &SynthesizerContext,
        worker_ctx: &mut SynthesizerWorkerContext,
    ) -> JsResult<boa_engine::JsValue> {
        let classes = syn_ctx.data.downcast_ref::<TsClasses>().unwrap();
        let worker_ctx = worker_ctx
            .data
            .downcast_mut::<JsWorkerContextData>()
            .unwrap();
        let mut engine_ctx = worker_ctx.get_engine_ctx(classes);

        engine_ctx.reset_with_context(ctx, classes);

        let js_func = Self::create_function(js, ctx, &mut engine_ctx)?;
        let func = js_func.as_callable().unwrap();

        let js_values = Self::get_js_args(ctx, output, engine_ctx)?;

        func.call(&boa_engine::JsValue::null(), &js_values, &mut engine_ctx)
    }

    fn get_js_args(
        ctx: &Context,
        output: &Value,
        engine_ctx: &mut EngineContext,
    ) -> JsResult<Vec<boa_engine::JsValue>> {
        let mut js_values = Vec::with_capacity(ctx.variable_count() + 1);
        for (_, value) in ctx.variables() {
            js_values.push(value.try_into_js(engine_ctx)?);
        }
        js_values.push(output.try_into_js(engine_ctx)?);

        Ok(js_values)
    }

    fn create_function(
        js: &str,
        ctx: &Context,
        engine_ctx: &mut EngineContext,
    ) -> JsResult<boa_engine::JsValue> {
        let mut arg_names: Vec<&str> = Vec::with_capacity(ctx.variable_count() + 1);
        for (var, _) in ctx.variables() {
            arg_names.push(var.as_str());
        }
        let code = format!("({}) => {{return {}}}", arg_names.join(", "), js);
        engine_ctx.eval(boa_engine::Source::from_bytes(&code))
    }

    pub fn evaluate_get_value(
        js: &str,
        ctx: &Context,
        output: &Value,
        syn_ctx: &SynthesizerContext,
        worker_ctx: &mut SynthesizerWorkerContext,
    ) -> JsResult<Value> {
        let js_value = Self::evaluate_get_js_value(js, ctx, output, syn_ctx, worker_ctx)?;
        Self::convert_js_value(js_value, syn_ctx, worker_ctx)
    }

    fn convert_js_value(
        js_value: boa_engine::JsValue,
        syn_ctx: &SynthesizerContext,
        worker_ctx: &mut SynthesizerWorkerContext,
    ) -> JsResult<Value> {
        let classes = syn_ctx.data.downcast_ref::<TsClasses>().unwrap();
        let worker_ctx = worker_ctx
            .data
            .downcast_mut::<JsWorkerContextData>()
            .unwrap();
        let mut engine_ctx = worker_ctx.get_engine_ctx(classes);
        Value::try_from_js(&js_value, &mut engine_ctx)
    }
}
