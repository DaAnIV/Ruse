use std::{any::Any, cmp::max, sync::Arc};

use num_traits::ToPrimitive;
use ruse_object_graph::Number;
use ruse_synthesizer::opcode::ExprAst;
use swc::PrintArgs;
use swc_common::{SourceMap, SyntaxContext, DUMMY_SP};
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
            ast::Expr::Assign(_) => ast::ParenExpr {
                expr: owned_node,
                span: DUMMY_SP,
            }
            .into(),
            ast::Expr::SuperProp(_) => todo!(),
            ast::Expr::Cond(_) => todo!(),
            ast::Expr::New(_) => ast::ParenExpr {
                expr: owned_node,
                span: DUMMY_SP,
            }
            .into(),
            ast::Expr::Seq(_) => ast::ParenExpr {
                expr: owned_node,
                span: DUMMY_SP,
            }
            .into(),
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

fn static_member_expr(obj_type: &str, field_name: &str) -> ast::MemberExpr {
    let obj_ident = ast::Ident {
        span: DUMMY_SP,
        sym: obj_type.into(),
        optional: false,
        ctxt: Default::default(),
    };
    ast::MemberExpr {
        span: DUMMY_SP,
        obj: ast::Expr::Ident(obj_ident).into(),
        prop: ast::MemberProp::Ident(ast::IdentName::from(field_name)),
    }
}

fn member_expr(obj: &Box<dyn ExprAst>, field_name: &str) -> ast::MemberExpr {
    ast::MemberExpr {
        span: DUMMY_SP,
        obj: TsExprAst::from(obj.as_ref()).get_paren_expr(),
        prop: ast::MemberProp::Ident(ast::IdentName::from(field_name)),
    }
}

fn new_obj_ast(obj_type: &str, args: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
    let obj_ident = ast::Ident {
        span: DUMMY_SP,
        sym: obj_type.into(),
        optional: false,
        ctxt: Default::default(),
    };

    let args = args
        .iter()
        .map(|x| {
            let arg_ast = TsExprAst::from(x.as_ref());
            ast::ExprOrSpread {
                spread: None,
                expr: arg_ast.node.to_owned(),
            }
        })
        .collect();

    let new_expr = ast::NewExpr {
        span: DUMMY_SP,
        ctxt: SyntaxContext::default(),
        callee: obj_ident.into(),
        args: Some(args),
        type_args: None,
    };

    TsExprAst::create(ast::Expr::New(new_expr))
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

fn static_member_call_ast(
    obj_type: &str,
    callee_name: &str,
    children: &[Box<dyn ExprAst>],
) -> Box<dyn ExprAst> {
    let obj_ident = ast::Ident {
        span: DUMMY_SP,
        sym: obj_type.into(),
        optional: false,
        ctxt: Default::default(),
    };

    let callee_expr = ast::MemberExpr {
        span: DUMMY_SP,
        obj: ast::Expr::Ident(obj_ident).into(),
        prop: ast::MemberProp::Ident(ast::IdentName::from(callee_name)),
    };

    let args = children
        .iter()
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

fn function_call_ast(callee_name: &str, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
    let callee_ident = ast::Ident {
        span: DUMMY_SP,
        sym: callee_name.into(),
        optional: false,
        ctxt: Default::default(),
    };

    let args = children
        .iter()
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
        callee: ast::Callee::Expr(ast::Expr::Ident(callee_ident).into()),
        args,
        type_args: None,
        ctxt: Default::default(),
    };

    TsExprAst::create(ast::Expr::Call(expr))
}

enum Wrapparound {
    YesWithMax,
    Yes,
    No,
}

fn get_index(value: &Number, len: usize, wraparound: Wrapparound) -> Result<usize, ()> {
    let ilen = len as isize;
    let ivalue = value.to_isize().ok_or(())?;

    if ivalue >= ilen {
        Ok(ilen as usize)
    } else if ivalue < 0 {
        match wraparound {
            Wrapparound::YesWithMax => Ok(max(ivalue + ilen, 0) as usize),
            Wrapparound::Yes => {
                let len = ivalue + ilen;
                if len < 0 {
                    Err(())
                } else {
                    Ok(len as usize)
                }
            },
            Wrapparound::No => Ok(0),
        }
    } else {
        Ok(ivalue as usize)
    }
}

mod array_ops;
mod assign_op;
mod bin;
mod dom_ops;
mod function;
mod ident;
mod lit;
mod member;
mod sequence;
mod set_ops;
mod string_ops;
mod unary;

pub use array_ops::*;
pub use assign_op::*;
pub use bin::*;
pub use dom_ops::*;
pub use function::*;
pub use ident::*;
pub use lit::*;
pub use member::*;
pub use sequence::*;
pub use set_ops::*;
pub use string_ops::*;
pub use unary::*;
