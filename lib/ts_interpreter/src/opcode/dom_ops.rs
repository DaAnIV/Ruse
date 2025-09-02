use ruse_object_graph::{value::*, *};
use ruse_synthesizer::context::*;
use ruse_synthesizer::location::*;
use ruse_synthesizer::opcode::{EvalResult, ExprAst, ExprOpcode};
use ruse_synthesizer::pure;

use crate::dom;
use crate::opcode::member_call_ast;

#[derive(Debug)]
pub struct GetElementByIdOp {
    arg_types: [ValueType; 2],
    id_field_name: FieldName,
}

impl GetElementByIdOp {
    pub fn new() -> Self {
        Self {
            arg_types: [
                ValueType::Object(dom::DomLoader::document_obj_type()),
                ValueType::String,
            ],
            id_field_name: field_name!("id"),
        }
    }

    fn node_id_equal(&self, node: &ObjectGraphNode, id: &StringValue) -> bool {
        if let Some(field) = node.get_field(&self.id_field_name) {
            field.value.string().as_ref().unwrap() == id
        } else {
            false
        }
    }
}

impl ExprOpcode for GetElementByIdOp {
    fn op_name(&self) -> &str {
        "Document: getElementById"
    }

    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        _syn_ctx: &SynthesizerContext,
        _worker_ctx: &mut SynthesizerWorkerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 2);

        let obj = args[0].val().obj().unwrap();
        let id = args[1].val().string_value().unwrap();

        for (graph, node_id, node) in
            graph_walk::ObjectGraphWalker::from_node(&post_ctx.graphs_map, obj.graph_id, obj.node)
        {
            for (field, neig) in node.pointers_iter() {
                let neig_node = graph.get_node_from_edge_end_point(&neig, &post_ctx.graphs_map);
                if self.node_id_equal(neig_node, &id) {
                    let val = ObjectValue {
                        obj_type: neig_node.obj_type().clone(),
                        graph_id: neig.graph.unwrap_or(graph.id),
                        node: neig.node,
                    };
                    let loc = ObjectFieldLoc {
                        graph: graph.id,
                        node: node_id,
                        field: field.clone(),
                        attrs: Attributes::default(),
                    };
                    return pure!(
                        post_ctx.get_loc_value(Value::Object(val), Location::ObjectField(loc))
                    );
                }
            }
        }

        Err(())
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), 2);
        member_call_ast("getElementById", children)
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}
