use std::sync::Arc;

use itertools::{izip, Itertools};
use ruse_object_graph::graph_map_value::GraphMapWrap;
use ruse_object_graph::value::{ObjectValue, Value};
use ruse_object_graph::{Cache, GraphsMap};
use ruse_synthesizer::context::ValuesMap;
use ruse_synthesizer::{context::SynthesizerContext, prog::SubProgram};
use ruse_ts_interpreter::js_object_wrapper::EngineContext;
use ruse_ts_interpreter::js_value::value_to_js_value;
use ruse_ts_interpreter::ts_class::TsClasses;

pub type SynthesizerPredicate = Box<dyn Fn(&SubProgram, &SynthesizerContext) -> bool + Send + Sync>;

pub struct PredicateBuilder {
    pub output_array: Option<Vec<Value>>,
    pub state_array: Option<Vec<ValuesMap>>,
    pub predicate_js: Option<Vec<String>>,

    pub graphs_map: GraphsMap,
    pub cache: Arc<Cache>,
}

impl PredicateBuilder {
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

        for (ctx, js) in p.post_ctx().iter().zip_eq(predicate_js) {
            engine_ctx.reset_with_context(ctx, classes, &self.cache);
            let mut arg_names = Vec::with_capacity(ctx.variable_count());
            let mut js_values = Vec::with_capacity(ctx.variable_count());
            for (var, value) in ctx
                .variables()
                .filter(|(_, v)| v.is_primitive() || v.is_null())
            {
                arg_names.push(var.as_str());
                js_values.push(value_to_js_value(classes, value, &mut engine_ctx));
            }
            // Best effort for partial contexts, take all of the roots of all graphs
            for (g, root_name, node) in ctx.graphs_map.all_roots() {
                if root_name.as_str() == Cache::OUTPUT_ROOT_NAME {
                    continue;
                }
                arg_names.push(root_name.as_str());
                let value = ObjectValue {
                    obj_type: g.obj_type(&node).unwrap().clone(),
                    graph_id: g.id,
                    node: node,
                }
                .into();
                js_values.push(value_to_js_value(classes, &value, &mut engine_ctx));
            }
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

pub struct ValidPredicateBuilder {}

impl ValidPredicateBuilder {
    fn predicate(&self, _p: &SubProgram, _syn_ctx: &SynthesizerContext) -> bool {
        true
    }

    pub fn finalize(self) -> SynthesizerPredicate {
        Box::new(move |p: &SubProgram, syn_ctx: &SynthesizerContext| self.predicate(p, syn_ctx))
    }
}
