use ruse_object_graph::*;
use ruse_synthesizer::context::*;
use ruse_synthesizer::opcode::ExprOpcode;
use ruse_synthesizer::value::*;
use ruse_synthesizer::*;
use swc_common::{util::take::Take, DUMMY_SP};
use swc_ecma_ast as ast;

use super::TsExprAst;

#[derive(Debug)]
pub enum LitOp {
    Null,
    Str(CachedString),
    Bool(bool),
    Num(Number),
}

#[derive(Debug)]
pub struct ArrayLitOp {
    pub size: u32,
}

impl ExprOpcode<TsExprAst> for LitOp {
    fn eval(&self, args: &[&LocValue], post_ctx: &mut Context, _cache: &Cache) -> Option<LocValue> {
        debug_assert_eq!(args.len(), 0);
        let val = match self {
            LitOp::Null => Value::Primitive(PrimitiveValue::Null),
            LitOp::Str(s) => vcstring!(s.clone()),
            LitOp::Bool(b) => vbool!(*b),
            LitOp::Num(n) => vnum!(*n),
        };

        Some(post_ctx.temp_value(val))
    }

    fn to_ast(&self, children: &Vec<TsExprAst>) -> TsExprAst {
        debug_assert_eq!(children.len(), 0);

        let expr = match self {
            LitOp::Null => ast::Lit::Null(ast::Null::dummy()).into(),
            LitOp::Str(s) => ast::Lit::Str(ast::Str {
                span: DUMMY_SP,
                value: s.as_str().into(),
                raw: None,
            }),
            LitOp::Bool(b) => ast::Lit::Bool(ast::Bool {
                span: DUMMY_SP,
                value: *b,
            }),
            LitOp::Num(n) => ast::Lit::Num(ast::Number {
                span: DUMMY_SP,
                value: n.0,
                raw: None,
            }),
        };

        ast::Expr::Lit(expr).into()
    }

    fn arg_types(&self) -> &[ValueType] {
        &[]
    }
}

impl ExprOpcode<TsExprAst> for ArrayLitOp {
    fn eval(&self, args: &[&LocValue], post_ctx: &mut Context, cache: &Cache) -> Option<LocValue> {
        let kv_map = (0..self.size)
            .zip(args)
            .map(|(i, val)| (scached!(cache; i.to_string()), val.val().clone()))
            .collect();

        Some(post_ctx.temp_value(Value::generate_object_from_map(
            cache.temp_string(),
            str_cached!(cache; "Array"),
            kv_map,
        )))
    }

    fn to_ast(&self, children: &Vec<TsExprAst>) -> TsExprAst {
        let expr = ast::ArrayLit {
            span: DUMMY_SP,
            elems: children
                .into_iter()
                .map(|x| {
                    Some(ast::ExprOrSpread {
                        spread: None,
                        expr: x.node.to_owned(),
                    })
                })
                .collect(),
        };

        ast::Expr::Array(expr).into()
    }

    fn arg_types(&self) -> &[ValueType] {
        &[]
    }
}
