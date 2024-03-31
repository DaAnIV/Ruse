use std::sync::Arc;

use ruse_synthesizer::opcode::ExprAst;
use swc::PrintArgs;
use swc_common::SourceMap;
use swc_ecma_ast as ast;

pub struct TsExprAst {
    pub node: Box<ast::Expr>,
}

impl From<ast::Expr> for TsExprAst {
    fn from(value: ast::Expr) -> Self {
        TsExprAst { node: value.into() }
    }
}

impl ExprAst for TsExprAst {
    fn to_string(&self) -> String {
        let c = swc::Compiler::new(Arc::<SourceMap>::default());
        c.print(&self.node, PrintArgs::default())
            .expect("Failed to get code")
            .code
    }
}

impl Default for TsExprAst {
    fn default() -> Self {
        Self {
            node: ast::Expr::Invalid(ast::Invalid::default()).into(),
        }
    }
}

mod bin;
mod unary;
mod ident;
mod lit;
mod member;

pub use bin::*;
pub use unary::*;
pub use ident::*;
pub use lit::*;
pub use member::*;
