#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use object_graph::{str_cached, Number};
    use ruse_bank_in_mem::subsumption_bank::SubsumptionProgBank;
    use ruse_object_graph::{
        self as object_graph, class_name, field_name, location::Location, root_name, value::Value,
        vnum, vstr, GraphIdGenerator, GraphsMap, ValueType,
    };
    use ruse_synthesizer::context::{Context, ContextArray};
    use ruse_synthesizer::synthesizer::SynthesizerOptions;
    use ruse_synthesizer::synthesizer_context::SynthesizerContext;
    use ruse_ts_interpreter::ts_classes::TsClassesBuilder;
    use swc_ecma_ast as ast;

    use crate::*;

    use tokio;

    #[tokio::test(flavor = "multi_thread")]
    async fn add_struct_fields() {
        let code = "export class User {
            constructor(public name: string, 
                        public surname: string) {}
        }";
        let id_gen1 = Arc::new(GraphIdGenerator::default());
        let id_gen2 = Arc::new(GraphIdGenerator::default());
        let mut graphs_map1 = GraphsMap::default();
        let mut graphs_map2 = GraphsMap::default();
        let mut builder = TsClassesBuilder::new();

        builder.add_classes(code).expect("Failed to add User class");
        let classes = builder.finalize().unwrap();

        let user_class_name = class_name!("User");
        let user_class = classes.get_user_class(&user_class_name).unwrap();

        let user1_graph_id = id_gen1.get_id_for_graph();
        graphs_map1.ensure_graph(user1_graph_id);
        let user1 = user_class.generate_object(
            HashMap::from([
                (field_name!("surname"), vstr!("Doe")),
                (field_name!("name"), vstr!("John")),
            ]),
            &mut graphs_map1,
            user1_graph_id,
            &id_gen1,
        );
        graphs_map1.set_as_root(root_name!("x"), user1.graph_id, user1.node);

        let user2_graph_id = id_gen2.get_id_for_graph();
        graphs_map2.ensure_graph(user2_graph_id);
        let user2 = user_class.generate_object(
            HashMap::from([
                (field_name!("surname"), vstr!("Simon")),
                (field_name!("name"), vstr!("Paul")),
            ]),
            &mut graphs_map2,
            user2_graph_id,
            &id_gen2,
        );
        graphs_map2.set_as_root(root_name!("x"), user2.graph_id, user2.node);

        let mut opcodes =
            construct_opcode_list(&[root_name!("x")], &[], &[str_cached!(" ")], false);
        add_str_opcodes(&mut opcodes, &ALL_BIN_STR_OPCODES);
        opcodes.extend_from_slice(&user_class.member_opcodes);

        let ctx = ContextArray::from(vec![
            Context::with_values(
                [(root_name!("x"), Value::Object(user1))].into(),
                graphs_map1.into(),
                id_gen1,
            ),
            Context::with_values(
                [(root_name!("x"), Value::Object(user2))].into(),
                graphs_map2.into(),
                id_gen2,
            ),
        ]);

        let syn_ctx = SynthesizerContext::from_context_array_with_data(ctx.clone(), classes);
        let mut synthesizer = create_ts_synthesizer(
            SubsumptionProgBank::default(),
            syn_ctx,
            opcodes,
            Box::new(move |p, _syn_ctx, _worker_ctx| {
                let expected_outputs = [str_cached!("John Doe"), str_cached!("Paul Simon")];
                if p.out_type() != &ValueType::String {
                    return false;
                }
                for (v, e) in p.out_value().iter().zip(expected_outputs) {
                    if matches!(v.loc(), Location::Temp) {
                        return false;
                    }
                    let v_str = unsafe { v.val().string_value().unwrap_unchecked() };
                    if v_str != e {
                        return false;
                    }
                }
                return true;
            }),
            Box::new(|_p, _syn_ctx, _worker_ctx| true),
            SynthesizerOptions {
                worker_count: 1,
                max_mutations: 3,
                output_embedding_overhead: None,
            },
        );

        for _ in 1..=5 {
            let res = synthesizer.run_iteration().await;
            if let Ok(Some(p)) = res {
                assert_eq!(p.get_code(), "(x.name + \" \") + x.surname");
                return;
            }
        }

        assert!(false, "Failed to find program")
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn mutating_object() {
        let code = "export class Point {
            constructor(public x: number, 
                        public y: number) {}
        }";
        let id_gen1 = Arc::new(GraphIdGenerator::default());
        let id_gen2 = Arc::new(GraphIdGenerator::default());
        let mut graphs_map1 = GraphsMap::default();
        let mut graphs_map2 = GraphsMap::default();
        let mut builder = TsClassesBuilder::new();

        builder.add_classes(code).unwrap();
        let classes = builder.finalize().unwrap();

        let point_class_name = class_name!("Point");
        let point_class = classes.get_user_class(&point_class_name).unwrap();

        let point1_graph_id = id_gen1.get_id_for_graph();
        graphs_map1.ensure_graph(point1_graph_id);
        let point1 = point_class.generate_object(
            HashMap::from([
                (field_name!("x"), vnum!(Number::from(4))),
                (field_name!("y"), vnum!(Number::from(17))),
            ]),
            &mut graphs_map1,
            point1_graph_id,
            &id_gen1,
        );
        graphs_map1.set_as_root(root_name!("p"), point1.graph_id, point1.node);

        let point2_graph_id = id_gen2.get_id_for_graph();
        graphs_map2.ensure_graph(point2_graph_id);
        let point2 = point_class.generate_object(
            HashMap::from([
                (field_name!("x"), vnum!(Number::from(5))),
                (field_name!("y"), vnum!(Number::from(3))),
            ]),
            &mut graphs_map2,
            point2_graph_id,
            &id_gen2,
        );
        graphs_map2.set_as_root(root_name!("p"), point2.graph_id, point2.node);

        let mut opcodes = construct_opcode_list(&[root_name!("p")], &[], &[], false);
        add_num_opcodes(
            &mut opcodes,
            &[ast::BinaryOp::Add],
            &[],
            &[ast::UpdateOp::PlusPlus],
        );
        opcodes.extend_from_slice(&point_class.member_opcodes);

        let ctx = ContextArray::from(vec![
            Context::with_values(
                [(root_name!("p"), Value::Object(point1))].into(),
                graphs_map1.into(),
                id_gen1,
            ),
            Context::with_values(
                [(root_name!("p"), Value::Object(point2))].into(),
                graphs_map2.into(),
                id_gen2,
            ),
        ]);

        let syn_ctx = SynthesizerContext::from_context_array_with_data(ctx.clone(), classes);
        let mut synthesizer = create_ts_synthesizer(
            SubsumptionProgBank::default(),
            syn_ctx,
            opcodes,
            Box::new(move |p, _syn_ctx, _worker_ctx| {
                let expected_outputs = [Number::from(10), Number::from(12)];
                if p.out_type() != &ValueType::Number {
                    return false;
                }
                for (v, e) in p.out_value().iter().zip(expected_outputs) {
                    if matches!(v.loc(), Location::Temp) {
                        return false;
                    }
                    let v_num = unsafe { v.val().number_value().unwrap_unchecked() };
                    if v_num != e {
                        return false;
                    }
                }
                return true;
            }),
            Box::new(|_p, _syn_ctx, _worker_ctx| true),
            SynthesizerOptions {
                worker_count: 1,
                max_mutations: 3,
                output_embedding_overhead: None,
            },
        );

        for _ in 1..=5 {
            let res = synthesizer.run_iteration().await;
            if let Ok(Some(p)) = res {
                assert_eq!(p.get_code(), "(++p.x) + p.x");
                return;
            }
        }

        assert!(false, "Failed to find program")
    }
}
