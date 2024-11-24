use std::cmp::min;

use ruse_object_graph::value::{Value, ValueType};
use ruse_object_graph::{vnum, vobj, Cache, Number};
use ruse_synthesizer::context::*;
use ruse_synthesizer::location::*;
use ruse_synthesizer::opcode::{EvalResult, ExprAst, ExprOpcode};

use swc_common::DUMMY_SP;
use swc_ecma_ast as ast;

use crate::opcode::{get_end_index, get_start_index, member_call_ast, member_field_ast};

use super::TsExprAst;

#[derive(Debug)]
pub struct ArrayIndexOp {
    arg_types: [ValueType; 2],
}

impl ArrayIndexOp {
    pub fn new(elem_type: &ValueType, cache: &Cache) -> Self {
        Self {
            arg_types: [
                ValueType::array_value_type(elem_type, cache),
                ValueType::Number,
            ],
        }
    }
}

impl ExprOpcode for ArrayIndexOp {
    fn op_name(&self) -> &str {
        "Array.prototype[Symbol.unscopables]"
    }

    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 2);

        let num = args[1].val().number_value().unwrap();
        let field_name = syn_ctx.cached_string(&(num.0 as usize).to_string());

        args[0]
            .get_obj_field_loc_value(&post_ctx.graphs_map, &field_name)
            .into()
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
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
pub struct ArrayLengthOp {
    arg_types: [ValueType; 1],
}

impl ArrayLengthOp {
    pub fn new(elem_type: &ValueType, cache: &Cache) -> Self {
        Self {
            arg_types: [ValueType::array_value_type(elem_type, cache)],
        }
    }
}

impl ExprOpcode for ArrayLengthOp {
    fn op_name(&self) -> &str {
        "Array: length"
    }

    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        _syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 1);

        let arr = args[0].val().obj().unwrap();
        let len = arr.total_field_count(&post_ctx.graphs_map);

        EvalResult::NoModification(post_ctx.temp_value(vnum!(len.into())))
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), 1);
        member_field_ast(&children[0], "length")
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}

#[derive(Debug)]
pub struct ArrayPushOp {
    arg_types: [ValueType; 2],
}

impl ArrayPushOp {
    pub fn new(elem_type: &ValueType, cache: &Cache) -> Self {
        Self {
            arg_types: [
                ValueType::array_value_type(elem_type, cache),
                elem_type.to_owned(),
            ],
        }
    }
}

impl ExprOpcode for ArrayPushOp {
    fn op_name(&self) -> &str {
        "Array.prototype.push"
    }

    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 2);

        let arr = args[0].val().obj().unwrap();
        let new_idx = arr.total_field_count(&post_ctx.graphs_map);
        let idx_field_name = syn_ctx.cached_string(&new_idx.to_string());
        post_ctx.set_field(arr.graph_id, arr.node, idx_field_name, args[1].val());

        let result = post_ctx.temp_value(vnum!(Number::from(
            arr.total_field_count(&post_ctx.graphs_map)
        )));

        EvalResult::DirtyContext(result)
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), 2);
        member_call_ast("push", children)
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}

#[derive(Debug)]
pub struct ArrayPopOp {
    arg_types: [ValueType; 1],
}

impl ArrayPopOp {
    pub fn new(elem_type: &ValueType, cache: &Cache) -> Self {
        Self {
            arg_types: [ValueType::array_value_type(elem_type, cache)],
        }
    }
}

impl ExprOpcode for ArrayPopOp {
    fn op_name(&self) -> &str {
        "Array.prototype.pop"
    }

    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 1);

        let arr = args[0].val().obj().unwrap();
        let arr_len = arr.total_field_count(&post_ctx.graphs_map);
        if arr_len == 0 {
            return EvalResult::None;
        }
        let idx_field_name = syn_ctx.cached_string(&(arr_len - 1).to_string());
        if let Some(result) = post_ctx.delete_field(arr.graph_id, arr.node, &idx_field_name) {
            EvalResult::DirtyContext(post_ctx.temp_value(result))
        } else {
            EvalResult::None
        }
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), 1);
        member_call_ast("pop", children)
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
            arg_types,
        }
    }
}

impl ExprOpcode for ArraySliceOp {
    fn op_name(&self) -> &str {
        "Array.prototype.slice"
    }

    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        let arr = args[0].val().obj().unwrap();
        let graph = arr.graph(&post_ctx.graphs_map);
        let arr_len = arr.total_field_count(&post_ctx.graphs_map);
        let start = get_start_index(args[1].val().number_value().unwrap().0 as isize, arr_len);
        let end = match args.get(2) {
            Some(v) => get_end_index(v.val().number_value().unwrap().0 as isize, arr_len),
            None => arr_len,
        };

        if start >= end {
            let empty_arr = post_ctx.create_output_array_object(&self.elem_type, [], &syn_ctx);
            return EvalResult::NoModification(post_ctx.temp_value(Value::Object(empty_arr)));
        }

        let new_arr = if self.elem_type.is_primitive() {
            let fields = graph
                .fields(&arr.node)
                .skip(start)
                .take(end - start)
                .map(|(_, p)| p.clone());
            post_ctx.create_output_primitive_array(&self.elem_type, fields, &syn_ctx)
        } else {
            let fields = graph
                .neighbors(&arr.node)
                .skip(start)
                .take(end - start)
                .map(|(_, n)| match n {
                    ruse_object_graph::EdgeEndPoint::Internal(node_index) => {
                        vobj!(arr.graph_id, *node_index)
                    }
                    ruse_object_graph::EdgeEndPoint::Chain(graph_id, node_index) => {
                        vobj!(*graph_id, *node_index)
                    }
                });
            post_ctx.create_output_array_object(&self.elem_type, fields, &syn_ctx)
        };
        EvalResult::NoModification(post_ctx.temp_value(Value::Object(new_arr)))
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
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
            arg_types,
        }
    }
}

impl ExprOpcode for ArrayConcatOp {
    fn op_name(&self) -> &str {
        "Array.prototype.concat"
    }

    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        let arr = args[0].val().obj().unwrap();
        let graph = arr.graph(&post_ctx.graphs_map);

        let new_arr = if self.elem_type.is_primitive() {
            let values = graph.fields(&arr.node).map(|x| x.1.clone()).chain(
                args.iter()
                    .skip(1)
                    .map(|x| x.val().primitive().unwrap().clone()),
            );
            post_ctx.create_output_primitive_array(&self.elem_type, values, &syn_ctx)
        } else {
            let values = graph
                .neighbors(&arr.node)
                .map(|(_, n)| match n {
                    ruse_object_graph::EdgeEndPoint::Internal(node_index) => {
                        vobj!(arr.graph_id, *node_index)
                    }
                    ruse_object_graph::EdgeEndPoint::Chain(graph_id, node_index) => {
                        vobj!(*graph_id, *node_index)
                    }
                })
                .chain(args.iter().skip(1).map(|x| x.val().clone()));
            post_ctx.create_output_array_object(&self.elem_type, values, &syn_ctx)
        };

        EvalResult::NoModification(post_ctx.temp_value(Value::Object(new_arr)))
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
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
            arg_types,
        }
    }
}

impl ExprOpcode for ArraySpliceOp {
    fn op_name(&self) -> &str {
        "Array.prototype.splice"
    }

    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        let arr = args[0].val().obj().unwrap();
        let graph = arr.graph(&post_ctx.graphs_map);
        let arr_len = arr.total_field_count(&post_ctx.graphs_map);
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

        let new_arr = if self.elem_type.is_primitive() {
            let items_to_add = args
                .iter()
                .skip(2)
                .map(|x| x.val().primitive().unwrap().clone());

            let fields = graph
                .fields(&arr.node)
                .take(start)
                .map(|(_, p)| p.clone())
                .chain(items_to_add)
                .chain(
                    graph
                        .fields(&arr.node)
                        .skip(start + delete_count)
                        .map(|(_, p)| p.clone()),
                );

            post_ctx.create_output_primitive_array(&self.elem_type, fields, &syn_ctx)
        } else {
            let items_to_add = args.iter().skip(2).map(|x| x.val().clone());

            let fields = graph
                .neighbors(&arr.node)
                .take(start)
                .map(|(_, n)| match n {
                    ruse_object_graph::EdgeEndPoint::Internal(node_index) => {
                        vobj!(arr.graph_id, *node_index)
                    }
                    ruse_object_graph::EdgeEndPoint::Chain(graph_id, node_index) => {
                        vobj!(*graph_id, *node_index)
                    }
                })
                .chain(items_to_add)
                .chain(graph.neighbors(&arr.node).skip(start + delete_count).map(
                    |(_, n)| match n {
                        ruse_object_graph::EdgeEndPoint::Internal(node_index) => {
                            vobj!(arr.graph_id, *node_index)
                        }
                        ruse_object_graph::EdgeEndPoint::Chain(graph_id, node_index) => {
                            vobj!(*graph_id, *node_index)
                        }
                    },
                ));
            post_ctx.create_output_array_object(&self.elem_type, fields, &syn_ctx)
        };

        let mut new_arr_loc = args[0].loc().clone();
        if !post_ctx.update_value(&Value::Object(new_arr), &mut new_arr_loc, syn_ctx) {
            return EvalResult::None;
        }

        let deleted_items_arr = if self.elem_type.is_primitive() {
            let fields = graph
                .fields(&arr.node)
                .skip(start)
                .take(delete_count)
                .map(|(_, p)| p.clone());
            post_ctx.create_output_primitive_array(&self.elem_type, fields, &syn_ctx)
        } else {
            let fields = graph
                .neighbors(&arr.node)
                .skip(start)
                .take(delete_count)
                .map(|(_, n)| match n {
                    ruse_object_graph::EdgeEndPoint::Internal(node_index) => {
                        vobj!(arr.graph_id, *node_index)
                    }
                    ruse_object_graph::EdgeEndPoint::Chain(graph_id, node_index) => {
                        vobj!(*graph_id, *node_index)
                    }
                });
            post_ctx.create_output_array_object(&self.elem_type, fields, &syn_ctx)
        };
        EvalResult::NoModification(post_ctx.temp_value(Value::Object(deleted_items_arr)))
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
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
            arg_types,
        }
    }
}

impl ExprOpcode for ArrayConcatArrayOp {
    fn op_name(&self) -> &str {
        "Array.prototype.concat"
    }

    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        let arr = args[0].val().obj().unwrap();
        let arr_to_add = args[1].val().obj().unwrap();
        let graph = arr.graph(&post_ctx.graphs_map);
        let graph_to_add = arr_to_add.graph(&post_ctx.graphs_map);

        let new_arr = if self.elem_type.is_primitive() {
            let values = graph
                .fields(&arr.node)
                .map(|x| x.1.clone())
                .chain(graph_to_add.fields(&arr_to_add.node).map(|x| x.1.clone()));
            post_ctx.create_output_primitive_array(&self.elem_type, values, &syn_ctx)
        } else {
            let values = graph
                .neighbors(&arr.node)
                .map(|(_, n)| match n {
                    ruse_object_graph::EdgeEndPoint::Internal(node_index) => {
                        vobj!(arr.graph_id, *node_index)
                    }
                    ruse_object_graph::EdgeEndPoint::Chain(graph_id, node_index) => {
                        vobj!(*graph_id, *node_index)
                    }
                })
                .chain(
                    graph_to_add
                        .neighbors(&arr_to_add.node)
                        .map(|(_, n)| match n {
                            ruse_object_graph::EdgeEndPoint::Internal(node_index) => {
                                vobj!(arr_to_add.graph_id, *node_index)
                            }
                            ruse_object_graph::EdgeEndPoint::Chain(graph_id, node_index) => {
                                vobj!(*graph_id, *node_index)
                            }
                        }),
                );
            post_ctx.create_output_array_object(&self.elem_type, values, &syn_ctx)
        };

        EvalResult::NoModification(post_ctx.temp_value(Value::Object(new_arr)))
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        member_call_ast("concat", children)
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}
