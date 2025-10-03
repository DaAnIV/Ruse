use ruse_object_graph::{location::*, value::*, *};
use ruse_object_graph::{Number, PrimitiveValue};
use ruse_synthesizer::opcode::{EvalResult, ExprAst, ExprOpcode};
use ruse_synthesizer::{context::*, dirty, synthesizer_context::*};

use swc_common::DUMMY_SP;
use swc_ecma_ast as ast;

use super::TsExprAst;

#[derive(Debug)]
pub struct AssignOp {
    pub op: ast::AssignOp,
    op_name: String,
    arg_types: [ValueType; 2],
}

impl AssignOp {
    pub fn new(op: ast::AssignOp, value_type: ValueType) -> Self {
        Self {
            op,
            op_name: Self::get_op_name(&op, &value_type),
            arg_types: [value_type.clone(), value_type],
        }
    }

    fn eval_assign_num(&self, lhs: Number, rhs: Number) -> Result<Value, ()> {
        let new_num = match self.op {
            ast::AssignOp::Assign => rhs,
            ast::AssignOp::AddAssign => Number::from(lhs.0 + rhs.0),
            ast::AssignOp::SubAssign => Number::from(lhs.0 - rhs.0),
            ast::AssignOp::MulAssign => Number::from(lhs.0 * rhs.0),
            ast::AssignOp::DivAssign => Number::from(lhs.0 / rhs.0),
            ast::AssignOp::ModAssign => Number::from(lhs.0 % rhs.0),
            ast::AssignOp::LShiftAssign => Number::from(
                (lhs.0.floor() as i64)
                    .overflowing_shl(rhs.0.floor() as u32)
                    .0,
            ),
            ast::AssignOp::RShiftAssign => Number::from(
                (lhs.0.floor() as i64)
                    .overflowing_shr(rhs.0.floor() as u32)
                    .0,
            ),
            ast::AssignOp::ZeroFillRShiftAssign => Number::from(
                (lhs.0.floor() as u64)
                    .overflowing_shr(rhs.0.floor() as u32)
                    .0,
            ),
            ast::AssignOp::BitOrAssign => {
                Number::from((lhs.0.floor() as u64) | (rhs.0.floor() as u64))
            }
            ast::AssignOp::BitXorAssign => {
                Number::from((lhs.0.floor() as u64) ^ (rhs.0.floor() as u64))
            }
            ast::AssignOp::BitAndAssign => {
                Number::from((lhs.0.floor() as u64) & (rhs.0.floor() as u64))
            }
            ast::AssignOp::ExpAssign => Number::from(lhs.0.powf(rhs.0)),
            _ => return Err(()),
        };

        Ok(vnum!(new_num))
    }

    fn eval_assign_str(&self, lhs: &str, rhs: &str) -> Result<Value, ()> {
        match self.op {
            ast::AssignOp::Assign => Ok(vstr!(rhs)),
            ast::AssignOp::AddAssign => {
                let mut new_string = String::with_capacity(lhs.len() + rhs.len());
                new_string.push_str(lhs);
                new_string.push_str(rhs);
                Ok(vstr!(new_string.as_str()))
            }
            _ => Err(()),
        }
    }

    fn eval_assign_bool(&self, lhs: bool, rhs: bool) -> Result<Value, ()> {
        let new_bool = match self.op {
            ast::AssignOp::AndAssign => lhs && rhs,
            ast::AssignOp::OrAssign => lhs || rhs,
            _ => return Err(()),
        };

        return Ok(vbool!(new_bool));
    }

    fn get_op_name(op: &ast::AssignOp, value_type: &ValueType) -> String {
        match op {
            ast::AssignOp::Assign => format!("Assign [{}]", value_type),
            ast::AssignOp::AddAssign => format!("AddAssign [{}]", value_type),
            ast::AssignOp::SubAssign => format!("SubAssign [{}]", value_type),
            ast::AssignOp::MulAssign => format!("MulAssign [{}]", value_type),
            ast::AssignOp::DivAssign => format!("DivAssign [{}]", value_type),
            ast::AssignOp::ModAssign => format!("ModAssign [{}]", value_type),
            ast::AssignOp::LShiftAssign => format!("LShiftAssign [{}]", value_type),
            ast::AssignOp::RShiftAssign => format!("RShiftAssign [{}]", value_type),
            ast::AssignOp::ZeroFillRShiftAssign => format!("ZeroFillRShiftAssign [{}]", value_type),
            ast::AssignOp::BitOrAssign => format!("BitOrAssign [{}]", value_type),
            ast::AssignOp::BitXorAssign => format!("BitXorAssign [{}]", value_type),
            ast::AssignOp::BitAndAssign => format!("BitAndAssign [{}]", value_type),
            ast::AssignOp::ExpAssign => format!("ExpAssign [{}]", value_type),

            ast::AssignOp::AndAssign => format!("AndAssign [{}]", value_type),
            ast::AssignOp::OrAssign => format!("OrAssign [{}]", value_type),
            ast::AssignOp::NullishAssign => format!("NullishAssign [{}]", value_type),
        }
    }
}

impl ExprOpcode for AssignOp {
    fn op_name(&self) -> &str {
        &self.op_name
    }

    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        syn_ctx: &SynthesizerContext,
        _worker_ctx: &mut SynthesizerWorkerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 2);

        let lhs = args[1];
        let rhs = args[0];

        if lhs.loc.is_temp() {
            return Err(());
        }

        let res = match (&lhs.val(), &rhs.val()) {
            (Value::Primitive(p), Value::Primitive(p2)) => match (p, p2) {
                (PrimitiveValue::Number(lhs), PrimitiveValue::Number(rhs)) => {
                    self.eval_assign_num(*lhs, *rhs)
                }
                (PrimitiveValue::Bool(lhs), PrimitiveValue::Bool(rhs)) => {
                    self.eval_assign_bool(*lhs, *rhs)
                }
                (PrimitiveValue::String(lhs), PrimitiveValue::String(rhs)) => {
                    self.eval_assign_str(lhs, rhs)
                }
                _ => return Err(()),
            },
            _ => return Err(()),
        }?;

        let mut loc = lhs.loc.clone();
        post_ctx.update_value(&res, &mut loc, syn_ctx.variables())?;

        dirty!(post_ctx.temp_value(vnull!()))
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), 2);

        let left = ast::ParenExpr {
            expr: TsExprAst::from(children[1].as_ref()).node.to_owned(),
            span: DUMMY_SP,
        };

        let expr = ast::AssignExpr {
            span: DUMMY_SP,
            op: self.op,
            left: ast::AssignTarget::Simple(ast::SimpleAssignTarget::Paren(left)),
            right: TsExprAst::from(children[0].as_ref()).get_paren_expr(),
        };

        TsExprAst::create(ast::Expr::Assign(expr))
    }
}
