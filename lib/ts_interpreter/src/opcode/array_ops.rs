use ruse_object_graph::{Cache, Number};
use ruse_synthesizer::opcode::{EvalResult, ExprAst, ExprOpcode};
use ruse_synthesizer::value::*;
use ruse_synthesizer::{context::*, vnum};

use swc_common::DUMMY_SP;
use swc_ecma_ast as ast;

use crate::opcode::member_call_ast;

use super::TsExprAst;

#[derive(Debug)]
pub struct IndexOp {
    arg_types: [ValueType; 2],
}

impl IndexOp {
    pub fn new(elem_type: &ValueType, cache: &Cache) -> Self {
        Self {
            arg_types: [
                ValueType::array_value_type(elem_type, cache),
                ValueType::Number,
            ],
        }
    }
}

impl ExprOpcode for IndexOp {
    fn eval(
        &self,
        args: &[&LocValue],
        _post_ctx: &mut Context,
        syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 2);

        let num = args[1].val().number_value().unwrap();
        let field_name = syn_ctx.cached_string(&(num.0 as usize).to_string());

        args[0].get_obj_field_loc_value(&field_name).into()
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
            arg_types: [
                ValueType::array_value_type(elem_type, cache),
                elem_type.to_owned(),
            ],
        }
    }
}

impl ExprOpcode for PushOp {
    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 2);

        let arr = args[0].val().obj().unwrap();
        let new_idx = arr.total_field_count();
        let idx_field_name = syn_ctx.cached_string(&new_idx.to_string());
        let mut dirty = false;
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
                    Location::Temp => unreachable!(),
                };
                let mut loc = Location::ObjectField(ObjectFieldLoc {
                    var: var.clone(),
                    node: arr.node,
                    field: idx_field_name,
                });
                if !post_ctx.update_value(args[1].val(), &mut loc, syn_ctx) {
                    return EvalResult::None;
                }
                dirty = true;
                ObjectValue {
                    graph: post_ctx.get_var_loc_value(var).val().obj().unwrap().graph.clone(),
                    node: unsafe { loc.object_field().unwrap_unchecked().node },
                }
            }
        };

        let result = post_ctx.temp_value(vnum!(Number::from(new_arr.total_field_count())));

        if dirty {
            EvalResult::DirtyContext(result)
        } else {
            EvalResult::NoModification(result)
        }
    }

    fn to_ast(&self, children: &Vec<Box<dyn ExprAst>>) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), 2);
        member_call_ast("push", children)
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}
