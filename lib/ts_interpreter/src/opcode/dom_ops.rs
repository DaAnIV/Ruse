use ruse_object_graph::{value::*, *};
use ruse_synthesizer::context::*;
use ruse_synthesizer::location::*;
use ruse_synthesizer::opcode::{EvalResult, ExprAst, ExprOpcode};

use crate::dom;
use crate::opcode::member_call_ast;

#[derive(Debug)]
pub struct GetElementByIdOp {
    arg_types: [ValueType; 2],
    id_field_name: CachedString,
}

impl GetElementByIdOp {
    pub fn new(cache: &Cache) -> Self {
        Self {
            arg_types: [
                ValueType::Object(dom::DomLoader::document_obj_type(cache)),
                ValueType::String,
            ],
            id_field_name: str_cached!(cache; "id"),
        }
    }

    fn node_id_equal(&self, node: &ObjectGraphNode, id: &CachedString) -> bool {
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
        _syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 2);

        let obj = args[0].val().obj().unwrap();
        let id = args[1].val().string_value().unwrap();
        let mut found = EvalResult::None;

        for (graph, node) in
            graph_walk::ObjectGraphWalker::from_node(&post_ctx.graphs_map, obj.graph_id, obj.node)
        {
            for (field, neig) in &node.pointers {
                let neig_node = graph.get_node_from_edge_end_point(&neig, &post_ctx.graphs_map);
                if self.node_id_equal(neig_node, &id) {
                    let val = match neig {
                        EdgeEndPoint::Internal(node_index) => ObjectValue {
                            graph_id: graph.id,
                            node: *node_index,
                        },
                        EdgeEndPoint::Chain(graph_id, node_index) => ObjectValue {
                            graph_id: *graph_id,
                            node: *node_index,
                        },
                    };
                    let loc = ObjectFieldLoc {
                        graph: graph.id,
                        node: node.id,
                        field: field.clone(),
                    };
                    found = EvalResult::NoModification(
                        post_ctx.get_loc_value(Value::Object(val), Location::ObjectField(loc)),
                    );
                }
            }
        }

        found
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), 2);
        member_call_ast("getElementById", children)
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}
