use ruse_object_graph::{value::*, *};
use ruse_synthesizer::{
    context::{Context, SynthesizerContext},
    location::*,
    opcode::{EvalResult, ExprAst, ExprOpcode},
};
use tracing::debug;

use crate::{
    js_object_wrapper::EngineContext,
    opcode::{member_call_ast, static_member_call_ast},
    ts_class::TsClasses,
};

pub struct ClassMethodOp {
    obj_type: CachedString,
    method_name: String,
    full_method_name: String,
    arg_types: Vec<ValueType>,
    is_static: bool,
}

impl ClassMethodOp {
    pub fn new<I>(obj_type: CachedString, method_name: String, args: I, is_static: bool) -> Self
    where
        I: IntoIterator<Item = ValueType>,
    {
        let full_method_name = format!("{}.{}", &obj_type, &method_name);
        let mut arg_types = vec![];
        if !is_static {
            arg_types.push(ValueType::Object(obj_type.clone()));
        };
        arg_types.extend(args);

        Self {
            obj_type,
            method_name,
            full_method_name,
            arg_types,
            is_static,
        }
    }
}

impl ExprOpcode for ClassMethodOp {
    fn op_name(&self) -> &str {
        &self.full_method_name
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }

    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        let classes = syn_ctx.data.downcast_ref::<TsClasses>().unwrap();
        let mut boa_ctx = EngineContext::new_boa_ctx();
        let mut engine_ctx = EngineContext::create_engine_ctx(&mut boa_ctx, classes);
        engine_ctx.reset_with_context(post_ctx, classes, &syn_ctx.cache);
        let class = classes.get_class(&self.obj_type).unwrap();

        let result = if self.is_static {
            class.call_static_method(
                &self.method_name,
                args.iter().map(|x| x.val()),
                &post_ctx.graphs_map,
                classes,
                &syn_ctx.cache,
                &mut engine_ctx,
            )
        } else {
            class.call_method(
                &self.method_name,
                args[0].val(),
                args.iter().skip(1).map(|x| x.val()),
                &post_ctx.graphs_map,
                classes,
                &syn_ctx.cache,
                &mut engine_ctx,
            )
        };

        match result {
            // Need to check if func changed the context
            Ok(res) => {
                let output = post_ctx.temp_value(res);
                if engine_ctx.is_dirty() {
                    EvalResult::DirtyContext(output)
                } else {
                    EvalResult::NoModification(output)
                }
            }
            Err(err) => {
                debug!(
                    "Failed to evaluate {}. error: {}",
                    self.full_method_name, err
                );
                EvalResult::None
            }
        }
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), self.arg_types.len());
        if self.is_static {
            static_member_call_ast(self.obj_type.as_str(), self.method_name.as_str(), children)
        } else {
            member_call_ast(self.method_name.as_str(), children)
        }
    }
}

impl std::fmt::Debug for ClassMethodOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClassMethodOp")
            .field("method_name", &self.method_name)
            .field("is_static", &self.is_static)
            .field("arg_types", &self.arg_types)
            .finish()
    }
}
