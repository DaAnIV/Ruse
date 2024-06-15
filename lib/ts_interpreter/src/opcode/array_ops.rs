use std::cmp::min;

use ruse_object_graph::{Cache, Number};
use ruse_synthesizer::opcode::{EvalResult, ExprAst, ExprOpcode};
use ruse_synthesizer::value::*;
use ruse_synthesizer::{context::*, vnum};

use swc_common::DUMMY_SP;
use swc_ecma_ast as ast;

use crate::opcode::{get_end_index, get_start_index, member_call_ast};

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
                if let Some(loc_value) = post_ctx.get_var_loc_value(var) {
                    ObjectValue {
                        graph: loc_value.val().obj().unwrap().graph.clone(),
                        node: unsafe { loc.object_field().unwrap_unchecked().node },
                    }
                } else {
                    return EvalResult::None;
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

#[derive(Debug)]
pub struct ArraySliceOp {
    elem_type: ValueType,
    arg_types: Vec<ValueType>,
}

impl ArraySliceOp {
    pub fn new(elem_type: &ValueType, with_end: bool, cache: &Cache) -> Self {
        let mut arg_types = vec![
            ValueType::array_value_type(elem_type, cache),
            ValueType::Number,
        ];
        if with_end {
            arg_types.push(ValueType::Number);
        }
        Self {
            elem_type: elem_type.clone(),
            arg_types: arg_types,
        }
    }
}

impl ExprOpcode for ArraySliceOp {
    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        let arr = args[0].val().obj().unwrap();
        let graph = arr.graph.clone();
        let arr_len = arr.total_field_count();
        let start = get_start_index(args[1].val().number_value().unwrap().0 as isize, arr_len);
        let end = match args.get(2) {
            Some(v) => get_end_index(v.val().number_value().unwrap().0 as isize, arr_len),
            None => arr_len,
        };

        if start >= end {
            return EvalResult::NoModification(post_ctx.temp_value(Value::create_array_object(
                &self.elem_type,
                [],
                &syn_ctx.cache,
            )));
        }

        let mut new_arr = if self.elem_type.is_primitive() {
            let fields = arr
                .fields()
                .skip(start)
                .take(end - start)
                .map(|(_, p)| p.clone());
            Value::create_primitive_array_object(&self.elem_type, fields, &syn_ctx.cache)
        } else {
            let fields = arr.neighbors().skip(start).take(end - start).map(|(_, n)| {
                Value::Object(ObjectValue {
                    graph: graph.clone(),
                    node: n,
                })
            });
            Value::create_array_object(&self.elem_type, fields, &syn_ctx.cache)
        };
        new_arr.mut_obj().unwrap().set_as_graph_root(syn_ctx.output_root_name().clone());
        EvalResult::NoModification(post_ctx.temp_value(new_arr))
    }

    fn to_ast(&self, children: &Vec<Box<dyn ExprAst>>) -> Box<dyn ExprAst> {
        member_call_ast("slice", children)
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}

#[derive(Debug)]
pub struct ArrayConcatOp {
    elem_type: ValueType,
    arg_types: Vec<ValueType>,
}

impl ArrayConcatOp {
    pub fn new(elem_type: &ValueType, count: usize, cache: &Cache) -> Self {
        assert!(count > 0);

        let mut arg_types = vec![ValueType::array_value_type(elem_type, cache)];
        arg_types.extend(vec![elem_type.clone(); count]);
        Self {
            elem_type: elem_type.clone(),
            arg_types: arg_types,
        }
    }
}

impl ExprOpcode for ArrayConcatOp {
    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        let arr = args[0].val().obj().unwrap();
        let graph = arr.graph.clone();

        let mut new_arr = if self.elem_type.is_primitive() {
            let values = arr.fields().map(|x| x.1.clone()).chain(
                args.iter()
                    .skip(1)
                    .map(|x| x.val().primitive().unwrap().clone()),
            );
            Value::create_primitive_array_object(&self.elem_type, values, &syn_ctx.cache)
        } else {
            let values = arr
                .neighbors()
                .map(|x| {
                    Value::Object(ObjectValue {
                        graph: graph.clone(),
                        node: x.1.clone(),
                    })
                })
                .chain(args.iter().skip(1).map(|x| x.val().clone()));
            Value::create_array_object(&self.elem_type, values, &syn_ctx.cache)
        };
        new_arr.mut_obj().unwrap().set_as_graph_root(syn_ctx.output_root_name().clone());

        debug_assert_ne!(args[0].val(), &new_arr);

        EvalResult::NoModification(post_ctx.temp_value(new_arr))
    }

    fn to_ast(&self, children: &Vec<Box<dyn ExprAst>>) -> Box<dyn ExprAst> {
        member_call_ast("concat", children)
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}

#[derive(Debug)]
pub struct ArraySpliceOp {
    elem_type: ValueType,
    arg_types: Vec<ValueType>,
}

impl ArraySpliceOp {
    pub fn new(elem_type: &ValueType, items_count: usize, cache: &Cache) -> Self {
        let mut arg_types = vec![
            ValueType::array_value_type(elem_type, cache),
            ValueType::Number,
        ];
        if items_count > 0 {
            arg_types.extend(vec![elem_type.clone(); items_count])
        }
        Self {
            elem_type: elem_type.clone(),
            arg_types: arg_types,
        }
    }
}

impl ExprOpcode for ArraySpliceOp {
    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        let arr = args[0].val().obj().unwrap();
        let graph = arr.graph.clone();
        let arr_len = arr.total_field_count();
        let start = get_start_index(args[1].val().number_value().unwrap().0 as isize, arr_len);
        let delete_count = match args.get(2) {
            Some(v) => {
                let ivalue = v.val().number_value().unwrap().0 as isize;
                if ivalue <= 0 {
                    return EvalResult::None;
                }
                min(ivalue as usize, arr_len - start)
            }
            None => arr_len - start,
        };

        let mut new_arr = if self.elem_type.is_primitive() {
            let items_to_add = args
                .iter()
                .skip(2)
                .map(|x| x.val().primitive().unwrap().clone());

            let fields = arr
                .fields()
                .take(start)
                .map(|(_, p)| p.clone())
                .chain(items_to_add)
                .chain(
                    arr.fields()
                        .skip(start + delete_count)
                        .map(|(_, p)| p.clone()),
                );

            Value::create_primitive_array_object(&self.elem_type, fields, &syn_ctx.cache)
        } else {
            let items_to_add = args.iter().skip(2).map(|x| x.val().clone());

            let fields = arr
                .neighbors()
                .take(start)
                .map(|(_, n)| {
                    Value::Object(ObjectValue {
                        graph: graph.clone(),
                        node: n,
                    })
                })
                .chain(items_to_add)
                .chain(arr.neighbors().skip(start + delete_count).map(|(_, n)| {
                    Value::Object(ObjectValue {
                        graph: graph.clone(),
                        node: n,
                    })
                }));
            Value::create_array_object(&self.elem_type, fields, &syn_ctx.cache)
        };
        if let Some(root) = graph.roots().find(|x| x.1 == &arr.node) {
            new_arr.mut_obj().unwrap().set_as_graph_root(root.0.clone());
        }

        let mut new_arr_loc = args[0].loc().clone();
        if !post_ctx.update_value(&new_arr, &mut &mut new_arr_loc, syn_ctx) {
            return EvalResult::None;
        }

        let deleted_items_arr = if self.elem_type.is_primitive() {
            let fields = arr
                .fields()
                .skip(start)
                .take(delete_count)
                .map(|(_, p)| p.clone());
            Value::create_primitive_array_object(&self.elem_type, fields, &syn_ctx.cache)
        } else {
            let fields = arr
                .neighbors()
                .skip(start)
                .take(delete_count)
                .map(|(_, n)| {
                    Value::Object(ObjectValue {
                        graph: graph.clone(),
                        node: n,
                    })
                });
            Value::create_array_object(&self.elem_type, fields, &syn_ctx.cache)
        };
        EvalResult::NoModification(post_ctx.temp_value(deleted_items_arr))
    }

    fn to_ast(&self, children: &Vec<Box<dyn ExprAst>>) -> Box<dyn ExprAst> {
        member_call_ast("splice", children)
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}

#[derive(Debug)]
pub struct ArrayConcatArrayOp {
    elem_type: ValueType,
    arg_types: Vec<ValueType>,
}

impl ArrayConcatArrayOp {
    pub fn new(elem_type: &ValueType, cache: &Cache) -> Self {
        let arg_types = vec![
            ValueType::array_value_type(elem_type, cache),
            ValueType::array_value_type(elem_type, cache),
        ];
        Self {
            elem_type: elem_type.clone(),
            arg_types: arg_types,
        }
    }
}

impl ExprOpcode for ArrayConcatArrayOp {
    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        let arr = args[0].val().obj().unwrap();
        let arr_to_add = args[1].val().obj().unwrap();
        let graph = arr.graph.clone();

        let mut new_arr = if self.elem_type.is_primitive() {
            let values = arr
                .fields()
                .map(|x| x.1.clone())
                .chain(arr_to_add.fields().map(|x| x.1.clone()));
            Value::create_primitive_array_object(&self.elem_type, values, &syn_ctx.cache)
        } else {
            let values = arr
                .neighbors()
                .map(|x| {
                    Value::Object(ObjectValue {
                        graph: graph.clone(),
                        node: x.1.clone(),
                    })
                })
                .chain(arr_to_add.neighbors().map(|x| {
                    Value::Object(ObjectValue {
                        graph: graph.clone(),
                        node: x.1.clone(),
                    })
                }));
            Value::create_array_object(&self.elem_type, values, &syn_ctx.cache)
        };
        new_arr.mut_obj().unwrap().set_as_graph_root(syn_ctx.output_root_name().clone());

        EvalResult::NoModification(post_ctx.temp_value(new_arr))
    }

    fn to_ast(&self, children: &Vec<Box<dyn ExprAst>>) -> Box<dyn ExprAst> {
        member_call_ast("concat", children)
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}
