use std::{
    ops::DerefMut,
    sync::Arc,
};

use ruse_object_graph::{Cache, CachedString};
use ruse_synthesizer::{
    context::Context,
    opcode::{ExprAst, ExprOpcode},
    value::{LocValue, ValueType},
};

use swc_common::DUMMY_SP;
use swc_ecma_ast as ast;

use crate::{
    js_value::{js_value_to_value, value_to_js_value},
    opcode::TsExprAst,
    ts_class::{TsClasses, TsGlobalObject},
};

pub struct ClassMethodOp {
    classes: TsClasses,
    method_name: String,
    arg_types: Vec<ValueType>,
    code: String,
}

impl ClassMethodOp {
    pub fn new(
        obj_type: CachedString,
        method_name: String,
        args: &[(&str, ValueType)],
        function_body: &str,
        classes: TsClasses,
    ) -> Self {
        let mut arg_types = vec![ValueType::Object(obj_type)];
        arg_types.extend(args.iter().map(|(_, value_type)| value_type.clone()));
        let caller_args = (0..(arg_types.len() - 1))
            .map(|i| format!("arg {}", i + 1))
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
            method_name: method_name,
            arg_types: arg_types,
            code: code,
            classes: classes,
        }
    }

    fn get_boa_ctx(&self, post_ctx: &mut Context, cache: &Arc<Cache>) -> boa_engine::Context {
        let boa_ctx = self.classes.create_boa_ctx();
        let global_obj = boa_ctx.global_object();
        let mut a = global_obj.downcast_mut::<TsGlobalObject>().unwrap();
        let b = a.deref_mut();
        b.cache = Some(cache.clone());
        b.context = Some(post_ctx.clone());

        boa_ctx
    }
}

impl ExprOpcode for ClassMethodOp {
    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }

    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        cache: &Arc<Cache>,
    ) -> Option<LocValue> {
        let mut boa_ctx = self.get_boa_ctx(post_ctx, cache);
        for (i, arg) in args.iter().enumerate() {
            let key = boa_engine::js_string!(format!("arg{}", i));
            let value = value_to_js_value(&self.classes, arg.val(), &mut boa_ctx, cache);
            if boa_ctx
                .register_global_property(key, value, boa_engine::property::Attribute::all())
                .is_err()
            {
                return None;
            }
        }
        let js_souce = boa_engine::Source::from_bytes(self.code.as_str());
        match boa_ctx.eval(js_souce) {
            Ok(res) => Some(post_ctx.temp_value(js_value_to_value(
                &self.classes,
                &res,
                &mut boa_ctx,
                cache,
            ))),
            Err(_) => None,
        }
    }

    fn to_ast(&self, children: &Vec<Box<dyn ExprAst>>) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), self.arg_types.len());

        let member = ast::MemberExpr {
            span: DUMMY_SP,
            obj: TsExprAst::from(children[0].as_ref()).get_paren_expr(),
            prop: ast::MemberProp::Ident(self.method_name.as_str().into()),
        };
        let call_expr = ast::CallExpr {
            span: DUMMY_SP,
            callee: ast::Callee::Expr(Box::new(member.into())),
            args: children
                .iter()
                .skip(1)
                .map(|x| ast::ExprOrSpread {
                    spread: None,
                    expr: TsExprAst::from(x.as_ref()).get_paren_expr(),
                })
                .collect(),
            type_args: None,
        };

        TsExprAst::create(ast::Expr::Call(call_expr))
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
