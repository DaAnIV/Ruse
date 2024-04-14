use ruse_object_graph::{Cache, Number, PrimitiveValue};
use ruse_synthesizer::opcode::SynthesizerExprOpcode;
use ruse_synthesizer::value::*;
use ruse_synthesizer::{context::*, vbool, vnum, vstring};

use swc_common::DUMMY_SP;
use swc_ecma_ast as ast;

use super::TsExprAst;

pub struct BinOp {
    pub op: ast::BinaryOp,
    pub arg_types: [ValueType; 2],
}

impl BinOp {
    fn eval_bin_num(&self, n1: &Number, n2: &Number) -> Value {
        match self.op {
            ast::BinaryOp::EqEq => vbool!(n1 == n2),
            ast::BinaryOp::NotEq => vbool!(n1 != n2),
            ast::BinaryOp::EqEqEq => vbool!(n1 == n2),
            ast::BinaryOp::NotEqEq => vbool!(n1 != n2),
            ast::BinaryOp::Lt => vbool!(n1 < n2),
            ast::BinaryOp::LtEq => vbool!(n1 <= n2),
            ast::BinaryOp::Gt => vbool!(n1 > n2),
            ast::BinaryOp::GtEq => vbool!(n1 >= n2),
            ast::BinaryOp::LShift => {
                let res = (n1.0.floor() as i64).overflowing_shl(n1.0.floor() as u32).0;
                vnum!(Number::from(res))
            }
            ast::BinaryOp::RShift => {
                let res = (n1.0.floor() as i64).overflowing_shr(n1.0.floor() as u32).0;
                vnum!(Number::from(res))
            }
            ast::BinaryOp::ZeroFillRShift => {
                let res = (n1.0.floor() as u64).overflowing_shr(n1.0.floor() as u32).0;
                vnum!(Number::from(res))
            }
            ast::BinaryOp::Add => vnum!(Number::from(n1.0 + n2.0)),
            ast::BinaryOp::Sub => vnum!(Number::from(n1.0 - n2.0)),
            ast::BinaryOp::Mul => vnum!(Number::from(n1.0 * n2.0)),
            ast::BinaryOp::Div => vnum!(Number::from(n1.0 / n2.0)),
            ast::BinaryOp::Mod => vnum!(Number::from(n1.0 % n2.0)),
            ast::BinaryOp::BitOr => {
                let res = (n1.0.floor() as u64) | (n1.0.floor() as u64);
                vnum!(Number::from(res))
            }
            ast::BinaryOp::BitXor => {
                let res = (n1.0.floor() as u64) ^ (n1.0.floor() as u64);
                vnum!(Number::from(res))
            }
            ast::BinaryOp::BitAnd => {
                let res = (n1.0.floor() as u64) & (n1.0.floor() as u64);
                vnum!(Number::from(res))
            }
            ast::BinaryOp::Exp => vnum!(Number::from(n1.0.powf(n2.0))),
            _ => unreachable!(),
        }
    }

    fn eval_bin_bool(&self, b1: bool, b2: bool) -> Value {
        match self.op {
            ast::BinaryOp::EqEq => vbool!(b1 == b2),
            ast::BinaryOp::NotEq => vbool!(b1 != b2),
            ast::BinaryOp::EqEqEq => vbool!(b1 == b2),
            ast::BinaryOp::NotEqEq => vbool!(b1 != b2),
            ast::BinaryOp::LogicalOr => vbool!(b1 || b2),
            ast::BinaryOp::LogicalAnd => vbool!(b1 && b2),
            _ => unreachable!(),
        }
    }

    fn eval_bin_str(&self, s1: &String, s2: &String, cache: &Cache) -> Value {
        match self.op {
            ast::BinaryOp::EqEq => vbool!(s1 == s2),
            ast::BinaryOp::NotEq => vbool!(s1 != s2),
            ast::BinaryOp::EqEqEq => vbool!(s1 == s2),
            ast::BinaryOp::NotEqEq => vbool!(s1 != s2),
            ast::BinaryOp::Lt => vbool!(s1 < s2),
            ast::BinaryOp::LtEq => vbool!(s1 <= s2),
            ast::BinaryOp::Gt => vbool!(s1 > s2),
            ast::BinaryOp::GtEq => vbool!(s1 >= s2),
            ast::BinaryOp::Add => {
                let mut value = String::with_capacity(s1.len() + s2.len());
                value.push_str(s1);
                value.push_str(s2);
                vstring!(cache; value)
            }
            _ => unreachable!(),
        }
    }
}

impl SynthesizerExprOpcode<TsExprAst> for BinOp {
    fn eval(&self, ctx: &mut Context, args: &[&LocValue], cache: &Cache) -> LocValue {
        debug_assert_eq!(args.len(), 2);
        debug_assert_eq!(args[0].val().val_type(), self.arg_types[0]);
        debug_assert_eq!(args[1].val().val_type(), self.arg_types[1]);

        let val = match (&args[0].val(), &args[1].val()) {
            (Value::Primitive(p1), Value::Primitive(p2)) => match (p1, p2) {
                (PrimitiveValue::Number(n1), PrimitiveValue::Number(n2)) => {
                    self.eval_bin_num(n1, n2)
                }
                (PrimitiveValue::Bool(b1), PrimitiveValue::Bool(b2)) => {
                    self.eval_bin_bool(*b1, *b2)
                }
                (PrimitiveValue::String(s1), PrimitiveValue::String(s2)) => {
                    self.eval_bin_str(s1, s2, cache)
                }
                (_, _) => unreachable!(),
            },
            (Value::Object(_), Value::Primitive(_)) => todo!(),
            (Value::Object(_), Value::Object(_)) => todo!(),
            _ => panic!("Unexpected binary args"),
        };

        ctx.temp_value(val)
    }

    fn to_ast(&self, children: &Vec<TsExprAst>) -> TsExprAst {
        debug_assert_eq!(children.len(), 2);

        let expr = ast::BinExpr {
            span: DUMMY_SP,
            op: self.op,
            left: children[0].get_paren_expr(),
            right: children[1].get_paren_expr(),
        };

        ast::Expr::Bin(expr).into()
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}
