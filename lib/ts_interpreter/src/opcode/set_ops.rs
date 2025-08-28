use ruse_object_graph::graph_map_value::GraphMapWrap;
use ruse_object_graph::{field_name, ValueType};
use ruse_object_graph::{vbool, vnum};
use ruse_synthesizer::location::*;
use ruse_synthesizer::opcode::{EvalResult, ExprAst, ExprOpcode};
use ruse_synthesizer::{context::*, dirty, pure};

use crate::opcode::{member_call_ast, member_expr, TsExprAst};

use swc_ecma_ast as ast;

#[derive(Debug)]
pub struct SetSizeOp {
    arg_types: [ValueType; 1],
}

impl SetSizeOp {
    pub fn new(elem_type: &ValueType) -> Self {
        Self {
            arg_types: [ValueType::set_value_type(elem_type)],
        }
    }
}

impl ExprOpcode for SetSizeOp {
    fn op_name(&self) -> &str {
        "Set: size"
    }

    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        _syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 1);

        let set = args[0].val().obj().unwrap();
        let len = set.total_field_count(&post_ctx.graphs_map);

        pure!(post_ctx.temp_value(vnum!(len.into())))
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), 1);
        let member = member_expr(&children[0], "size");
        TsExprAst::create(ast::Expr::Member(member))
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}

#[derive(Debug)]
pub struct SetHasOp {
    arg_types: [ValueType; 2],
}

impl SetHasOp {
    pub fn new(elem_type: &ValueType) -> Self {
        Self {
            arg_types: [ValueType::set_value_type(elem_type), elem_type.to_owned()],
        }
    }
}

impl ExprOpcode for SetHasOp {
    fn op_name(&self) -> &str {
        "Set.prototype.has"
    }

    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        _syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 2);

        let set = args[0].val().obj().unwrap();
        let value = args[1].val();
        let value_key = field_name!(value.wrap(&post_ctx.graphs_map).to_string().as_str());

        if set
            .get_field_value(&value_key, &post_ctx.graphs_map)
            .is_some()
        {
            return pure!(post_ctx.temp_value(vbool!(true)));
        }
        dirty!(post_ctx.temp_value(vbool!(false)))
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), 2);
        member_call_ast("add", children)
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}

#[derive(Debug)]
pub struct SetAddOp {
    arg_types: [ValueType; 2],
}

impl SetAddOp {
    pub fn new(elem_type: &ValueType) -> Self {
        Self {
            arg_types: [ValueType::set_value_type(elem_type), elem_type.to_owned()],
        }
    }
}

impl ExprOpcode for SetAddOp {
    fn op_name(&self) -> &str {
        "Set.prototype.add"
    }

    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        _syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 2);

        let set = args[0].val().obj().unwrap();
        let value = args[1].val();
        let value_key = field_name!(value.wrap(&post_ctx.graphs_map).to_string().as_str());

        if set
            .get_field_value(&value_key, &post_ctx.graphs_map)
            .is_some()
        {
            return pure!(args[0].clone());
        }
        post_ctx.set_field(set.graph_id, set.node, value_key, args[1].val());

        dirty!(args[0].clone())
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), 2);
        member_call_ast("add", children)
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}

#[derive(Debug)]
pub struct SetDeleteOp {
    arg_types: [ValueType; 2],
}

impl SetDeleteOp {
    pub fn new(elem_type: &ValueType) -> Self {
        Self {
            arg_types: [ValueType::set_value_type(elem_type), elem_type.to_owned()],
        }
    }
}

impl ExprOpcode for SetDeleteOp {
    fn op_name(&self) -> &str {
        "Set.prototype.delete"
    }

    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        _syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 2);

        let set = args[0].val().obj().unwrap();
        let value = args[1].val();
        let value_key = field_name!(value.wrap(&post_ctx.graphs_map).to_string().as_str());

        if set
            .get_field_value(&value_key, &post_ctx.graphs_map)
            .is_none()
        {
            return pure!(post_ctx.temp_value(vbool!(false)));
        }
        post_ctx.delete_field(set.graph_id, set.node, &value_key);

        dirty!(post_ctx.temp_value(vbool!(true)))
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), 2);
        member_call_ast("delete", children)
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}
