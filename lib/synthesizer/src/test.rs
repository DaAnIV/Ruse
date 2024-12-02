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
        vbool, Cache, CachedString, FieldName, FieldsMap, GraphsMap, Number, ObjectGraph,
        ObjectType, PrimitiveValue,
    };

    use crate::{
        context::{Context, GraphIdGenerator, SynthesizerContext, ValuesMap},
        location::{LocValue, Location, VarLoc},
        opcode::{EvalResult, ExprAst, ExprOpcode},
        prog::SubProgram,
    };

    pub struct TestAst {
        pub code: String,
    }

    impl ExprAst for TestAst {
        fn to_string(&self) -> String {
            self.code.clone()
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
            Box::new(TestAst {
                code: "Test".to_owned(),
            })
        }
    }

    #[derive(Debug)]
    pub struct GetVar {
        arg_types: Vec<ValueType>,
        id: CachedString,
    }

    impl GetVar {
        pub fn new(id: CachedString) -> Self {
            Self {
                id,
                arg_types: vec![],
            }
        }
    }

    impl ExprOpcode for GetVar {
        fn op_name(&self) -> &str {
            "GetVar"
        }

        fn arg_types(&self) -> &[ValueType] {
            &self.arg_types
        }

        fn eval(
            &self,
            _: &[&LocValue],
            post_ctx: &mut Context,
            _: &SynthesizerContext,
        ) -> EvalResult {
            if let Some(var) = post_ctx.get_var_loc_value(&self.id) {
                EvalResult::NoModification(var)
            } else {
                EvalResult::None
            }
        }

        fn to_ast(&self, _: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
            Box::new(TestAst {
                code: self.id.to_string(),
            })
        }
    }

    #[derive(Debug)]
    pub struct SetArgField {
        arg_types: Vec<ValueType>,
        field: FieldName,
        field_new_value: PrimitiveValue,
    }

    impl SetArgField {
        pub fn new(
            obj_type: ObjectType,
            field: FieldName,
            field_new_value: PrimitiveValue,
        ) -> Self {
            Self {
                arg_types: vec![ValueType::Object(obj_type)],
                field,
                field_new_value,
            }
        }
    }

    impl ExprOpcode for SetArgField {
        fn op_name(&self) -> &str {
            "SetArgField"
        }

        fn arg_types(&self) -> &[ValueType] {
            &self.arg_types
        }

        fn eval(
            &self,
            args: &[&LocValue],
            post_ctx: &mut Context,
            _: &SynthesizerContext,
        ) -> EvalResult {
            let obj = args[0].val().obj().unwrap();
            let new_value = Value::Primitive(self.field_new_value.clone());
            let res = post_ctx.set_field(obj.graph_id, obj.node, self.field.clone(), &new_value);
            EvalResult::DirtyContext(post_ctx.temp_value(vbool!(res)))
        }

        fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
            Box::new(TestAst {
                code: format!(
                    "{}.{} = {}",
                    children[0].to_string(),
                    self.field,
                    self.field_new_value
                ),
            })
        }
    }

    #[derive(Debug)]
    pub struct UpdateVarOpcode {
        pub arg_types: Vec<ValueType>,
        pub id: CachedString,
        pub new_value: Value,
        pub returns: EvalResult,
    }

    impl ExprOpcode for UpdateVarOpcode {
        fn op_name(&self) -> &str {
            "UpdateVar"
        }

        fn arg_types(&self) -> &[ValueType] {
            &self.arg_types
        }

        fn eval(
            &self,
            _: &[&LocValue],
            post_ctx: &mut Context,
            syn_ctx: &SynthesizerContext,
        ) -> EvalResult {
            let mut loc = Location::Var(VarLoc {
                var: self.id.clone(),
            });
            post_ctx.update_value(&self.new_value, &mut loc, syn_ctx);
            self.returns.clone()
        }

        fn to_ast(&self, _: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
            Box::new(TestAst {
                code: "UpdateVar".to_string(),
            })
        }
    }

    pub fn evaluate_prog(p: &mut Arc<SubProgram>, syn_ctx: &SynthesizerContext) -> bool {
        unsafe { Arc::get_mut(p).unwrap_unchecked() }.evaluate(syn_ctx)
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

        let mut values = ValuesMap::default();
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

        let mut values = ValuesMap::default();
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
    use crate::{context::ContextArray, test::helpers::*};
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

#[cfg(test)]
mod prog_tests {
    use std::sync::Arc;

    use crate::{
        context::{Context, ContextArray, GraphIdGenerator, SynthesizerContext},
        embedding::merge_context,
        prog::SubProgram,
        test::helpers::*,
    };
    use ruse_object_graph::{str_cached, vnum, vobj, vstr, Cache, GraphsMap, Number, ObjectGraph};

    #[test]
    fn modify_post_ctx_test() {
        let cache = Arc::new(Cache::new());
        let graphs_map = GraphsMap::default();
        let id_gen = Arc::new(GraphIdGenerator::default());

        let pre_ctx = ContextArray::from(vec![Context::with_values(
            [(str_cached!(cache; "x"), vnum!(Number::from(7u64)))].into(),
            graphs_map.into(),
            id_gen,
        )]);
        let post_ctx = pre_ctx.clone();
        let syn_ctx = SynthesizerContext::from_context_array(pre_ctx.clone(), cache);

        let opcode = Arc::new(UpdateVarOpcode {
            arg_types: vec![],
            id: syn_ctx.cached_string("x"),
            new_value: vnum!(Number::from(5)),
            returns: crate::opcode::EvalResult::DirtyContext(
                post_ctx[0].temp_value(vnum!(Number::from(0))),
            ),
        });

        let mut p = SubProgram::with_opcode(opcode, pre_ctx, post_ctx);
        evaluate_prog(&mut p, &syn_ctx);
        println!("{}", p.pre_ctx());
        println!("{}", p.out_value().wrap(p.post_ctx()));
        println!("{}", p.post_ctx());
    }

    #[test]
    fn embedding_test1() {
        let cache = Arc::new(Cache::new());
        let mut graphs_map = GraphsMap::default();
        let id_gen = Arc::new(GraphIdGenerator::default());
        let obj_type = str_cached!(cache; "Foo");
        let field_name = str_cached!(cache; "f");

        let x_graph_id = id_gen.get_id_for_graph();
        let y_graph_id = id_gen.get_id_for_graph();
        let x_node_id = id_gen.get_id_for_node();
        let y_node_id = id_gen.get_id_for_node();

        let mut x_graph = ObjectGraph::new(x_graph_id);
        let mut y_graph = ObjectGraph::new(y_graph_id);

        x_graph.add_object_from_map(
            x_node_id,
            obj_type.clone(),
            [(field_name.clone(), vstr!(cache; "x"))],
        );
        y_graph.add_object_from_map(
            y_node_id,
            obj_type.clone(),
            [(field_name.clone(), vstr!(cache; "y"))],
        );

        graphs_map.insert_graph(x_graph.into());
        graphs_map.insert_graph(y_graph.into());

        let start_ctx = ContextArray::from(vec![Context::with_values(
            [
                (str_cached!(cache; "x"), vobj!(x_graph_id, x_node_id)),
                (str_cached!(cache; "y"), vobj!(y_graph_id, y_node_id)),
            ]
            .into(),
            graphs_map.into(),
            id_gen,
        )]);
        let syn_ctx = SynthesizerContext::from_context_array(start_ctx.clone(), cache);

        let get_x = Arc::new(GetVar::new(syn_ctx.cached_string("x")));
        let get_y = Arc::new(GetVar::new(syn_ctx.cached_string("y")));

        let x_ctx = start_ctx
            .get_partial_context(&[syn_ctx.cached_string("x")])
            .unwrap();
        let y_ctx = start_ctx
            .get_partial_context(&[syn_ctx.cached_string("y")])
            .unwrap();

        let mut p_x = SubProgram::with_opcode(get_x, x_ctx.clone(), x_ctx.clone());
        let mut p_y = SubProgram::with_opcode(get_y, y_ctx.clone(), y_ctx.clone());
        evaluate_prog(&mut p_x, &syn_ctx);
        evaluate_prog(&mut p_y, &syn_ctx);

        println!("p_x: {}", p_x);
        println!("p_y: {}", p_y);

        println!("p_x out: {:?}", p_x.out_value()[0].val());
        println!("p_y out: {:?}", p_y.out_value()[0].val());

        let (pre_ctx, post_ctx) = merge_context(
            &p_x.pre_ctx()[0],
            &p_x.post_ctx()[0],
            &p_y.pre_ctx()[0],
            &p_y.post_ctx()[0],
        )
        .unwrap();

        println!("merged pre: {}", pre_ctx);
        println!("merged post: {}", post_ctx);
        
        println!("p_x out: {:?}", p_x.out_value()[0].val());
        println!("p_y out: {:?}", p_y.out_value()[0].val());
        println!("x: {:?}", post_ctx.get_var_loc_value(&syn_ctx.cached_string("x")).unwrap().val());
        println!("y: {:?}", post_ctx.get_var_loc_value(&syn_ctx.cached_string("y")).unwrap().val());
    }
}
