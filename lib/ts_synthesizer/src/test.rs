#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use object_graph::{str_cached, Number};
    use ruse_object_graph::{
        self as object_graph,
        value::{Value, ValueType},
        vnum, vstr, GraphsMap,
    };
    use ruse_synthesizer::{
        bank::SubsumptionProgBank,
        context::{Context, ContextArray, GraphIdGenerator, SynthesizerContext},
        location::Location,
    };
    use ruse_ts_interpreter::ts_class::TsClassesBuilder;
    use swc_ecma_ast as ast;

    use crate::*;

    use tokio;

    #[tokio::test(flavor = "multi_thread")]
    async fn add_struct_fields() {
        let code = "class User {
            constructor(public name: string, 
                        public surname: string) {}
        }";
        let cache = Arc::new(object_graph::Cache::new());
        let id_gen1 = Arc::new(GraphIdGenerator::default());
        let id_gen2 = Arc::new(GraphIdGenerator::default());
        let mut graphs_map1 = GraphsMap::default();
        let mut graphs_map2 = GraphsMap::default();
        let mut builder = TsClassesBuilder::new();

        let user_class_name = builder
            .add_class(code, &cache)
            .expect("Failed to add User class");

        let classes = builder.finalize(&cache);

        let user_class = classes.get_class(&user_class_name).unwrap();

        let user1_graph_id = id_gen1.get_id_for_graph();
        graphs_map1.ensure_graph(user1_graph_id);
        let user1 = user_class.generate_object(
            HashMap::from([
                (str_cached!(cache; "surname"), vstr!(cache; "Doe")),
                (str_cached!(cache; "name"), vstr!(cache; "John")),
            ]),
            &mut graphs_map1,
            user1_graph_id,
            &id_gen1,
        );
        graphs_map1.set_as_root(str_cached!(cache; "x"), user1.graph_id, user1.node);

        let user2_graph_id = id_gen2.get_id_for_graph();
        graphs_map2.ensure_graph(user2_graph_id);
        let user2 = user_class.generate_object(
            HashMap::from([
                (str_cached!(cache; "surname"), vstr!(cache; "Simon")),
                (str_cached!(cache; "name"), vstr!(cache; "Paul")),
            ]),
            &mut graphs_map2,
            user2_graph_id,
            &id_gen2,
        );
        graphs_map2.set_as_root(str_cached!(cache; "x"), user2.graph_id, user2.node);

        let mut opcodes = construct_opcode_list(
            &[str_cached!(cache; "x")],
            &[],
            &[str_cached!(cache; " ")],
            false,
        );
        add_str_opcodes(&mut opcodes, &ALL_BIN_STR_OPCODES);
        opcodes.extend_from_slice(&user_class.member_opcodes);

        let ctx = ContextArray::from(vec![
            Context::with_values(
                [(str_cached!(cache; "x"), Value::Object(user1))].into(),
                graphs_map1.into(),
                id_gen1,
            ),
            Context::with_values(
                [(str_cached!(cache; "x"), Value::Object(user2))].into(),
                graphs_map2.into(),
                id_gen2,
            ),
        ]);

        let cache_clone = cache.clone();
        let syn_ctx = SynthesizerContext::from_context_array_with_data(ctx.clone(), classes, cache);
        let mut synthesizer = TsSynthesizer::new(
            SubsumptionProgBank::default(),
            syn_ctx,
            opcodes,
            Box::new(move |p, _syn_ctx| {
                let expected_outputs = [
                    str_cached!(cache_clone; "John Doe"),
                    str_cached!(cache_clone; "Paul Simon"),
                ];
                if p.out_type() != &ValueType::String {
                    return false;
                }
                for (v, e) in p.out_value().iter().zip(expected_outputs) {
                    if v.loc() != &Location::Temp {
                        return false;
                    }
                    let v_str = unsafe { v.val().string_value().unwrap_unchecked() };
                    if v_str != e {
                        return false;
                    }
                }
                return true;
            }),
            Box::new(|_p, _syn_ctx| true),
            3,
            1,
        );

        for _ in 1..=5 {
            let res = synthesizer.run_iteration().await;
            if let Some(p) = res {
                assert_eq!(p.get_code(), "(x.name + \" \") + x.surname");
                return;
            }
        }

        assert!(false, "Failed to find program")
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn mutating_object() {
        let code = "class Point {
            constructor(public x: number, 
                        public y: number) {}
        }";
        let cache = Arc::new(object_graph::Cache::new());
        let id_gen1 = Arc::new(GraphIdGenerator::default());
        let id_gen2 = Arc::new(GraphIdGenerator::default());
        let mut graphs_map1 = GraphsMap::default();
        let mut graphs_map2 = GraphsMap::default();
        let mut builder = TsClassesBuilder::new();

        let point_class_name = builder.add_class(code, &cache).unwrap();

        let classes = builder.finalize(&cache);

        let point_class = classes.get_class(&point_class_name).unwrap();

        let point1_graph_id = id_gen1.get_id_for_graph();
        graphs_map1.ensure_graph(point1_graph_id);
        let point1 = point_class.generate_object(
            HashMap::from([
                (str_cached!(cache; "x"), vnum!(Number::from(4))),
                (str_cached!(cache; "y"), vnum!(Number::from(17))),
            ]),
            &mut graphs_map1,
            point1_graph_id,
            &id_gen1,
        );
        graphs_map1.set_as_root(str_cached!(cache; "p"), point1.graph_id, point1.node);

        let point2_graph_id = id_gen2.get_id_for_graph();
        graphs_map2.ensure_graph(point2_graph_id);
        let point2 = point_class.generate_object(
            HashMap::from([
                (str_cached!(cache; "x"), vnum!(Number::from(5))),
                (str_cached!(cache; "y"), vnum!(Number::from(3))),
            ]),
            &mut graphs_map2,
            point2_graph_id,
            &id_gen2,
        );
        graphs_map2.set_as_root(str_cached!(cache; "p"), point2.graph_id, point2.node);

        let mut opcodes = construct_opcode_list(&[str_cached!(cache; "p")], &[], &[], false);
        add_num_opcodes(
            &mut opcodes,
            &[ast::BinaryOp::Add],
            &[],
            &[ast::UpdateOp::PlusPlus],
        );
        opcodes.extend_from_slice(&point_class.member_opcodes);

        let ctx = ContextArray::from(vec![
            Context::with_values(
                [(str_cached!(cache; "p"), Value::Object(point1))].into(),
                graphs_map1.into(),
                id_gen1,
            ),
            Context::with_values(
                [(str_cached!(cache; "p"), Value::Object(point2))].into(),
                graphs_map2.into(),
                id_gen2,
            ),
        ]);

        let syn_ctx = SynthesizerContext::from_context_array_with_data(ctx.clone(), classes, cache);
        let mut synthesizer = TsSynthesizer::new(
            SubsumptionProgBank::default(),
            syn_ctx,
            opcodes,
            Box::new(move |p, _syn_ctx| {
                let expected_outputs = [Number::from(10), Number::from(12)];
                if p.out_type() != &ValueType::Number {
                    return false;
                }
                for (v, e) in p.out_value().iter().zip(expected_outputs) {
                    if v.loc() != &Location::Temp {
                        return false;
                    }
                    let v_num = unsafe { v.val().number_value().unwrap_unchecked() };
                    if v_num != e {
                        return false;
                    }
                }
                return true;
            }),
            Box::new(|_p, _syn_ctx| true),
            3,
            1,
        );

        for _ in 1..=5 {
            let res = synthesizer.run_iteration().await;
            if let Some(p) = res {
                assert_eq!(p.get_code(), "(++p.x) + p.x");
                return;
            }
        }

        assert!(false, "Failed to find program")
    }
}
