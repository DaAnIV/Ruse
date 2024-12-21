use std::{any::Any, sync::Arc};

use ruse_synthesizer::opcode::ExprAst;
use swc::PrintArgs;
use swc_common::{SourceMap, DUMMY_SP};
use swc_ecma_ast as ast;

pub struct TsExprAst {
    pub node: Box<ast::Expr>,
}

impl TsExprAst {
    pub fn create(node: ast::Expr) -> Box<dyn ExprAst> {
        Box::new(Self { node: node.into() })
    }

    pub fn get_paren_expr(&self) -> Box<ast::Expr> {
        let owned_node = self.node.to_owned();
        match *owned_node {
            ast::Expr::Unary(_) => ast::ParenExpr {
                expr: owned_node,
                span: DUMMY_SP,
            }
            .into(),
            ast::Expr::Update(_) => ast::ParenExpr {
                expr: owned_node,
                span: DUMMY_SP,
            }
            .into(),
            ast::Expr::Bin(_) => ast::ParenExpr {
                expr: owned_node,
                span: DUMMY_SP,
            }
            .into(),
            ast::Expr::Assign(_) => todo!(),
            ast::Expr::SuperProp(_) => todo!(),
            ast::Expr::Cond(_) => todo!(),
            ast::Expr::New(_) => todo!(),
            ast::Expr::Seq(_) => todo!(),
            ast::Expr::PrivateName(_) => todo!(),
            ast::Expr::OptChain(_) => todo!(),
            _ => owned_node,
        }
    }

    pub(crate) fn from(ast: &dyn ExprAst) -> &Self {
        ast.as_any().downcast_ref().unwrap()
    }
}

impl ExprAst for TsExprAst {
    fn to_string(&self) -> String {
        let c = swc::Compiler::new(Arc::<SourceMap>::default());
        c.print(&self.node, PrintArgs::default())
            .expect("Failed to get code")
            .code
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Default for TsExprAst {
    fn default() -> Self {
        Self {
            node: ast::Expr::Invalid(ast::Invalid::default()).into(),
        }
    }
}

fn member_field_ast(obj: &Box<dyn ExprAst>, field_name: &str) -> Box<dyn ExprAst> {
    let field_expr = ast::MemberExpr {
        span: DUMMY_SP,
        obj: TsExprAst::from(obj.as_ref()).get_paren_expr(),
        prop: ast::MemberProp::Ident(ast::IdentName::from(field_name)),
    };

    TsExprAst::create(ast::Expr::Member(field_expr))
}

fn member_call_ast(callee_name: &str, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
    let callee_expr = ast::MemberExpr {
        span: DUMMY_SP,
        obj: TsExprAst::from(children[0].as_ref()).get_paren_expr(),
        prop: ast::MemberProp::Ident(ast::IdentName::from(callee_name)),
    };

    let args = children
        .iter()
        .skip(1)
        .map(|x| {
            let arg_ast = TsExprAst::from(x.as_ref());
            ast::ExprOrSpread {
                spread: None,
                expr: arg_ast.node.to_owned(),
            }
        })
        .collect();

    let expr = ast::CallExpr {
        span: DUMMY_SP,
        callee: ast::Callee::Expr(ast::Expr::Member(callee_expr).into()),
        args,
        type_args: None,
        ctxt: Default::default(),
    };

    TsExprAst::create(ast::Expr::Call(expr))
}

fn get_start_index(value: isize, len: usize) -> usize {
    let ilen = len as isize;

    if value >= ilen {
        len
    } else if value < -(ilen) {
        0
    } else if value < 0 {
        (value + ilen) as usize
    } else {
        value as usize
    }
}

fn get_end_index(value: isize, len: usize) -> usize {
    let ilen = len as isize;

    if value >= ilen {
        len
    } else if value < -(ilen) {
        0
    } else if value < 0 {
        (value + ilen) as usize
    } else {
        value as usize
    }
}

mod set_ops;
mod array_ops;
mod bin;
mod dom_ops;
mod function;
mod ident;
mod lit;
mod member;
mod string_ops;
mod unary;

pub use set_ops::*;
pub use array_ops::*;
pub use bin::*;
pub use dom_ops::*;
pub use function::*;
pub use ident::*;
pub use lit::*;
pub use member::*;
pub use string_ops::*;
pub use unary::*;
