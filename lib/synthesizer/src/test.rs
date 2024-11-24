#[cfg(feature = "test_helpers")]
pub mod helpers {
    use std::{
        hash::{DefaultHasher, Hash, Hasher},
        sync::Arc,
    };

    use rand::{seq::IteratorRandom, Rng};
    use ruse_object_graph::{
        generator::object_graph_generator::generate_random_str,
        scached,
        value::{ObjectValue, Value, ValueType},
        Cache, CachedString, FieldsMap, GraphsMap, Number, ObjectGraph, PrimitiveValue,
    };

    use crate::{
        context::{Context, GraphIdGenerator, SynthesizerContext, ValuesMap},
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
        fn op_name(&self) -> &str {
            "Test"
        }

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

    pub fn calculate_hash<T: Hash>(t: &T) -> u64 {
        let mut s = DefaultHasher::new();
        t.hash(&mut s);
        s.finish()
    }

    fn generate_random_primitive_value<R: Rng + ?Sized>(
        rng: &mut R,
        cache: &Cache,
    ) -> PrimitiveValue {
        let types = [ValueType::Bool, ValueType::Number, ValueType::String];
        match types.iter().choose(rng).unwrap() {
            ValueType::Number => PrimitiveValue::Number(Number::from(rng.next_u64())),
            ValueType::Bool => PrimitiveValue::Bool(rng.gen_bool(0.5)),
            ValueType::String => {
                PrimitiveValue::String(scached!(cache; generate_random_str(4, rng)))
            }
            _ => unreachable!(),
        }
    }

    fn generate_fields<R: Rng + ?Sized>(count: usize, rng: &mut R, cache: &Cache) -> FieldsMap {
        let mut fields = FieldsMap::new();
        for _ in 0..count {
            let key = scached!(cache; generate_random_str(4, rng));
            let value = generate_random_primitive_value(rng, cache);
            fields.insert(key, value);
        }

        fields
    }

    pub fn generate_random_object_value<R: Rng + ?Sized>(
        root_name: CachedString,
        rng: &mut R,
        graphs_map: &mut GraphsMap,
        graph_id_gen: &GraphIdGenerator,
        cache: &Cache,
    ) -> ObjectValue {
        let graph_id = graph_id_gen.get_id_for_graph();
        let root_id = graph_id_gen.get_id_for_node();
        let mut graph = ObjectGraph::new(graph_id);

        let fields = generate_fields(4, rng, cache);
        let obj_type = scached!(cache; generate_random_str(4, rng));
        graph.add_node(root_id, obj_type, fields);
        graph.set_as_root(root_name, root_id);

        for _ in 0..4 {
            let neig_id = graph_id_gen.get_id_for_node();
            let fields = generate_fields(4, rng, cache);
            let obj_type = scached!(cache; generate_random_str(4, rng));
            graph.add_node(neig_id, obj_type, fields);

            let edge_name = scached!(cache; generate_random_str(4, rng));
            graph.set_edge(&root_id, neig_id, edge_name);
        }

        graphs_map.insert_graph(graph.into());

        ObjectValue {
            graph_id: graph_id,
            node: root_id,
        }
    }

    pub fn generate_random_context<R: Rng + ?Sized>(
        rng: &mut R,
        num_primitive: usize,
        num_objects: usize,
        cache: &Cache,
    ) -> Context {
        let graph_id_gen = Arc::new(GraphIdGenerator::default());
        let mut graphs_map = GraphsMap::default();

        let mut values = ValuesMap::new();
        for _ in 0..num_primitive {
            let key = scached!(cache; generate_random_str(4, rng));
            let value = generate_random_primitive_value(rng, cache);
            values.insert(key, Value::Primitive(value));
        }
        for _ in 0..num_objects {
            let key = scached!(cache; generate_random_str(4, rng));
            let value = generate_random_object_value(
                key.clone(),
                rng,
                &mut graphs_map,
                &graph_id_gen,
                cache,
            );
            values.insert(key, Value::Object(value));
        }

        Context::with_values(values, graphs_map.into(), graph_id_gen)
    }

    pub fn generate_context_from_array<I>(
        key: CachedString,
        elem_type: &ValueType,
        elements: I,
        cache: &Cache,
    ) -> Context
    where
        I: IntoIterator<Item = Value>,
    {
        let graph_id_gen = Arc::new(GraphIdGenerator::default());
        let mut graphs_map = GraphsMap::default();

        let mut values = ValuesMap::new();
        let graph_id = graph_id_gen.get_id_for_graph();
        let mut graph = ObjectGraph::new(graph_id);
        let node =
            graph.add_array_object(graph_id_gen.get_id_for_node(), elem_type, elements, cache);
        graphs_map.insert_graph(graph.into());
        values.insert(key, Value::Object(ObjectValue { graph_id, node }));

        Context::with_values(values, graphs_map.into(), graph_id_gen)
    }
}

#[cfg(test)]
mod bank_iterator_tests {
    use crate::{
        bank::ProgramsMap, bank_iterator::bank_iterator, context::GraphIdGenerator,
        multi_programs_map_product::multi_programs_map_product, test::helpers::*,
    };
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

    async fn run_gatherer(bank: &ProgBank, op: &Arc<dyn ExprOpcode>) -> Vec<Vec<Arc<SubProgram>>> {
        let all_children = Arc::new(DashMap::<usize, Vec<Arc<SubProgram>>>::default());
        for triplet in bank_iterator(bank, op.arg_types()) {
            all_children.insert(all_children.len(), triplet.children);
        }

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

    fn create_programs_map(n: usize, syn_ctx: &SynthesizerContext) -> ProgramsMap {
        let map = ProgramsMap::default();
        for i in 0..n {
            let value = Number::from(i);
            let p = get_prog_for_bank(vnum!(value), syn_ctx);
            map.0.insert(p.clone().into(), p);
        }
        map
    }

    #[test]
    fn programs_map_ref_iterator() {
        let cache = Arc::new(Cache::new());
        let _id_gen = GraphIdGenerator::default();
        let syn_ctx = SynthesizerContext::from_context_array(ContextArray::default(), cache);
        let map = create_programs_map(5, &syn_ctx);
        for p in map.iter() {
            println!("{}", p.out_value()[0].val.number_value().unwrap());
        }
    }

    #[test]
    fn programs_map_ref_2_iterator() {
        let cache = Arc::new(Cache::new());
        let _id_gen = GraphIdGenerator::default();
        let syn_ctx = SynthesizerContext::from_context_array(ContextArray::default(), cache);
        let map = create_programs_map(5, &syn_ctx);
        for (p1, p2) in map.iter().zip(map.iter()) {
            let n1 = p1.out_value()[0].val.number_value().unwrap();
            let n2 = p2.out_value()[0].val.number_value().unwrap();
            println!("({}, {})", n1, n2);
        }
    }

    #[test]
    fn programs_map_multi_iter() {
        let cache = Arc::new(Cache::new());
        let _id_gen = GraphIdGenerator::default();
        let syn_ctx = SynthesizerContext::from_context_array(ContextArray::default(), cache);
        let map = create_programs_map(2, &syn_ctx);
        let map_ptr = std::ptr::from_ref(&map);
        for triplet in multi_programs_map_product([map_ptr, map_ptr].into_iter()) {
            println!(
                "{:#?}",
                triplet
                    .children
                    .iter()
                    .map(|p| p.out_value()[0].val.number_value().unwrap())
                    .collect_vec()
            );
        }
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

        let all_children = run_gatherer(&bank, &bin_op).await;
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

        let all_children = run_gatherer(&bank, &bin_op).await;
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

        let all_children = run_gatherer(&bank, &bin_op).await;
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

        let all_children = run_gatherer(&bank, &tri_op).await;
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
}

#[cfg(test)]
mod context_tests {
    use crate::{
        context::ContextArray,
        test::helpers::*,
    };
    use rand::{rngs::StdRng, SeedableRng};
    use ruse_object_graph::{str_cached, value::ValueType, vstr, Cache};

    const SEED: u64 = 10;

    #[test]
    fn simple_context_eq_self_test() {
        let mut rng = StdRng::seed_from_u64(SEED);
        let cache = Cache::new();

        let ctx = generate_random_context(&mut rng, 4, 0, &cache);

        assert_eq!(calculate_hash(&ctx), calculate_hash(&ctx));
        assert_eq!(ctx, ctx);
    }

    #[test]
    fn simple_context_eq_test() {
        let mut rng1 = StdRng::seed_from_u64(SEED);
        let mut rng2 = StdRng::seed_from_u64(SEED);
        let cache = Cache::new();

        let ctx1 = generate_random_context(&mut rng1, 4, 0, &cache);
        let ctx2 = generate_random_context(&mut rng2, 4, 0, &cache);

        assert_eq!(calculate_hash(&ctx1), calculate_hash(&ctx2));
        assert_eq!(ctx1, ctx2);
    }

    #[test]
    fn simple_context_array_eq_test() {
        let mut rng1 = StdRng::seed_from_u64(SEED);
        let mut rng2 = StdRng::seed_from_u64(SEED);
        let cache = Cache::new();

        let ctx_arr1 = ContextArray::from(vec![
            generate_random_context(&mut rng1, 4, 0, &cache),
            generate_random_context(&mut rng1, 5, 0, &cache),
            generate_random_context(&mut rng1, 4, 0, &cache),
        ]);
        let ctx_arr2 = ContextArray::from(vec![
            generate_random_context(&mut rng2, 4, 0, &cache),
            generate_random_context(&mut rng2, 5, 0, &cache),
            generate_random_context(&mut rng2, 4, 0, &cache),
        ]);

        assert_eq!(calculate_hash(&ctx_arr1), calculate_hash(&ctx_arr2));
        assert_eq!(ctx_arr1, ctx_arr2);
    }

    #[test]
    fn context_eq_self_test() {
        let mut rng = StdRng::seed_from_u64(SEED);
        let cache = Cache::new();

        let ctx = generate_random_context(&mut rng, 4, 4, &cache);

        assert_eq!(calculate_hash(&ctx), calculate_hash(&ctx));
        assert_eq!(ctx, ctx);
    }

    #[test]
    fn context_eq_test() {
        let mut rng1 = StdRng::seed_from_u64(SEED);
        let mut rng2 = StdRng::seed_from_u64(SEED);
        let cache = Cache::new();

        let ctx1 = generate_random_context(&mut rng1, 4, 4, &cache);
        let ctx2 = generate_random_context(&mut rng2, 4, 4, &cache);

        assert_eq!(calculate_hash(&ctx1), calculate_hash(&ctx2));
        assert_eq!(ctx1, ctx2);
    }

    #[test]
    fn context_array_eq_test() {
        let mut rng1 = StdRng::seed_from_u64(SEED);
        let mut rng2 = StdRng::seed_from_u64(SEED);
        let cache = Cache::new();

        let ctx_arr1 = ContextArray::from(vec![
            generate_random_context(&mut rng1, 4, 4, &cache),
            generate_random_context(&mut rng1, 5, 5, &cache),
            generate_random_context(&mut rng1, 4, 4, &cache),
        ]);
        let ctx_arr2 = ContextArray::from(vec![
            generate_random_context(&mut rng2, 4, 4, &cache),
            generate_random_context(&mut rng2, 5, 5, &cache),
            generate_random_context(&mut rng2, 4, 4, &cache),
        ]);

        assert_eq!(calculate_hash(&ctx_arr1), calculate_hash(&ctx_arr2));
        assert_eq!(ctx_arr1, ctx_arr2);
    }

    #[test]
    fn context_with_array_value_eq_test() {
        let cache = Cache::new();

        let ctx1 = generate_context_from_array(
            str_cached!(cache; "names"),
            &ValueType::String,
            ["Augusta", "Ada", "King"].iter().map(|s| vstr!(cache; s)),
            &cache,
        );

        let ctx2 = generate_context_from_array(
            str_cached!(cache; "names"),
            &ValueType::String,
            ["Augusta", "Ada", "King"].iter().map(|s| vstr!(cache; s)),
            &cache,
        );

        assert_eq!(calculate_hash(&ctx1), calculate_hash(&ctx2));
        assert_eq!(ctx1, ctx2);
    }
}
