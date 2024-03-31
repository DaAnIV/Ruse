use std::sync::Arc;

use ruse_object_graph::Cache;
use ruse_synthesizer::context::*;
use ruse_synthesizer::opcode::SynthesizerExprOpcode;
use ruse_synthesizer::value::*;

use swc_common::DUMMY_SP;
use swc_ecma_ast as ast;

use super::TsExprAst;

pub struct IdentOp {
    pub name: Arc<String>,
}

impl SynthesizerExprOpcode<TsExprAst> for IdentOp {
    fn eval(&self, ctx: &Context, args: &[&LocValue], _cache: &mut Cache) -> (Context, LocValue) {
        debug_assert_eq!(args.len(), 0);

        let value = ctx.get_var_value(&self.name);

        (
            ctx.clone(),
            LocValue {
                val: value,
                loc: Location::Var(VarLoc {
                    var: self.name.clone(),
                }),
            },
        )
    }

    fn to_ast(&self, children: &Vec<TsExprAst>) -> TsExprAst {
        debug_assert_eq!(children.len(), 0);

        let expr = ast::Ident {
            span: DUMMY_SP,
            sym: self.name.as_str().into(),
            optional: false,
        };

        ast::Expr::Ident(expr).into()
    }
    
    fn arg_types(&self) -> &[ValueType] {
        &[]
    }
}
