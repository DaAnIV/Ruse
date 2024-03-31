use ruse_object_graph::{Cache, Number, PrimitiveValue};
use ruse_synthesizer::opcode::SynthesizerExprOpcode;
use ruse_synthesizer::{context::*, vbool, vnum};
use ruse_synthesizer::{temp_val, value::*};

use swc_common::DUMMY_SP;
use swc_ecma_ast as ast;

use super::TsExprAst;

pub struct UnaryOp {
    pub op: ast::UnaryOp,
    arg_types: [ValueType; 1],
}

pub struct UpdateOp {
    pub op: ast::UpdateOp,
    pub prefix: bool,
}

impl UnaryOp {
    pub fn new(op: ast::UnaryOp, value_type: ValueType) -> Self {
        Self {
            op: op,
            arg_types: [value_type],
        }
    }

    fn eval_unary_num(&self, n: &Number) -> Value {
        match self.op {
            ast::UnaryOp::Minus => vnum!(Number(-n.0)),
            ast::UnaryOp::Plus => vnum!(n.clone()),
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

impl SynthesizerExprOpcode<TsExprAst> for UnaryOp {
    fn eval(&self, ctx: &Context, args: &[&LocValue], _cache: &mut Cache) -> (Context, LocValue) {
        debug_assert_eq!(args.len(), 1);
        let res = match &args[0].val {
            Value::Primitive(p) => match p {
                PrimitiveValue::Number(n) => self.eval_unary_num(n),
                PrimitiveValue::Bool(b) => self.eval_unary_bool(*b),
                _ => unreachable!(),
            },
            Value::Object(_) => todo!(),
        };

        (ctx.clone(), temp_val!(res))
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }

    fn to_ast(&self, children: &Vec<TsExprAst>) -> TsExprAst {
        debug_assert_eq!(children.len(), 1);

        let expr = ast::UnaryExpr {
            span: DUMMY_SP,
            op: self.op,
            arg: children[0].node.to_owned(),
        };

        ast::Expr::Unary(expr).into()
    }
}

impl UpdateOp {
    pub fn new(op: ast::UpdateOp, prefix: bool) -> Self {
        Self {
            op: op,
            prefix: prefix,
        }
    }
}

impl SynthesizerExprOpcode<TsExprAst> for UpdateOp {
    fn eval(&self, ctx: &Context, args: &[&LocValue], _cache: &mut Cache) -> (Context, LocValue) {
        debug_assert_eq!(args.len(), 1);

        let n = args[0].val.primitive().unwrap().number().unwrap();

        let res = match self.op {
            ast::UpdateOp::PlusPlus => vnum!(Number(n.0 + 1f64)),
            ast::UpdateOp::MinusMinus => vnum!(Number(n.0 - 1f64)),
        };

        let new_ctx = match args[0].loc.is_temp() {
            true => ctx.clone(),
            false => ctx.update_value(&args[0].loc, res.clone()),
        };

        match self.prefix {
            true => (new_ctx, temp_val!(res)),
            false => (new_ctx, args[0].clone()),
        }
    }

    fn arg_types(&self) -> &[ValueType] {
        &[ValueType::Number]
    }

    fn to_ast(&self, children: &Vec<TsExprAst>) -> TsExprAst {
        debug_assert_eq!(children.len(), 1);

        let expr = ast::UpdateExpr {
            span: DUMMY_SP,
            op: self.op,
            arg: children[0].node.to_owned(),
            prefix: self.prefix
        };

        ast::Expr::Update(expr).into()
    }
}
