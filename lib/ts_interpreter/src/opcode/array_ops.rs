use std::sync::Arc;

use ruse_object_graph::{scached, Cache, Number, PrimitiveValue};
use ruse_synthesizer::context::*;
use ruse_synthesizer::opcode::{ExprAst, ExprOpcode};
use ruse_synthesizer::value::*;

use swc_common::DUMMY_SP;
use swc_ecma_ast as ast;

use super::TsExprAst;

#[derive(Debug)]
pub struct IndexOp {
    arg_types: [ValueType; 2],
}

impl IndexOp {
    pub fn new(elem_type: &ValueType, cache: &Cache) -> Self {
        Self {
            arg_types: [ValueType::array_value_type(elem_type, cache), ValueType::Number],
        }
    }
}

impl ExprOpcode for IndexOp {
    fn eval(
        &self,
        args: &[&LocValue],
        _post_ctx: &mut Context,
        cache: &Arc<Cache>,
    ) -> Option<LocValue> {
        debug_assert_eq!(args.len(), 2);

        let num = args[1].val().number_value().unwrap();
        let field_name = scached!(cache; (num.0 as usize).to_string());

        args[0].get_obj_field_loc_value(&field_name)
    }

    fn to_ast(&self, children: &Vec<Box<dyn ExprAst>>) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), 2);

        let field = TsExprAst::from(children[1].as_ref());

        let expr = ast::MemberExpr {
            span: DUMMY_SP,
            obj: TsExprAst::from(children[0].as_ref()).get_paren_expr(),
            prop: ast::MemberProp::Computed(ast::ComputedPropName {
                span: DUMMY_SP,
                expr: field.node.to_owned(),
            }),
        };

        TsExprAst::create(ast::Expr::Member(expr))
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}

#[derive(Debug)]
pub struct PushOp {
    arg_types: [ValueType; 2],
}

impl PushOp {
    pub fn new(elem_type: &ValueType, cache: &Cache) -> Self {
        Self {
            arg_types: [ValueType::array_value_type(elem_type, cache), elem_type.to_owned()],
        }
    }
}

impl ExprOpcode for PushOp {
    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        cache: &Arc<Cache>,
    ) -> Option<LocValue> {
        debug_assert_eq!(args.len(), 2);

        let arr = args[0].val().obj().unwrap();
        let new_idx = arr.total_field_count();
        let idx_field_name = scached!(cache; new_idx.to_string());
        let new_arr = match &args[0].loc() {
            Location::Temp => {
                let (new_graph, node) =
                    post_ctx.set_field(&arr.graph, arr.node, &idx_field_name, args[1].val());

                ObjectValue {
                    graph: new_graph,
                    node: node.clone(),
                }
            }
            _ => {
                let var = match &args[0].loc() {
                    Location::Var(l) => &l.var,
                    Location::ObjectField(l) => &l.var,
                    Location::Temp => unreachable!()
                };
                let mut loc = Location::ObjectField(ObjectFieldLoc {
                    var: var.clone(),
                    node: arr.node,
                    field: idx_field_name,
                });
                if !post_ctx.update_value(args[1].val(), &mut loc) {
                    return None;
                }
                ObjectValue {
                    graph: post_ctx.get_var_loc_value(var).val().obj().unwrap().graph.clone(),
                    node: unsafe { loc.object_field().unwrap_unchecked().node },
                }
            }
        };

        Some(
            post_ctx.temp_value(Value::Primitive(PrimitiveValue::Number(Number::from(
                new_arr.total_field_count(),
            )))),
        )
    }

    fn to_ast(&self, children: &Vec<Box<dyn ExprAst>>) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), 2);

        let val = TsExprAst::from(children[1].as_ref());

        let callee_expr = ast::MemberExpr {
            span: DUMMY_SP,
            obj: TsExprAst::from(children[0].as_ref()).get_paren_expr(),
            prop: ast::MemberProp::Ident(ast::Ident {
                span: DUMMY_SP,
                sym: "push".into(),
                optional: false,
            }),
        };

        let expr = ast::CallExpr {
            span: DUMMY_SP,
            callee: ast::Callee::Expr(ast::Expr::Member(callee_expr).into()),
            args: vec![ast::ExprOrSpread {
                spread: None,
                expr: val.node.to_owned(),
            }],
            type_args: None,
        };

        TsExprAst::create(ast::Expr::Call(expr))
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}
