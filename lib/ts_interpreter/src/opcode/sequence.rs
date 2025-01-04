use itertools::Itertools;
use ruse_object_graph::value::ValueType;
use ruse_synthesizer::context::*;
use ruse_synthesizer::location::*;
use ruse_synthesizer::opcode::{EvalResult, ExprAst, ExprOpcode};

use swc_common::DUMMY_SP;
use swc_ecma_ast as ast;

use super::TsExprAst;

#[derive(Debug)]
pub struct SequenceOp {
    arg_types: Vec<ValueType>,
    op_name: String,
}

impl SequenceOp {
    pub fn new(arg_types: Vec<ValueType>) -> Self {
        let op_name = format!(
            "Sequence[{}]",
            arg_types.iter().map(|x| x.to_string()).join(",")
        );
        Self { arg_types, op_name }
    }
}

impl ExprOpcode for SequenceOp {
    fn op_name(&self) -> &str {
        &self.op_name
    }

    fn eval(
        &self,
        args: &[&LocValue],
        _post_ctx: &mut Context,
        _syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        if let Some(out) = args.last() {
            EvalResult::NoModification((*out).clone())
        } else {
            EvalResult::None
        }
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        let seq_expr = ast::SeqExpr {
            span: DUMMY_SP,
            exprs: children
                .iter()
                .map(|x| TsExprAst::from(x.as_ref()).node.to_owned())
                .collect(),
        };

        TsExprAst::create(ast::Expr::Seq(seq_expr))
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }

    fn is_terminal(&self) -> bool {
        true
    }
}
