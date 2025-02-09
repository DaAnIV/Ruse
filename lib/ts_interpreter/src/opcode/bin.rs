use ruse_object_graph::value::*;
use ruse_object_graph::{vbool, vcstring, vnum, Number, PrimitiveValue};
use ruse_synthesizer::location::*;
use ruse_synthesizer::opcode::{EvalResult, ExprAst, ExprOpcode};
use ruse_synthesizer::{context::*, pure};

use swc_common::DUMMY_SP;
use swc_ecma_ast as ast;

use super::TsExprAst;

#[derive(Debug)]
pub struct BinOp {
    pub op: ast::BinaryOp,
    op_name: String,
    arg_types: [ValueType; 2],
}

impl BinOp {
    pub fn new(op: ast::BinaryOp, lh_type: ValueType, rh_type: ValueType) -> Self {
        Self {
            op,
            op_name: Self::get_op_name(&op, &lh_type, &rh_type),
            arg_types: [lh_type, rh_type],
        }
    }

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

    fn eval_bin_str(&self, s1: &str, s2: &str, syn_ctx: &SynthesizerContext) -> Value {
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
                vcstring!(syn_ctx.cached_string(&value))
            }
            _ => unreachable!(),
        }
    }

    fn get_op_name(op: &ast::BinaryOp, lh_type: &ValueType, rh_type: &ValueType) -> String {
        match op {
            ast::BinaryOp::EqEq => format!("Binary EqEq [{}, {}]", lh_type, rh_type),
            ast::BinaryOp::NotEq => format!("Binary NotEq [{}, {}]", lh_type, rh_type),
            ast::BinaryOp::EqEqEq => format!("Binary EqEqEq [{}, {}]", lh_type, rh_type),
            ast::BinaryOp::NotEqEq => format!("Binary NotEqEq [{}, {}]", lh_type, rh_type),
            ast::BinaryOp::Lt => format!("Binary Lt [{}, {}]", lh_type, rh_type),
            ast::BinaryOp::LtEq => format!("Binary LtEq [{}, {}]", lh_type, rh_type),
            ast::BinaryOp::Gt => format!("Binary Gt [{}, {}]", lh_type, rh_type),
            ast::BinaryOp::GtEq => format!("Binary GtEq [{}, {}]", lh_type, rh_type),
            ast::BinaryOp::LShift => format!("Binary LShift [{}, {}]", lh_type, rh_type),
            ast::BinaryOp::RShift => format!("Binary RShift [{}, {}]", lh_type, rh_type),
            ast::BinaryOp::ZeroFillRShift => {
                format!("Binary ZeroFillRShift [{}, {}]", lh_type, rh_type)
            }
            ast::BinaryOp::Add => format!("Binary Add [{}, {}]", lh_type, rh_type),
            ast::BinaryOp::Sub => format!("Binary Sub [{}, {}]", lh_type, rh_type),
            ast::BinaryOp::Mul => format!("Binary Mul [{}, {}]", lh_type, rh_type),
            ast::BinaryOp::Div => format!("Binary Div [{}, {}]", lh_type, rh_type),
            ast::BinaryOp::Mod => format!("Binary Mod [{}, {}]", lh_type, rh_type),
            ast::BinaryOp::BitOr => format!("Binary BitOr [{}, {}]", lh_type, rh_type),
            ast::BinaryOp::BitXor => format!("Binary BitXor [{}, {}]", lh_type, rh_type),
            ast::BinaryOp::BitAnd => format!("Binary BitAnd [{}, {}]", lh_type, rh_type),
            ast::BinaryOp::LogicalOr => format!("Binary LogicalOr [{}, {}]", lh_type, rh_type),
            ast::BinaryOp::LogicalAnd => format!("Binary LogicalAnd [{}, {}]", lh_type, rh_type),
            ast::BinaryOp::In => format!("Binary In [{}, {}]", lh_type, rh_type),
            ast::BinaryOp::InstanceOf => format!("Binary InstanceOf [{}, {}]", lh_type, rh_type),
            ast::BinaryOp::Exp => format!("Binary Exp [{}, {}]", lh_type, rh_type),
            ast::BinaryOp::NullishCoalescing => {
                format!("Binary NullishCoalescing [{}, {}]", lh_type, rh_type)
            }
        }
    }
}

impl ExprOpcode for BinOp {
    fn op_name(&self) -> &str {
        &self.op_name
    }

    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
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
                    self.eval_bin_str(s1, s2, syn_ctx)
                }
                (_, _) => unreachable!(),
            },
            (Value::Object(_), Value::Primitive(_)) => todo!(),
            (Value::Object(_), Value::Object(_)) => todo!(),
            _ => return Err(()),
        };

        pure!(post_ctx.temp_value(val))
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), 2);

        let expr = ast::BinExpr {
            span: DUMMY_SP,
            op: self.op,
            left: TsExprAst::from(children[0].as_ref()).get_paren_expr(),
            right: TsExprAst::from(children[1].as_ref()).get_paren_expr(),
        };

        TsExprAst::create(ast::Expr::Bin(expr))
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}
