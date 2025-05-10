use itertools::{izip, Itertools};
use ruse_object_graph::graph_map_value::GraphMapWrap;
use ruse_object_graph::GraphsMap;
use ruse_object_graph::{value::Value, ValueType};
use ruse_synthesizer::context::ValuesMap;
use ruse_synthesizer::{context::SynthesizerContext, prog::SubProgram};
use ruse_ts_interpreter::engine_context::EngineContext;
use ruse_ts_interpreter::js_value::value_to_js_value;
use ruse_ts_interpreter::ts_classes::TsClasses;

pub type SynthesizerPredicate = Box<dyn Fn(&SubProgram, &SynthesizerContext) -> bool + Send + Sync>;

pub struct PredicateBuilder {
    pub output_type: Option<ValueType>,
    pub output_array: Option<Vec<Value>>,
    pub state_array: Option<Vec<ValuesMap>>,
    pub predicate_js: Option<Vec<String>>,

    pub graphs_map: GraphsMap,
}

impl PredicateBuilder {
    fn output_type_predicate(&self, p: &SubProgram, _syn_ctx: &SynthesizerContext) -> bool {
        if let Some(output_type) = &self.output_type {
            p.out_type() == output_type
        } else {
            true
        }
    }

    fn output_predicate_inner(
        &self,
        output_array: &[Value],
        p: &SubProgram,
        _syn_ctx: &SynthesizerContext,
    ) -> bool {
        for (actual, actual_ctx, expected) in
            izip!(p.out_value().iter(), p.post_ctx().iter(), output_array)
        {
            if actual.val().wrap(&actual_ctx.graphs_map) != expected.wrap(&self.graphs_map) {
                return false;
            }
        }

        true
    }

    fn output_predicate(&self, p: &SubProgram, syn_ctx: &SynthesizerContext) -> bool {
        if let Some(output_array) = &self.output_array {
            return self.output_predicate_inner(output_array, p, syn_ctx);
        } else {
            true
        }
    }

    fn state_predicate_inner(
        &self,
        state_array: &[ValuesMap],
        p: &SubProgram,
        _syn_ctx: &SynthesizerContext,
    ) -> bool {
        for (actual, expected) in p.post_ctx().iter().zip_eq(state_array) {
            for (var, value) in expected.iter() {
                let actual_value = match actual.get_var_value(var) {
                    None => return false,
                    Some(v) => v,
                };
                if actual_value.wrap(&actual.graphs_map) != value.wrap(&self.graphs_map) {
                    return false;
                }
            }
        }

        true
    }

    fn state_predicate(&self, p: &SubProgram, syn_ctx: &SynthesizerContext) -> bool {
        if let Some(state_array) = &self.state_array {
            return self.state_predicate_inner(state_array, p, syn_ctx);
        } else {
            true
        }
    }

    fn js_predicate_inner(
        &self,
        predicate_js: &[String],
        p: &SubProgram,
        syn_ctx: &SynthesizerContext,
    ) -> bool {
        let classes = syn_ctx.data.downcast_ref::<TsClasses>().unwrap();
        let mut boa_ctx = EngineContext::new_boa_ctx();
        let mut engine_ctx = EngineContext::create_engine_ctx(&mut boa_ctx, classes);

        debug_assert!(
            p.post_ctx().len() == p.out_value().len() && p.post_ctx().len() == predicate_js.len()
        );

        for (ctx, output, js) in izip!(p.post_ctx().iter(), p.out_value().iter(), predicate_js) {
            engine_ctx.reset_with_context(ctx, classes);
            let mut arg_names = Vec::with_capacity(ctx.variable_count() + 1);
            let mut js_values = Vec::with_capacity(ctx.variable_count() + 1);
            for (var, value) in ctx.variables() {
                arg_names.push(var.as_str());
                js_values.push(value_to_js_value(classes, value, &mut engine_ctx).unwrap());
            }
            arg_names.push("__output__");
            js_values.push(value_to_js_value(classes, output.val(), &mut engine_ctx).unwrap());

            let code = format!(
                "function func({}) {{return {}}}\nfunc",
                arg_names.join(", "),
                js
            );
            let js_func = engine_ctx
                .eval(boa_engine::Source::from_bytes(&code))
                .unwrap();
            let func = js_func.as_callable().unwrap();
            match func.call(&boa_engine::JsValue::null(), &js_values, &mut engine_ctx) {
                Ok(val) => {
                    if let Some(b) = val.as_boolean() {
                        if b {
                            continue;
                        }
                    }
                    return false;
                }
                Err(_) => return false,
            };
        }

        true
    }

    fn js_predicate(&self, p: &SubProgram, syn_ctx: &SynthesizerContext) -> bool {
        if let Some(predicate_js) = &self.predicate_js {
            return self.js_predicate_inner(predicate_js, p, syn_ctx);
        } else {
            true
        }
    }

    fn predicate(&self, p: &SubProgram, syn_ctx: &SynthesizerContext) -> bool {
        if !self.output_type_predicate(p, syn_ctx) {
            return false;
        }
        if !self.output_predicate(p, syn_ctx) {
            return false;
        }
        if !self.state_predicate(p, syn_ctx) {
            return false;
        }
        if !self.js_predicate(p, syn_ctx) {
            return false;
        }

        true
    }

    pub fn finalize(self) -> SynthesizerPredicate {
        Box::new(move |p: &SubProgram, syn_ctx: &SynthesizerContext| self.predicate(p, syn_ctx))
    }
}

pub struct ValidPredicateBuilder {
    pub max_string_size: usize,
}

impl ValidPredicateBuilder {
    fn predicate(&self, p: &SubProgram, _syn_ctx: &SynthesizerContext) -> bool {
        if p.out_type() == &ValueType::String {
            if p.out_value().iter().any(|x| {
                let str_val = unsafe { x.val().string_value().unwrap_unchecked() };
                str_val.len() > self.max_string_size
            }) {
                return false;
            }
        }
        true
    }

    pub fn finalize(self) -> SynthesizerPredicate {
        Box::new(move |p: &SubProgram, syn_ctx: &SynthesizerContext| self.predicate(p, syn_ctx))
    }
}
