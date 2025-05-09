use std::sync::Arc;

use ruse_object_graph::ClassName;
use ruse_object_graph::FieldName;
use ruse_object_graph::ObjectGraph;
use ruse_object_graph::ValueType;
use ruse_synthesizer::context::*;
use ruse_synthesizer::location::*;
use ruse_synthesizer::opcode::{EvalResult, ExprAst, ExprOpcode};

use ruse_synthesizer::pure;
use swc_common::DUMMY_SP;
use swc_ecma_ast as ast;

use crate::opcode::member_field_ast;

use super::TsExprAst;

#[derive(Debug)]
pub struct MemberOp {
    arg_types: [ValueType; 1],
    field_name: FieldName,
    full_op_name: String,
}

impl MemberOp {
    pub fn new(class_name: ClassName, field_name: FieldName) -> Self {
        let full_op_name = format!("{}.{}", &class_name, &field_name);
        Self {
            arg_types: [ValueType::Object(ruse_object_graph::ObjectType::Class(
                class_name,
            ))],
            field_name,
            full_op_name,
        }
    }
}

impl ExprOpcode for MemberOp {
    fn op_name(&self) -> &str {
        &self.full_op_name
    }

    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        _syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 1);

        let member = args[0]
            .get_obj_field_loc_value(&post_ctx.graphs_map, &self.field_name)
            .ok_or(())?;
        pure!(member)
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), 1);

        member_field_ast(&children[0], &self.field_name)
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}

#[derive(Debug)]
pub struct StaticMemberOp {
    member_expr: ast::MemberExpr,
    full_op_name: String,
    initial_graph: Arc<ObjectGraph>,
    value: LocValue,
}

impl StaticMemberOp {
    pub fn new(
        obj_type: ClassName,
        field_name: FieldName,
        initial_graph: Arc<ObjectGraph>,
        value: LocValue,
    ) -> Self {
        let full_op_name = format!("{}.{}", &obj_type, &field_name);
        let obj_ident = ast::Ident {
            span: DUMMY_SP,
            sym: obj_type.as_str().into(),
            optional: false,
            ctxt: Default::default(),
        };
        let member_expr = ast::MemberExpr {
            span: DUMMY_SP,
            obj: Box::new(ast::Expr::Ident(obj_ident)),
            prop: ast::MemberProp::Ident(ast::IdentName::from(field_name.as_str())),
        };
        Self {
            member_expr,
            full_op_name,
            initial_graph,
            value,
        }
    }
}

impl ExprOpcode for StaticMemberOp {
    fn op_name(&self) -> &str {
        &self.full_op_name
    }

    fn eval(
        &self,
        _args: &[&LocValue],
        post_ctx: &mut Context,
        _: &SynthesizerContext,
    ) -> EvalResult {
        post_ctx.insert_if_new(self.initial_graph.clone());
        pure!(self.value.clone())
    }

    fn to_ast(&self, _children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        TsExprAst::create(ast::Expr::Member(self.member_expr.clone()))
    }

    fn arg_types(&self) -> &[ValueType] {
        &[]
    }
}
