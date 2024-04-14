use std::sync::Arc;

use ruse_synthesizer::opcode::ExprAst;
use swc::PrintArgs;
use swc_common::{SourceMap, DUMMY_SP};
use swc_ecma_ast as ast;

pub struct TsExprAst {
    pub node: Box<ast::Expr>,
}

impl TsExprAst {
    pub fn get_paren_expr(&self) -> Box<ast::Expr> {
        let owned_node = self.node.to_owned();
        match *owned_node {
            ast::Expr::Unary(_) => ast::ParenExpr {
                expr: owned_node,
                span: DUMMY_SP,
            }.into(),
            ast::Expr::Update(_) => ast::ParenExpr {
                expr: owned_node,
                span: DUMMY_SP,
            }.into(),
            ast::Expr::Bin(_) => ast::ParenExpr {
                expr: owned_node,
                span: DUMMY_SP,
            }.into(),
            ast::Expr::Assign(_) => todo!(),
            ast::Expr::Member(_) => todo!(),
            ast::Expr::SuperProp(_) => todo!(),
            ast::Expr::Cond(_) => todo!(),
            ast::Expr::Call(_) => todo!(),
            ast::Expr::New(_) => todo!(),
            ast::Expr::Seq(_) => todo!(),
            ast::Expr::PrivateName(_) => todo!(),
            ast::Expr::OptChain(_) => todo!(),
            _ => owned_node,
        }
    }
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
mod ident;
mod lit;
mod member;
mod unary;

pub use bin::*;
pub use ident::*;
pub use lit::*;
pub use member::*;
pub use unary::*;
