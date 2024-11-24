use ruse_object_graph::value::*;
use ruse_object_graph::{vbool, vcstring, vnum, Number, PrimitiveValue};
use ruse_synthesizer::context::*;
use ruse_synthesizer::location::*;
use ruse_synthesizer::opcode::{EvalResult, ExprAst, ExprOpcode};

use swc_common::DUMMY_SP;
use swc_ecma_ast as ast;

use super::TsExprAst;

#[derive(Debug)]
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

    fn eval_bin_str(&self, s1: &String, s2: &String, syn_ctx: &SynthesizerContext) -> Value {
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
}

impl ExprOpcode for BinOp {
    fn op_name(&self) -> &str {
        match self.op {
            ast::BinaryOp::EqEq => "Binary EqEq",
            ast::BinaryOp::NotEq => "Binary NotEq",
            ast::BinaryOp::EqEqEq => "Binary EqEqEq",
            ast::BinaryOp::NotEqEq => "Binary NotEqEq",
            ast::BinaryOp::Lt => "Binary Lt",
            ast::BinaryOp::LtEq => "Binary LtEq",
            ast::BinaryOp::Gt => "Binary Gt",
            ast::BinaryOp::GtEq => "Binary GtEq",
            ast::BinaryOp::LShift => "Binary LShift",
            ast::BinaryOp::RShift => "Binary RShift",
            ast::BinaryOp::ZeroFillRShift => "Binary ZeroFillRShift",
            ast::BinaryOp::Add => "Binary Add",
            ast::BinaryOp::Sub => "Binary Sub",
            ast::BinaryOp::Mul => "Binary Mul",
            ast::BinaryOp::Div => "Binary Div",
            ast::BinaryOp::Mod => "Binary Mod",
            ast::BinaryOp::BitOr => "Binary BitOr",
            ast::BinaryOp::BitXor => "Binary BitXor",
            ast::BinaryOp::BitAnd => "Binary BitAnd",
            ast::BinaryOp::LogicalOr => "Binary LogicalOr",
            ast::BinaryOp::LogicalAnd => "Binary LogicalAnd",
            ast::BinaryOp::In => "Binary In",
            ast::BinaryOp::InstanceOf => "Binary InstanceOf",
            ast::BinaryOp::Exp => "Binary Exp",
            ast::BinaryOp::NullishCoalescing => "Binary NullishCoalescing",
        }
    }

    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 2);
        debug_assert_eq!(
            args[0].val().val_type(&post_ctx.graphs_map),
            self.arg_types[0]
        );
        debug_assert_eq!(
            args[1].val().val_type(&post_ctx.graphs_map),
            self.arg_types[1]
        );

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
            _ => panic!("Unexpected binary args"),
        };

        EvalResult::NoModification(post_ctx.temp_value(val))
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
