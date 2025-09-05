use itertools::Itertools;
use num_traits::ToPrimitive;
use ruse_object_graph::{field_name, vnum, vobj, vstr, Number};
use ruse_object_graph::{value::Value, ValueType};
use ruse_synthesizer::location::*;
use ruse_synthesizer::opcode::{EvalResult, ExprAst, ExprOpcode};
use ruse_synthesizer::{context::*, dirty, pure};
use std::cmp::min;

use swc_common::DUMMY_SP;
use swc_ecma_ast as ast;

use crate::opcode::{get_end_index, get_start_index, member_call_ast, member_expr};

use super::TsExprAst;

#[derive(Debug)]
pub struct ArrayIndexOp {
    arg_types: [ValueType; 2],
}

impl ArrayIndexOp {
    pub fn new(elem_type: &ValueType) -> Self {
        Self {
            arg_types: [ValueType::array_value_type(elem_type), ValueType::Number],
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
        _syn_ctx: &SynthesizerContext,
        _worker_ctx: &mut SynthesizerWorkerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 2);

        let num = args[1].val().number_value().unwrap();
        let index = num.to_usize().ok_or(())?;
        let field_name = field_name!(index.to_string().as_str());

        let field_value = args[0]
            .get_obj_field_loc_value(&post_ctx.graphs_map, &field_name)
            .ok_or(())?;

        pure!(field_value)
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
    pub fn new(elem_type: &ValueType) -> Self {
        Self {
            arg_types: [ValueType::array_value_type(elem_type)],
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
        _worker_ctx: &mut SynthesizerWorkerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 1);

        let arr = args[0].val().obj().unwrap();
        let len = arr.total_field_count(&post_ctx.graphs_map);

        pure!(post_ctx.temp_value(vnum!(len.into())))
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), 1);
        let member = member_expr(&children[0], "length");
        TsExprAst::create(ast::Expr::Member(member))
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
    pub fn new(elem_type: &ValueType) -> Self {
        Self {
            arg_types: [ValueType::array_value_type(elem_type), elem_type.to_owned()],
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
        _syn_ctx: &SynthesizerContext,
        _worker_ctx: &mut SynthesizerWorkerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 2);

        let arr = args[0].val().obj().unwrap();
        let new_idx = arr.total_field_count(&post_ctx.graphs_map);
        let idx_field_name = field_name!(new_idx.to_string().as_str());
        post_ctx.set_field(arr.graph_id, arr.node, idx_field_name, args[1].val());

        let result = post_ctx.temp_value(vnum!(Number::from(
            arr.total_field_count(&post_ctx.graphs_map)
        )));

        dirty!(result)
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
    pub fn new(elem_type: &ValueType) -> Self {
        Self {
            arg_types: [ValueType::array_value_type(elem_type)],
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
        _syn_ctx: &SynthesizerContext,
        _worker_ctx: &mut SynthesizerWorkerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 1);

        let arr = args[0].val().obj().unwrap();
        let arr_len = arr.total_field_count(&post_ctx.graphs_map);
        if arr_len == 0 {
            return Err(());
        }
        let idx_field_name = field_name!((arr_len - 1).to_string().as_str());
        if let Some(result) = post_ctx.delete_field(arr.graph_id, arr.node, &idx_field_name) {
            dirty!(post_ctx.temp_value(result))
        } else {
            Err(())
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
    pub fn new(elem_type: &ValueType, with_end: bool) -> Self {
        let mut arg_types = vec![ValueType::array_value_type(elem_type), ValueType::Number];
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
        _syn_ctx: &SynthesizerContext,
        _worker_ctx: &mut SynthesizerWorkerContext,
    ) -> EvalResult {
        let arr = args[0].val().obj().unwrap();
        let graph = arr.graph(&post_ctx.graphs_map);
        let arr_len = arr.total_field_count(&post_ctx.graphs_map);
        let start = get_start_index(&args[1].val().number_value().unwrap(), arr_len)?;
        let end = match args.get(2) {
            Some(v) => get_end_index(&v.val().number_value().unwrap(), arr_len)?,
            None => arr_len,
        };

        if start >= end {
            let empty_arr = post_ctx.create_output_array_object(&self.elem_type, []);
            return pure!(post_ctx.temp_value(Value::Object(empty_arr)));
        }

        let new_arr = if self.elem_type.is_primitive() {
            let fields = graph
                .primitive_fields(&arr.node)
                .skip(start)
                .take(end - start)
                .map(|(_, p)| p.clone());
            post_ctx.create_output_primitive_array_from_fields(&self.elem_type, fields)
        } else {
            let field_obj_type = self.elem_type.obj_type().unwrap();
            let fields = graph
                .neighbors(&arr.node)
                .skip(start)
                .take(end - start)
                .map(|(_, n)| {
                    vobj!(
                        field_obj_type.clone(),
                        n.graph.unwrap_or(arr.graph_id),
                        n.node
                    )
                });
            post_ctx.create_output_array_object(&self.elem_type, fields)
        };
        pure!(post_ctx.temp_value(Value::Object(new_arr)))
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
    pub fn new(elem_type: &ValueType, count: usize) -> Self {
        assert!(count > 0);

        let mut arg_types = vec![ValueType::array_value_type(elem_type)];
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
        _syn_ctx: &SynthesizerContext,
        _worker_ctx: &mut SynthesizerWorkerContext,
    ) -> EvalResult {
        let arr = args[0].val().obj().unwrap();
        let graph = arr.graph(&post_ctx.graphs_map);

        let new_arr = if self.elem_type.is_primitive() {
            let values = graph
                .primitive_fields(&arr.node)
                .map(|x| x.1.clone())
                .chain(
                    args.iter()
                        .skip(1)
                        .map(|x| x.val().primitive().unwrap().clone().into()),
                );
            post_ctx.create_output_primitive_array_from_fields(&self.elem_type, values)
        } else {
            let field_obj_type = self.elem_type.obj_type().unwrap();
            let values = graph
                .neighbors(&arr.node)
                .map(|(_, n)| {
                    vobj!(
                        field_obj_type.clone(),
                        n.graph.unwrap_or(arr.graph_id),
                        n.node
                    )
                })
                .chain(args.iter().skip(1).map(|x| x.val().clone()));
            post_ctx.create_output_array_object(&self.elem_type, values)
        };

        pure!(post_ctx.temp_value(Value::Object(new_arr)))
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
    pub fn new(elem_type: &ValueType, with_delete: bool) -> Self {
        let mut arg_types = vec![ValueType::array_value_type(elem_type), ValueType::Number];
        if with_delete {
            arg_types.push(ValueType::Number);
        }
        // if items_count > 0 {
        //     arg_types.extend(vec![elem_type.clone(); items_count])
        // }
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
        _syn_ctx: &SynthesizerContext,
        _worker_ctx: &mut SynthesizerWorkerContext,
    ) -> EvalResult {
        let arr = args[0].val().obj().unwrap();
        let graph = arr.graph(&post_ctx.graphs_map);
        let arr_len = arr.total_field_count(&post_ctx.graphs_map);
        let start = get_start_index(&args[1].val().number_value().unwrap(), arr_len)?;
        let delete_count = match args.get(2) {
            Some(v) => {
                let ivalue = v.val().number_value().unwrap().0 as isize;
                if ivalue <= 0 {
                    return Err(());
                }
                min(ivalue as usize, arr_len - start)
            }
            None => arr_len - start,
        };

        let items_to_add_len = args.len().max(3) - 3;
        let items_to_add = args.iter().skip(3).map(|x| x.val().clone());
        let new_arr_len = items_to_add_len + (arr_len - delete_count);

        let fields = items_to_add.chain(
            graph
                .primitive_fields(&arr.node)
                .skip(start + delete_count)
                .map(|(_, p)| Value::Primitive(p.value.clone())),
        );

        let deleted_items_arr = if self.elem_type.is_primitive() {
            let fields = graph
                .primitive_fields(&arr.node)
                .skip(start)
                .take(delete_count)
                .map(|(_, p)| p.value.clone());
            post_ctx.create_output_primitive_array(&self.elem_type, fields)
        } else {
            let field_obj_type = self.elem_type.obj_type().unwrap();
            let fields = graph
                .neighbors(&arr.node)
                .skip(start)
                .take(delete_count)
                .map(|(_, n)| {
                    vobj!(
                        field_obj_type.clone(),
                        n.graph.unwrap_or(arr.graph_id),
                        n.node
                    )
                });
            post_ctx.create_output_array_object(&self.elem_type, fields)
        };

        for (i, field) in fields.enumerate() {
            post_ctx.set_field(
                graph.id,
                arr.node,
                field_name!((i + start).to_string().as_str()),
                &field,
            );
        }

        for i in new_arr_len..arr_len {
            post_ctx.delete_field(graph.id, arr.node, &field_name!((i.to_string()).as_str()));
        }

        dirty!(post_ctx.temp_value(Value::Object(deleted_items_arr)))
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
    pub fn new(elem_type: &ValueType) -> Self {
        let arg_types = vec![
            ValueType::array_value_type(elem_type),
            ValueType::array_value_type(elem_type),
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
        _syn_ctx: &SynthesizerContext,
        _worker_ctx: &mut SynthesizerWorkerContext,
    ) -> EvalResult {
        let arr = args[0].val().obj().unwrap();
        let arr_to_add = args[1].val().obj().unwrap();
        let graph = arr.graph(&post_ctx.graphs_map);
        let graph_to_add = arr_to_add.graph(&post_ctx.graphs_map);

        let new_arr = if self.elem_type.is_primitive() {
            let values = graph
                .primitive_fields(&arr.node)
                .map(|x| x.1.value.clone())
                .chain(
                    graph_to_add
                        .primitive_fields(&arr_to_add.node)
                        .map(|x| x.1.value.clone()),
                );
            post_ctx.create_output_primitive_array(&self.elem_type, values)
        } else {
            let field_obj_type = self.elem_type.obj_type().unwrap();
            let values = graph
                .neighbors(&arr.node)
                .map(|(_, n)| {
                    vobj!(
                        field_obj_type.clone(),
                        n.graph.unwrap_or(arr.graph_id),
                        n.node
                    )
                })
                .chain(graph_to_add.neighbors(&arr_to_add.node).map(|(_, n)| {
                    vobj!(
                        field_obj_type.clone(),
                        n.graph.unwrap_or(arr_to_add.graph_id),
                        n.node
                    )
                }));
            post_ctx.create_output_array_object(&self.elem_type, values)
        };

        pure!(post_ctx.temp_value(Value::Object(new_arr)))
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        member_call_ast("concat", children)
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}

#[derive(Debug)]
pub struct ArrayReverseOp {
    arg_types: Vec<ValueType>,
}

impl ArrayReverseOp {
    pub fn new(elem_type: &ValueType) -> Self {
        let arg_types = vec![ValueType::array_value_type(elem_type)];
        Self { arg_types }
    }
}

impl ExprOpcode for ArrayReverseOp {
    fn op_name(&self) -> &str {
        "Array.prototype.reverse"
    }

    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        _syn_ctx: &SynthesizerContext,
        _worker_ctx: &mut SynthesizerWorkerContext,
    ) -> EvalResult {
        let arr = args[0].val().obj().unwrap();
        let graph = arr.graph(&post_ctx.graphs_map);

        let values: Vec<Value> = arr.array_values_iterator(&post_ctx.graphs_map).collect();
        for (i, value) in values.iter().rev().enumerate() {
            post_ctx.set_field(graph.id, arr.node, field_name!(i.to_string()), &value);
        }

        dirty!(post_ctx.temp_value(Value::Object(arr.clone())))
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        member_call_ast("reverse", children)
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}

#[derive(Debug)]
pub struct ArrayJoinOp {
    arg_types: Vec<ValueType>,
}

impl ArrayJoinOp {
    pub fn new(elem_type: &ValueType, seperator: bool) -> Self {
        if !elem_type.is_primitive() {
            unimplemented!("Array.prototype.join with non-primitive type");
        }
        let mut arg_types = vec![ValueType::array_value_type(elem_type)];
        if seperator {
            arg_types.push(ValueType::String);
        }
        Self { arg_types }
    }
}

impl ExprOpcode for ArrayJoinOp {
    fn op_name(&self) -> &str {
        "Array.prototype.join"
    }

    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        _syn_ctx: &SynthesizerContext,
        _worker_ctx: &mut SynthesizerWorkerContext,
    ) -> EvalResult {
        let arr = args[0].val().obj().unwrap();

        let values: Vec<Value> = arr.array_values_iterator(&post_ctx.graphs_map).collect();
        let seperator = if self.arg_types.len() == 2 {
            args[1].val().string_value().unwrap().to_string()
        } else {
            ",".to_string()
        };

        let result = values
            .iter()
            .map(|x| x.primitive().unwrap().to_string())
            .join(&seperator);

        pure!(post_ctx.temp_value(vstr!(result.as_str())))
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        member_call_ast("join", children)
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}
