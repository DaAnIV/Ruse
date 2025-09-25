use std::collections::HashMap;

use boa_engine::JsResult;
use ruse_object_graph::value::Value;
use ruse_synthesizer::context::{Context, SynthesizerContext, SynthesizerWorkerContext};

use crate::{
    engine_context::EngineContext,
    js_value::{TryFromJs, TryIntoJs},
    js_worker_context::JsWorkerContextData,
    ts_classes::TsClasses,
};

pub struct JsEvaluator {
    utils_code: String,
}

impl JsEvaluator {
    pub fn new_with_utils(utils_code: String) -> Self {
        Self { utils_code }
    }

    pub fn new() -> Self {
        Self {
            utils_code: String::new(),
        }
    }

    pub fn evaluate_get_js_value(
        &self,
        js: &str,
        ctx: &Context,
        output: &Value,
        extra_args: &HashMap<&str, boa_engine::JsValue>,
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

        let js_func = self.create_function(js, ctx, extra_args, &mut engine_ctx)?;
        let func = js_func.as_callable().unwrap();

        let js_values = Self::get_js_args(ctx, output, extra_args, engine_ctx)?;

        func.call(&boa_engine::JsValue::null(), &js_values, &mut engine_ctx)
    }

    fn get_js_args(
        ctx: &Context,
        output: &Value,
        extra_args: &HashMap<&str, boa_engine::JsValue>,
        engine_ctx: &mut EngineContext,
    ) -> JsResult<Vec<boa_engine::JsValue>> {
        let mut js_values = Vec::with_capacity(ctx.variable_count() + 1);
        for (_, value) in ctx.variables() {
            js_values.push(value.try_into_js(engine_ctx)?);
        }
        js_values.push(output.try_into_js(engine_ctx)?);
        js_values.extend(extra_args.values().cloned());

        Ok(js_values)
    }

    fn create_function(
        &self,
        js: &str,
        ctx: &Context,
        extra_args: &HashMap<&str, boa_engine::JsValue>,
        engine_ctx: &mut EngineContext,
    ) -> JsResult<boa_engine::JsValue> {
        let mut arg_names: Vec<&str> = Vec::with_capacity(ctx.variable_count() + 1);
        for (var, _) in ctx.variables() {
            arg_names.push(var.as_str());
        }
        arg_names.push("__output__");
        arg_names.extend(extra_args.keys());
        let code = format!(
            "({}) => {{{}\nreturn {}}}",
            arg_names.join(", "),
            &self.utils_code,
            js
        );
        engine_ctx.eval(boa_engine::Source::from_bytes(&code))
    }

    pub fn evaluate_get_value(
        &self,
        js: &str,
        ctx: &Context,
        output: &Value,
        extra_args: &HashMap<&str, boa_engine::JsValue>,
        syn_ctx: &SynthesizerContext,
        worker_ctx: &mut SynthesizerWorkerContext,
    ) -> JsResult<Value> {
        let js_value =
            self.evaluate_get_js_value(js, ctx, output, extra_args, syn_ctx, worker_ctx)?;
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
