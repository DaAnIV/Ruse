use ruse_object_graph::{value::*, *};
use ruse_synthesizer::{
    context::{Context, SynthesizerContext},
    location::*,
    opcode::{EvalResult, ExprAst, ExprOpcode},
};

use crate::{
    js_value::{js_value_to_value, value_to_js_value},
    opcode::member_call_ast,
    ts_class::TsClasses,
};

pub struct ClassMethodOp {
    classes: TsClasses,
    method_name: String,
    full_method_name: String,
    arg_types: Vec<ValueType>,
    code: String,
}

impl ClassMethodOp {
    pub fn new(
        obj_type: CachedString,
        method_name: String,
        args: &[(String, ValueType)],
        function_body: &str,
        classes: TsClasses,
    ) -> Self {
        let full_method_name = format!("{}.{}", &obj_type, &method_name);
        let mut arg_types = vec![ValueType::Object(obj_type)];
        arg_types.extend(args.iter().map(|(_, value_type)| value_type.clone()));
        let caller_args = (0..(arg_types.len() - 1))
            .map(|i| format!("arg{}", i + 1))
            .collect::<Vec<_>>()
            .join(", ");
        let args_names = args
            .iter()
            .map(|(name, _)| name.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let code = format!(
            "function func({}) {}\nfunc.call(arg0, {});",
            args_names, function_body, caller_args
        );
        Self {
            method_name,
            full_method_name,
            arg_types,
            code,
            classes,
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
        let mut boa_ctx = self.classes.get_engine_ctx(post_ctx, &syn_ctx.cache);
        for (i, arg) in args.iter().enumerate() {
            let key = boa_engine::js_string!(format!("arg{}", i));
            let value = value_to_js_value(
                &self.classes,
                arg.val(),
                &mut boa_ctx,
                post_ctx,
                &syn_ctx.cache,
            );
            if boa_ctx
                .register_global_property(key, value, boa_engine::property::Attribute::all())
                .is_err()
            {
                return EvalResult::None;
            }
        }
        let js_souce = boa_engine::Source::from_bytes(self.code.as_str());
        match boa_ctx.eval(js_souce) {
            // Need to check if func changed the context
            Ok(res) => {
                let output = post_ctx.temp_value(js_value_to_value(
                &self.classes,
                &res,
                &mut boa_ctx,
                &syn_ctx.cache,
            ));
            if boa_ctx.is_dirty() {
                EvalResult::DirtyContext(output)
            } else {
                EvalResult::NoModification(output)
            }
        },
            Err(_) => EvalResult::None,
        }
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), self.arg_types.len());
        member_call_ast(self.method_name.as_str(), children)
    }
}

impl std::fmt::Debug for ClassMethodOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClassMethodOp")
            .field("method_name", &self.method_name)
            .field("arg_types", &self.arg_types)
            .field("code", &self.code)
            .finish()
    }
}
