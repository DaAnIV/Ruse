use ruse_object_graph::CachedString;
use ruse_synthesizer::context::*;
use ruse_synthesizer::opcode::{EvalResult, ExprAst, ExprOpcode};
use ruse_synthesizer::value::*;

use swc_common::DUMMY_SP;
use swc_ecma_ast as ast;

use super::TsExprAst;

#[derive(Debug)]
pub struct IdentOp {
    pub name: CachedString,
    required_args: [CachedString; 1],
}

impl IdentOp {
    pub fn new(var_name: CachedString) -> Self {
        Self {
            name: var_name.clone(),
            required_args: [var_name],
        }
    }
}

impl ExprOpcode for IdentOp {
    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        _: &SynthesizerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 0);

        post_ctx.get_var_loc_value(&self.name).into()
    }

    fn to_ast(&self, children: &Vec<Box<dyn ExprAst>>) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), 0);

        let expr = ast::Ident {
            span: DUMMY_SP,
            sym: self.name.as_str().into(),
            optional: false,
        };

        TsExprAst::create(ast::Expr::Ident(expr))
    }

    fn arg_types(&self) -> &[ValueType] {
        &[]
    }

    fn required_variables(&self) -> &[CachedString] {
        &self.required_args
    }
}
