use itertools::{izip, Itertools};
use ruse_object_graph::graph_map_value::GraphMapWrap;
use ruse_object_graph::GraphsMap;
use ruse_object_graph::{value::Value, ValueType};
use ruse_synthesizer::context::{SynthesizerWorkerContext, ValuesMap};
use ruse_synthesizer::{context::SynthesizerContext, prog::SubProgram};
use ruse_ts_interpreter::js_evaluator::JsEvaluator;

pub type SynthesizerPredicate = Box<
    dyn Fn(&SubProgram, &SynthesizerContext, &mut SynthesizerWorkerContext) -> bool + Send + Sync,
>;

pub trait Predicate: Send + Sync {
    fn predicate(
        &self,
        p: &SubProgram,
        syn_ctx: &SynthesizerContext,
        worker_ctx: &mut SynthesizerWorkerContext,
        graphs_map: &GraphsMap,
    ) -> bool;
}

pub struct PredicateBuilder {
    predicates: Vec<Box<dyn Predicate>>,
    graphs_map: GraphsMap,
}

impl PredicateBuilder {
    pub fn new(graphs_map: GraphsMap) -> Self {
        Self {
            predicates: Vec::new(),
            graphs_map,
        }
    }

    /// Add a predicate to the builder.
    /// The predicate would be run in the order they are added.
    /// Short-circuiting is applied, i.e., if one predicate returns false,
    /// the remaining predicates would not be executed.
    pub fn add_predicate<P: Predicate + 'static>(&mut self, predicate: P) {
        self.predicates.push(Box::new(predicate));
    }

    pub fn finalize(self) -> SynthesizerPredicate {
        let predicates = self.predicates;
        let graphs_map = self.graphs_map;
        Box::new(
            move |p: &SubProgram,
                  syn_ctx: &SynthesizerContext,
                  worker_ctx: &mut SynthesizerWorkerContext| {
                for predicate in &predicates {
                    if !predicate.predicate(p, syn_ctx, worker_ctx, &graphs_map) {
                        return false;
                    }
                }
                true
            },
        )
    }
}

pub struct OutputTypePredicate {
    pub output_type: ValueType,
}

impl Predicate for OutputTypePredicate {
    fn predicate(
        &self,
        p: &SubProgram,
        _syn_ctx: &SynthesizerContext,
        _worker_ctx: &mut SynthesizerWorkerContext,
        _graphs_map: &GraphsMap,
    ) -> bool {
        p.out_type() == &self.output_type
    }
}

pub struct OutputPredicate {
    pub output_array: Vec<Value>,
}

impl Predicate for OutputPredicate {
    fn predicate(
        &self,
        p: &SubProgram,
        _syn_ctx: &SynthesizerContext,
        _worker_ctx: &mut SynthesizerWorkerContext,
        graphs_map: &GraphsMap,
    ) -> bool {
        for (actual, actual_ctx, expected) in
            izip!(p.out_value().iter(), p.post_ctx().iter(), self.output_array.iter())
        {
            if actual.val().wrap(&actual_ctx.graphs_map)
                != expected.wrap(graphs_map)
            {
                return false;
            }
        }

        true
    }
}

pub struct StatePredicate {
    pub state_array: Vec<ValuesMap>,
}

impl Predicate for StatePredicate {
    fn predicate(
        &self,
        p: &SubProgram,
        _syn_ctx: &SynthesizerContext,
        _worker_ctx: &mut SynthesizerWorkerContext,
        graphs_map: &GraphsMap,
    ) -> bool {
        for (actual, expected) in p.post_ctx().iter().zip_eq(self.state_array.iter()) {
            for (var, value) in expected.iter() {
                let actual_value = match actual.get_var_value(var) {
                    None => return false,
                    Some(v) => v,
                };
                if actual_value.wrap(&actual.graphs_map) != value.wrap(graphs_map) {
                    return false;
                }
            }
        }

        true
    }
}

pub struct JsPredicate {
    pub predicate_js: Vec<String>,
}

impl Predicate for JsPredicate {
    fn predicate(
        &self,
        p: &SubProgram,
        syn_ctx: &SynthesizerContext,
        worker_ctx: &mut SynthesizerWorkerContext,
        _graphs_map: &GraphsMap,
    ) -> bool {
        debug_assert!(
            p.post_ctx().len() == p.out_value().len() && p.post_ctx().len() == self.predicate_js.len()
        );
        for (ctx, output, js) in izip!(p.post_ctx().iter(), p.out_value().iter(), self.predicate_js.iter()) {
            match JsEvaluator::evaluate_get_js_value(js, ctx, output.val(), syn_ctx, worker_ctx) {
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
    
}

pub struct StringSizeValidPredicate {
    pub max_string_size: usize,
}

impl Predicate for StringSizeValidPredicate {
    fn predicate(
        &self,
        p: &SubProgram,
        _syn_ctx: &SynthesizerContext,
        _worker_ctx: &mut SynthesizerWorkerContext,
        _graphs_map: &GraphsMap,
    ) -> bool {
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
}

pub struct NumberValidPredicate {
    pub allow_non_finite: bool,
}

impl Predicate for NumberValidPredicate {
    fn predicate(
        &self,
        p: &SubProgram,
        _syn_ctx: &SynthesizerContext,
        _worker_ctx: &mut SynthesizerWorkerContext,
        _graphs_map: &GraphsMap,
    ) -> bool {
        if p.out_type() == &ValueType::Number {
            for x in p.out_value().iter() {
                let num_val = unsafe { x.val().number_value().unwrap_unchecked() }.0;
                if !self.allow_non_finite && !num_val.is_finite() {
                    return false;
                }
            }
        }
        true
    }
}
