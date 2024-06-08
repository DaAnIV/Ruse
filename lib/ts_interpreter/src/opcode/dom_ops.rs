use std::collections::HashMap;
use std::sync::Arc;

use ruse_object_graph::{
    scached, str_cached, Cache, CachedString, NodeIndex, Number, ObjectData, PrimitiveValue
};
use ruse_synthesizer::context::*;
use ruse_synthesizer::opcode::{ExprAst, ExprOpcode};
use ruse_synthesizer::value::*;

use swc_common::DUMMY_SP;
use swc_ecma_ast as ast;

use crate::dom;

use super::TsExprAst;

#[derive(Debug)]
pub struct GetElementByIdOp {
    arg_types: [ValueType; 2],
    id_field_name: CachedString
}

impl GetElementByIdOp {
    pub fn new(cache: &Cache) -> Self {
        Self {
            arg_types: [
                ValueType::Object(dom::DomLoader::document_obj_type(cache)),
                ValueType::String,
            ],
            id_field_name: str_cached!(cache; "id")
        }
    }

    fn node_id_equal(&self, node: &ObjectData, id: &CachedString) -> bool {
        if let Some(node_id) = node.fields.get(&self.id_field_name) {
            node_id.string().as_ref().unwrap() == id
        } else {
            false
        }
    }
}

impl ExprOpcode for GetElementByIdOp {
    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        cache: &Arc<Cache>,
    ) -> Option<LocValue> {
        debug_assert_eq!(args.len(), 2);

        let obj = args[0].val().obj().unwrap();
        let id = args[1].val().string_value().unwrap();
        let mut found = None;

        let mut parent =
            HashMap::<NodeIndex, (NodeIndex, CachedString)>::with_capacity(obj.graph.node_count());
        let mut stack = Vec::with_capacity(obj.graph.node_count());
        stack.push(obj.node);
        while let Some(node) = stack.pop() {
            let node_weight = obj.graph.node_weight(node).unwrap();

            if self.node_id_equal(node_weight, &id) {
                let val = ObjectValue {
                    graph: obj.graph.clone(),
                    node: node,
                };
                let (parent, field) = &parent[&node];
                let loc = ObjectFieldLoc {
                    var: dom::DomLoader::document_root_name(cache),
                    node: *parent,
                    field: field.clone(),
                };
                found =
                    Some(post_ctx.get_loc_value(Value::Object(val), Location::ObjectField(loc)));
                break;
            }

            for i in 0..node_weight.neighbors_count() {
                let child_field_name = scached!(cache; i.to_string());
                let child_node = node_weight.get_neighbor(&child_field_name).unwrap();
                parent.insert(child_node, (node, child_field_name));
                stack.push(child_node);
            }
        }

        found
    }

    fn to_ast(&self, children: &Vec<Box<dyn ExprAst>>) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), 2);

        let val = TsExprAst::from(children[1].as_ref());

        let callee_expr = ast::MemberExpr {
            span: DUMMY_SP,
            obj: TsExprAst::from(children[0].as_ref()).get_paren_expr(),
            prop: ast::MemberProp::Ident(ast::Ident {
                span: DUMMY_SP,
                sym: "getElementById".into(),
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
