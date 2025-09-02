use opcode::EvalResult;
use ruse_object_graph::value::*;
use ruse_object_graph::*;
use ruse_synthesizer::location::*;
use ruse_synthesizer::opcode::ExprOpcode;
use ruse_synthesizer::*;
use ruse_synthesizer::{context::*, opcode::ExprAst};
use swc_common::{util::take::Take, DUMMY_SP};
use swc_ecma_ast as ast;

use super::TsExprAst;

#[derive(Debug)]
pub enum LitOp {
    Null,
    Str(StringValue),
    Bool(bool),
    Num(Number),
}

impl ExprOpcode for LitOp {
    fn op_name(&self) -> &str {
        match self {
            LitOp::Null => "NullLiteral",
            LitOp::Str(_) => "StringLiteral",
            LitOp::Bool(_) => "BoolLiteral",
            LitOp::Num(_) => "NumLiteral",
        }
    }

    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        _syn_ctx: &SynthesizerContext,
        _worker_ctx: &mut SynthesizerWorkerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 0);
        let val = match self {
            LitOp::Null => Value::Null,
            LitOp::Str(s) => vstr!(s.as_str()),
            LitOp::Bool(b) => vbool!(*b),
            LitOp::Num(n) => vnum!(*n),
        };

        pure!(post_ctx.temp_value(val))
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), 0);

        let expr = match self {
            LitOp::Null => ast::Lit::Null(ast::Null::dummy()),
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

        TsExprAst::create(ast::Expr::Lit(expr))
    }

    fn arg_types(&self) -> &[ValueType] {
        &[]
    }
}

#[derive(Debug)]
pub struct ArrayLitOp {
    elem_type: ValueType,
    arg_types: Vec<ValueType>,
}

impl ArrayLitOp {
    pub fn new(elem_type: ValueType, size: usize) -> Self {
        Self {
            elem_type: elem_type.clone(),
            arg_types: vec![elem_type; size],
        }
    }
}

impl ExprOpcode for ArrayLitOp {
    fn op_name(&self) -> &str {
        "ArrayLiteral"
    }

    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        _syn_ctx: &SynthesizerContext,
        _worker_ctx: &mut SynthesizerWorkerContext,
    ) -> EvalResult {
        let values = args.iter().map(|val| (val.val().clone()));

        let arr = post_ctx.create_output_array_object(&self.elem_type, values);

        pure!(post_ctx.temp_value(Value::Object(arr)))
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        let expr = ast::ArrayLit {
            span: DUMMY_SP,
            elems: children
                .iter()
                .map(|x| {
                    Some(ast::ExprOrSpread {
                        spread: None,
                        expr: TsExprAst::from(x.as_ref()).node.to_owned(),
                    })
                })
                .collect(),
        };

        TsExprAst::create(ast::Expr::Array(expr))
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}
