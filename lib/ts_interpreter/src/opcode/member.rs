use ruse_object_graph::CachedString;
use ruse_object_graph::value::*;
use ruse_synthesizer::context::*;
use ruse_synthesizer::location::*;
use ruse_synthesizer::opcode::{EvalResult, ExprAst, ExprOpcode};

use swc_common::DUMMY_SP;
use swc_ecma_ast as ast;

use super::TsExprAst;

#[derive(Debug)]
pub struct MemberOp {
    arg_types: [ValueType; 1],
    field_name: CachedString,
}

impl MemberOp {
    pub fn new(obj_type: CachedString, field_name: CachedString) -> Self {
        Self {
            arg_types: [ValueType::Object(obj_type)],
            field_name,
        }
    }
}

impl ExprOpcode for MemberOp {
    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        _: &SynthesizerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 1);

        args[0].get_obj_field_loc_value(&post_ctx.graphs_map, &self.field_name).into()
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), 1);

        let expr = ast::MemberExpr {
            span: DUMMY_SP,
            obj: TsExprAst::from(children[0].as_ref()).get_paren_expr(),
            prop: ast::MemberProp::Ident(ast::IdentName::from(self.field_name.as_str())),
        };

        TsExprAst::create(ast::Expr::Member(expr))
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}
