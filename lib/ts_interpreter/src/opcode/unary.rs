use ruse_object_graph::{value::*, *};
use ruse_object_graph::{Number, PrimitiveValue};
use ruse_synthesizer::context::*;
use ruse_synthesizer::location::*;
use ruse_synthesizer::opcode::{EvalResult, ExprAst, ExprOpcode};

use swc_common::DUMMY_SP;
use swc_ecma_ast as ast;

use super::TsExprAst;

#[derive(Debug)]
pub struct UnaryOp {
    pub op: ast::UnaryOp,
    arg_types: [ValueType; 1],
}

#[derive(Debug)]
pub struct UpdateOp {
    pub op: ast::UpdateOp,
    pub prefix: bool,
}

impl UnaryOp {
    pub fn new(op: ast::UnaryOp, value_type: ValueType) -> Self {
        Self {
            op,
            arg_types: [value_type],
        }
    }

    fn eval_unary_num(&self, n: &Number) -> Value {
        match self.op {
            ast::UnaryOp::Minus => vnum!(Number(-n.0)),
            ast::UnaryOp::Plus => vnum!(*n),
            ast::UnaryOp::Tilde => vnum!(Number::from(!(n.0.floor() as u64))),
            _ => unreachable!(),
        }
    }

    fn eval_unary_bool(&self, b: bool) -> Value {
        match self.op {
            ast::UnaryOp::Bang => vbool!(!b),
            _ => unreachable!(),
        }
    }
}

impl ExprOpcode for UnaryOp {
    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        _: &SynthesizerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 1);
        let res = match &args[0].val() {
            Value::Primitive(p) => match p {
                PrimitiveValue::Number(n) => self.eval_unary_num(n),
                PrimitiveValue::Bool(b) => self.eval_unary_bool(*b),
                _ => unreachable!(),
            },
            Value::Object(_) => todo!(),
        };

        EvalResult::NoModification(post_ctx.temp_value(res))
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), 1);

        let expr = ast::UnaryExpr {
            span: DUMMY_SP,
            op: self.op,
            arg: TsExprAst::from(children[0].as_ref()).get_paren_expr(),
        };

        TsExprAst::create(ast::Expr::Unary(expr))
    }
}

impl UpdateOp {
    pub fn new(op: ast::UpdateOp, prefix: bool) -> Self {
        Self { op, prefix }
    }
}

impl ExprOpcode for UpdateOp {
    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 1);

        if args[0].loc().is_temp() {
            return EvalResult::None;
        }
        let n = args[0].val().primitive().unwrap().number().unwrap();

        let res = match self.op {
            ast::UpdateOp::PlusPlus => vnum!(Number(n.0 + 1f64)),
            ast::UpdateOp::MinusMinus => vnum!(Number(n.0 - 1f64)),
        };

        let mut loc = args[0].loc().clone();
        if !post_ctx.update_value(&res.clone(), &mut loc, syn_ctx) {
            return EvalResult::None;
        }

        EvalResult::DirtyContext(match self.prefix {
            true => post_ctx.temp_value(res),
            false => post_ctx.temp_value(args[0].val().clone()),
        })
    }

    fn arg_types(&self) -> &[ValueType] {
        &[ValueType::Number]
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), 1);

        let expr = ast::UpdateExpr {
            span: DUMMY_SP,
            op: self.op,
            arg: TsExprAst::from(children[0].as_ref()).get_paren_expr(),
            prefix: self.prefix,
        };

        TsExprAst::create(ast::Expr::Update(expr))
    }
}
