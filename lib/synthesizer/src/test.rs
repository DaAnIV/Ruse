#[cfg(test)]
mod helpers {
    use ruse_object_graph::value::ValueType;

    use crate::{
        context::{Context, SynthesizerContext},
        location::LocValue,
        opcode::{EvalResult, ExprAst, ExprOpcode},
    };

    pub struct TestAst {}

    impl ExprAst for TestAst {
        fn to_string(&self) -> String {
            "".to_owned()
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    #[derive(Debug)]
    pub struct TestOpcode {
        pub arg_types: Vec<ValueType>,
        pub returns: EvalResult,
    }

    impl ExprOpcode for TestOpcode {
        fn arg_types(&self) -> &[ValueType] {
            &self.arg_types
        }

        fn eval(&self, _: &[&LocValue], _: &mut Context, _: &SynthesizerContext) -> EvalResult {
            self.returns.clone()
        }

        fn to_ast(&self, _: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
            Box::new(TestAst {})
        }
    }
}

#[cfg(test)]
mod work_gatherer_tests {
    use crate::{context::GraphIdGenerator, test::helpers::*, work_gatherer::WorkGatherBuilder};
    use std::sync::Arc;

    use dashmap::DashMap;
    use itertools::Itertools;
    use ruse_object_graph::{
        value::{Value, ValueType},
        vnum, Cache, Number,
    };

    use crate::{
        bank::{ProgBank, TypeMap},
        context::{ContextArray, SynthesizerContext},
        location::{LocValue, Location},
        opcode::{EvalResult, ExprOpcode},
        prog::SubProgram,
    };

    async fn run_gatherer(
        bank: &ProgBank,
        op: &Arc<dyn ExprOpcode>,
        chunk_size: usize,
        syn_ctx: &SynthesizerContext,
    ) -> Vec<Vec<Arc<SubProgram>>> {
        let cancel_token = Default::default();
        let all_children = Arc::new(DashMap::<usize, Vec<Arc<SubProgram>>>::default());
        let all_children_clone = all_children.clone();
        let handler = Arc::new(
            move |_: Arc<dyn ExprOpcode>,
                  _: ContextArray,
                  children: Vec<Arc<SubProgram>>,
                  _: ContextArray| {
                all_children_clone.insert(all_children_clone.len(), children);
                None
            },
        );
        let mut gatherer = WorkGatherBuilder::new(handler, cancel_token)
            .chunk_size(chunk_size)
            .build();
        gatherer
            .gather_work_for_next_iteration(bank, op.arg_types(), &vec![op.clone()], syn_ctx)
            .await;
        gatherer.wait_for_all_tasks().await;

        all_children.iter().map(|x| x.value().clone()).collect()
    }

    fn get_prog_for_bank(value: Value, syn_ctx: &SynthesizerContext) -> Arc<SubProgram> {
        let init_op: Arc<dyn ExprOpcode> = Arc::new(TestOpcode {
            arg_types: vec![],
            returns: EvalResult::NoModification(LocValue {
                loc: Location::Temp,
                val: value,
            }),
        });

        let mut p = SubProgram::with_opcode(
            init_op,
            syn_ctx.start_context.clone(),
            syn_ctx.start_context.clone(),
        );
        Arc::get_mut(&mut p).unwrap().evaluate(syn_ctx);
        p
    }

    #[allow(dead_code)]
    fn print_all_children(all_children: &[Vec<Arc<SubProgram>>]) {
        for c in all_children {
            let values: Vec<String> = c
                .iter()
                .map(|x| {
                    let num = x.out_value()[0].val().number_value().unwrap().0 as u64;
                    format!("{:x}", num)
                })
                .collect();
            println!("{:?}", values);
        }
    }

    fn add_iteration(bank: &mut ProgBank, n: usize, syn_ctx: &SynthesizerContext) {
        let iteration = bank.iteration_count();
        let type_map = Arc::new(TypeMap::default());
        for i in 0..n {
            let value = Number::from(iteration << 32 | i);
            let p = get_prog_for_bank(vnum!(value), syn_ctx);
            type_map.insert_program(p.clone());
        }
        bank.insert(type_map);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn one_iteration_one_program() {
        let cache = Arc::new(Cache::new());
        let _id_gen = GraphIdGenerator::default();
        let syn_ctx = SynthesizerContext::from_context_array(ContextArray::default(), cache);
        let mut bank = ProgBank::default();
        let bin_op: Arc<dyn ExprOpcode> = Arc::new(TestOpcode {
            arg_types: vec![ValueType::Number, ValueType::Number],
            returns: EvalResult::NoModification(LocValue {
                loc: Location::Temp,
                val: vnum!(Number::from(5)),
            }),
        });

        add_iteration(&mut bank, 1, &syn_ctx);

        let all_children = run_gatherer(&bank, &bin_op, 1, &syn_ctx).await;
        assert_eq!(all_children.len(), 1);
        // print_all_children(&all_children);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn two_iterations() {
        let cache = Arc::new(Cache::new());
        let _id_gen = GraphIdGenerator::default();
        let syn_ctx = SynthesizerContext::from_context_array(ContextArray::default(), cache);
        let mut bank = ProgBank::default();
        let bin_op: Arc<dyn ExprOpcode> = Arc::new(TestOpcode {
            arg_types: vec![ValueType::Number, ValueType::Number],
            returns: EvalResult::NoModification(LocValue {
                loc: Location::Temp,
                val: vnum!(Number::from(5)),
            }),
        });

        add_iteration(&mut bank, 2, &syn_ctx);
        add_iteration(&mut bank, 3, &syn_ctx);

        let all_children = run_gatherer(&bank, &bin_op, 1, &syn_ctx).await;
        assert_eq!(all_children.len(), 5usize.pow(2) - 2usize.pow(2));
        assert!(all_children.iter().all_unique());
        for c in &all_children {
            assert!(c.iter().any(|x| {
                let num = x.out_value()[0].val().number_value().unwrap().0 as usize;
                (num >> 32) == (bank.iteration_count() - 1)
            }));
        }
        // print_all_children(&all_children);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn three_iterations_binary() {
        let cache = Arc::new(Cache::new());
        let _id_gen = GraphIdGenerator::default();
        let syn_ctx = SynthesizerContext::from_context_array(ContextArray::default(), cache);
        let mut bank = ProgBank::default();
        let bin_op: Arc<dyn ExprOpcode> = Arc::new(TestOpcode {
            arg_types: vec![ValueType::Number, ValueType::Number],
            returns: EvalResult::NoModification(LocValue {
                loc: Location::Temp,
                val: vnum!(Number::from(5)),
            }),
        });

        add_iteration(&mut bank, 2, &syn_ctx);
        add_iteration(&mut bank, 3, &syn_ctx);
        add_iteration(&mut bank, 4, &syn_ctx);

        let all_children = run_gatherer(&bank, &bin_op, 1, &syn_ctx).await;
        assert_eq!(all_children.len(), 9usize.pow(2) - 5usize.pow(2));
        assert!(all_children.iter().all_unique());
        for c in &all_children {
            assert!(c.iter().any(|x| {
                let num = x.out_value()[0].val().number_value().unwrap().0 as usize;
                (num >> 32) == (bank.iteration_count() - 1)
            }));
        }
        // print_all_children(&all_children);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn three_iterations_trinary() {
        let cache = Arc::new(Cache::new());
        let _id_gen = GraphIdGenerator::default();
        let syn_ctx = SynthesizerContext::from_context_array(ContextArray::default(), cache);
        let mut bank = ProgBank::default();
        let tri_op: Arc<dyn ExprOpcode> = Arc::new(TestOpcode {
            arg_types: vec![ValueType::Number, ValueType::Number, ValueType::Number],
            returns: EvalResult::NoModification(LocValue {
                loc: Location::Temp,
                val: vnum!(Number::from(5)),
            }),
        });

        add_iteration(&mut bank, 2, &syn_ctx);
        add_iteration(&mut bank, 3, &syn_ctx);
        add_iteration(&mut bank, 4, &syn_ctx);

        let all_children = run_gatherer(&bank, &tri_op, 1, &syn_ctx).await;
        assert_eq!(all_children.len(), 9usize.pow(3) - 5usize.pow(3));
        assert!(all_children.iter().all_unique());
        for c in &all_children {
            assert!(c.iter().any(|x| {
                let num = x.out_value()[0].val().number_value().unwrap().0 as usize;
                (num >> 32) == (bank.iteration_count() - 1)
            }));
        }
        // print_all_children(&all_children);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn three_iterations_binary_big_chunk() {
        let cache = Arc::new(Cache::new());
        let syn_ctx = SynthesizerContext::from_context_array(ContextArray::default(), cache);
        let mut bank = ProgBank::default();
        let bin_op: Arc<dyn ExprOpcode> = Arc::new(TestOpcode {
            arg_types: vec![ValueType::Number, ValueType::Number],
            returns: EvalResult::NoModification(LocValue {
                loc: Location::Temp,
                val: vnum!(Number::from(5)),
            }),
        });

        add_iteration(&mut bank, 2, &syn_ctx);
        add_iteration(&mut bank, 3, &syn_ctx);
        add_iteration(&mut bank, 4, &syn_ctx);

        let all_children = run_gatherer(&bank, &bin_op, 25, &syn_ctx).await;
        assert_eq!(all_children.len(), 9usize.pow(2) - 5usize.pow(2));
        assert!(all_children.iter().all_unique());
        for c in &all_children {
            assert!(c.iter().any(|x| {
                let num = x.out_value()[0].val().number_value().unwrap().0 as usize;
                (num >> 32) == (bank.iteration_count() - 1)
            }));
        }
        // print_all_children(&all_children);
    }
}

#[cfg(test)]
mod embedding_tests {
    use std::{collections::HashMap, sync::Arc};

    use ruse_object_graph::{
        dot::{self, SubgraphConfig},
        fields, str_cached,
        value::Value,
        vobj, Cache, GraphsMap, ObjectGraph, PrimitiveValue,
    };

    use crate::{
        context::{Context, GraphIdGenerator, VariableName},
        embedding,
    };

    #[test]
    fn embedding_test() {
        let cache = Cache::new();
        let id_gen = Arc::new(GraphIdGenerator::default());

        let graph_id = id_gen.get_id_for_graph();
        let mut p_1_graph = ObjectGraph::new(graph_id);
        let mut q_1_graph = ObjectGraph::new(graph_id);
        let mut p_2_graph = ObjectGraph::new(graph_id);
        let mut q_2_graph = ObjectGraph::new(graph_id);
        let mut p_1_graphs_map = GraphsMap::default();
        let mut q_1_graphs_map = GraphsMap::default();
        let mut p_2_graphs_map = GraphsMap::default();
        let mut q_2_graphs_map = GraphsMap::default();

        let root_name_x = str_cached!(&cache; "x");
        let obj_name_x = str_cached!(&cache; "Zoo");
        let field_name_x = str_cached!(&cache; "zoo");

        let root_name_y = str_cached!(&cache; "y");
        let obj_name_y = str_cached!(&cache; "A");
        let field_name_y = str_cached!(&cache; "a");

        let mut id = id_gen.get_id_for_node();

        let p_1_y = p_1_graph.add_simple_object(
            id,
            obj_name_y.clone(),
            fields!((field_name_y.clone(), PrimitiveValue::Number(4u64.into()))),
        );
        p_1_graph.set_as_root(root_name_y.clone(), p_1_y);
        let q_1_y = q_1_graph.add_simple_object(
            id,
            obj_name_y.clone(),
            fields!((field_name_y.clone(), PrimitiveValue::Number(5u64.into()))),
        );
        q_1_graph.set_as_root(root_name_y.clone(), q_1_y);

        id = id_gen.get_id_for_node();
        let p_2_y = p_2_graph.add_simple_object(
            id,
            obj_name_y.clone(),
            fields!((field_name_y.clone(), PrimitiveValue::Number(5u64.into()))),
        );
        p_2_graph.set_as_root(root_name_y.clone(), p_2_y);
        let q_2_y = q_2_graph.add_simple_object(
            id,
            obj_name_y.clone(),
            fields!((field_name_y.clone(), PrimitiveValue::Number(10u64.into()))),
        );
        q_2_graph.set_as_root(root_name_y.clone(), q_2_y);

        id = id_gen.get_id_for_node();
        let p_2_x = id;
        let q_2_x = id;
        {
            let p_2_x_node = p_2_graph.add_node(p_2_x, obj_name_x.clone(), fields!());
            p_2_x_node.insert_internal_edge(field_name_x.clone(), p_2_y);
        }
        p_2_graph.set_as_root(root_name_x.clone(), p_2_x);
        {
            let q_2_x_node = q_2_graph.add_node(q_2_x, obj_name_x.clone(), fields!());
            q_2_x_node.insert_internal_edge(field_name_x.clone(), q_2_y);
        }
        q_2_graph.set_as_root(root_name_x.clone(), q_2_x);

        let p_1_graph_arc = Arc::new(p_1_graph);
        let q_1_graph_arc = Arc::new(q_1_graph);
        let p_2_graph_arc = Arc::new(p_2_graph);
        let q_2_graph_arc = Arc::new(q_2_graph);
        p_1_graphs_map.insert_graph(p_1_graph_arc);
        q_1_graphs_map.insert_graph(q_1_graph_arc);
        p_2_graphs_map.insert_graph(p_2_graph_arc);
        q_2_graphs_map.insert_graph(q_2_graph_arc);

        dot::Dot::print_header();
        println!(
            "{}",
            dot::Dot::from_graphs_map_with_config(
                &p_1_graphs_map,
                dot::DotConfig {
                    prefix: Some("p1".to_owned()),
                    subgraph: Some(SubgraphConfig {
                        name: "p1".to_owned()
                    }),
                    print_only_content: false
                }
            )
        );
        println!(
            "{}",
            dot::Dot::from_graphs_map_with_config(
                &q_1_graphs_map,
                dot::DotConfig {
                    prefix: Some("q1".to_owned()),
                    subgraph: Some(SubgraphConfig {
                        name: "q1".to_owned()
                    }),
                    print_only_content: false
                }
            )
        );
        println!(
            "{}",
            dot::Dot::from_graphs_map_with_config(
                &p_2_graphs_map,
                dot::DotConfig {
                    prefix: Some("p2".to_owned()),
                    subgraph: Some(SubgraphConfig {
                        name: "p2".to_owned()
                    }),
                    print_only_content: false
                }
            )
        );
        println!(
            "{}",
            dot::Dot::from_graphs_map_with_config(
                &q_2_graphs_map,
                dot::DotConfig {
                    prefix: Some("q2".to_owned()),
                    subgraph: Some(SubgraphConfig {
                        name: "q2".to_owned()
                    }),
                    print_only_content: false
                }
            )
        );

        let p_1_values =
            HashMap::<VariableName, Value>::from([(root_name_y.clone(), vobj!(0, p_1_y))]);
        let q_1_values =
            HashMap::<VariableName, Value>::from([(root_name_y.clone(), vobj!(0, q_1_y))]);
        let p_2_values = HashMap::<VariableName, Value>::from([
            (root_name_y.clone(), vobj!(0, p_2_y)),
            (root_name_x.clone(), vobj!(0, p_2_x)),
        ]);
        let q_2_values = HashMap::<VariableName, Value>::from([
            (root_name_y.clone(), vobj!(0, q_2_y)),
            (root_name_x.clone(), vobj!(0, q_2_x)),
        ]);

        let p_1 = Context::with_values(p_1_values, p_1_graphs_map.into(), id_gen.clone());
        let q_1 = Context::with_values(q_1_values, q_1_graphs_map.into(), id_gen.clone());
        let p_2 = Context::with_values(p_2_values, p_2_graphs_map.into(), id_gen.clone());
        let q_2 = Context::with_values(q_2_values, q_2_graphs_map.into(), id_gen.clone());

        let (p_1_hat, q_2_hat) = embedding::merge_context(&p_1, &q_1, &p_2, &q_2).unwrap();

        println!(
            "{}",
            dot::Dot::from_graphs_map_with_config(
                &p_1_hat.graphs_map,
                dot::DotConfig {
                    prefix: Some("p1_hat".to_owned()),
                    subgraph: Some(SubgraphConfig {
                        name: "p1_hat".to_owned()
                    }),
                    print_only_content: false
                }
            )
        );
        println!(
            "{}",
            dot::Dot::from_graphs_map_with_config(
                &q_2_hat.graphs_map,
                dot::DotConfig {
                    prefix: Some("q2_hat".to_owned()),
                    subgraph: Some(SubgraphConfig {
                        name: "q2_hat".to_owned()
                    }),
                    print_only_content: false
                }
            )
        );
        dot::Dot::print_footer();
    }
}
