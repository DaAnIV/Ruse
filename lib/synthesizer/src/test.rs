#[cfg(feature = "test_helpers")]
#[allow(dead_code)]
pub mod helpers {
    use std::{
        collections::HashMap,
        hash::{DefaultHasher, Hash, Hasher},
        sync::Arc,
    };

    use rand::{seq::IteratorRandom, Rng};
    use ruse_object_graph::{
        field_name,
        generator::object_graph_generator::generate_random_str,
        location::{LocValue, Location, ObjectFieldLoc, RootLoc},
        root_name, str_cached,
        value::{ObjectValue, Value},
        vbool, FieldName, FieldsMap, GraphIdGenerator, GraphsMap, Number, ObjectType,
        PrimitiveValue, RootName, ValueType,
    };

    use crate::{
        context::{Context, ContextArray, ValuesMap, VariableName},
        dirty,
        embedding::merge_context_arrays,
        opcode::{EvalResult, ExprAst, ExprOpcode},
        partial_context::PartialContextBuilder,
        prog::SubProgram,
        pure,
        synthesizer_context::{SynthesizerContext, SynthesizerWorkerContext},
    };
    use tracing::level_filters::LevelFilter;
    use tracing_subscriber::{filter::Targets, prelude::*};

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

        fn eval(
            &self,
            _: &[&LocValue],
            _: &mut Context,
            _: &SynthesizerContext,
            _: &mut SynthesizerWorkerContext,
        ) -> EvalResult {
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
        id: VariableName,
    }

    impl GetVar {
        pub fn new(id: VariableName) -> Self {
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
            syn_ctx: &SynthesizerContext,
            _: &mut SynthesizerWorkerContext,
        ) -> EvalResult {
            if let Some(var) = post_ctx.get_var_loc_value(&self.id, syn_ctx.variables()) {
                pure!(var)
            } else {
                Err(())
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
            syn_ctx: &SynthesizerContext,
            _: &mut SynthesizerWorkerContext,
        ) -> EvalResult {
            let obj = args[0].val().obj().unwrap();
            let new_value = Value::Primitive(self.field_new_value.clone());
            let mut loc = Location::ObjectField(ObjectFieldLoc {
                graph: obj.graph_id,
                node: obj.node,
                field: self.field.clone(),
                attrs: Default::default(),
            });
            post_ctx.update_value(&new_value, &mut loc, syn_ctx.variables())?;
            dirty!(post_ctx.temp_value(vbool!(true)))
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
        pub id: VariableName,
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
            _: &mut SynthesizerWorkerContext,
        ) -> EvalResult {
            let mut loc = Location::Root(RootLoc {
                root: self.id.clone(),
                attrs: Default::default(),
            });
            post_ctx
                .update_value(&self.new_value, &mut loc, syn_ctx.variables())
                .expect("Failed to update var");
            self.returns.clone()
        }

        fn to_ast(&self, _: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
            Box::new(TestAst {
                code: "UpdateVar".to_string(),
            })
        }
    }

    pub fn evaluate_prog(
        p: &mut Arc<SubProgram>,
        syn_ctx: &SynthesizerContext,
        worker_ctx: &mut SynthesizerWorkerContext,
    ) -> bool {
        unsafe { Arc::get_mut(p).unwrap_unchecked() }.evaluate(syn_ctx, worker_ctx)
    }

    pub fn calculate_hash<T: Hash>(t: &T) -> u64 {
        let mut s = DefaultHasher::new();
        t.hash(&mut s);
        s.finish()
    }

    fn generate_random_primitive_value<R: Rng + ?Sized>(rng: &mut R) -> PrimitiveValue {
        let types = [ValueType::Bool, ValueType::Number, ValueType::String];
        match types.iter().choose(rng).unwrap() {
            ValueType::Number => PrimitiveValue::Number(Number::from(rng.next_u64())),
            ValueType::Bool => PrimitiveValue::Bool(rng.gen_bool(0.5)),
            ValueType::String => PrimitiveValue::String(str_cached!(generate_random_str(4, rng))),
            _ => unreachable!(),
        }
    }

    fn generate_fields<R: Rng + ?Sized>(count: usize, rng: &mut R) -> FieldsMap {
        let mut fields = FieldsMap::new();
        for _ in 0..count {
            let key = field_name!(generate_random_str(4, rng));
            let value = generate_random_primitive_value(rng);
            fields.insert(key, value.into());
        }

        fields
    }

    pub fn generate_random_object_value<R: Rng + ?Sized>(
        root_name: RootName,
        rng: &mut R,
        graphs_map: &mut GraphsMap,
        graph_id_gen: &GraphIdGenerator,
    ) -> ObjectValue {
        let graph_id = graph_id_gen.get_id_for_graph();
        let root_id = graph_id_gen.get_id_for_node();
        graphs_map.ensure_graph(graph_id);

        let fields = generate_fields(4, rng);
        let obj_type = ObjectType::class_obj_type(&generate_random_str(4, rng));
        graphs_map.construct_node(graph_id, root_id, obj_type.clone(), fields);

        for _ in 0..4 {
            let neig_id = graph_id_gen.get_id_for_node();
            let fields = generate_fields(4, rng);
            let obj_type = ObjectType::class_obj_type(&generate_random_str(4, rng));
            graphs_map.construct_node(graph_id, neig_id, obj_type, fields);

            let edge_name = field_name!(generate_random_str(4, rng));
            graphs_map.set_edge(edge_name, graph_id, root_id, graph_id, neig_id);
        }

        graphs_map.set_as_root(root_name, graph_id, root_id);

        ObjectValue {
            obj_type: obj_type,
            graph_id: graph_id,
            node: root_id,
        }
    }

    pub fn generate_random_context<R: Rng + ?Sized>(
        rng: &mut R,
        num_primitive: usize,
        num_objects: usize,
    ) -> Box<Context> {
        let graph_id_gen = Arc::new(GraphIdGenerator::default());
        let mut graphs_map = GraphsMap::default();

        let mut values = ValuesMap::default();
        for _ in 0..num_primitive {
            let key = root_name!(generate_random_str(4, rng));
            let value = generate_random_primitive_value(rng);
            values.insert(key, Value::Primitive(value));
        }
        for _ in 0..num_objects {
            let key = root_name!(generate_random_str(4, rng));
            let value =
                generate_random_object_value(key.clone(), rng, &mut graphs_map, &graph_id_gen);
            values.insert(key, Value::Object(value));
        }

        Context::with_values(values, graphs_map.into(), graph_id_gen)
    }

    pub fn add_array_to_values<I>(
        values: &mut ValuesMap,
        graphs_map: &mut GraphsMap,
        graph_id_gen: &Arc<GraphIdGenerator>,
        key: RootName,
        elem_type: &ValueType,
        elements: I,
    ) where
        I: IntoIterator<Item = Value>,
    {
        let graph_id = graph_id_gen.get_id_for_graph();
        let node_id = graph_id_gen.get_id_for_node();
        graphs_map.ensure_graph(graph_id);
        let node = graphs_map.add_array_object(graph_id, node_id, elem_type, elements);

        graphs_map.set_as_root(key.clone(), graph_id, node);
        values.insert(
            key,
            Value::Object(ObjectValue {
                obj_type: ObjectType::array_obj_type(elem_type),
                graph_id,
                node,
            }),
        );
    }

    pub fn generate_context_from_array<I>(
        key: RootName,
        elem_type: &ValueType,
        elements: I,
    ) -> Box<Context>
    where
        I: IntoIterator<Item = Value>,
    {
        let graph_id_gen: Arc<GraphIdGenerator> = Arc::new(GraphIdGenerator::default());
        let mut graphs_map = GraphsMap::default();
        let mut values = ValuesMap::default();

        add_array_to_values(
            &mut values,
            &mut graphs_map,
            &graph_id_gen,
            key,
            elem_type,
            elements,
        );

        Context::with_values(values, graphs_map.into(), graph_id_gen)
    }

    pub fn get_init_prog(
        op: Arc<dyn ExprOpcode>,
        ctx_arr: &ContextArray,
        syn_ctx: &SynthesizerContext,
        worker_ctx: &mut SynthesizerWorkerContext,
    ) -> Arc<SubProgram> {
        let partial_context_builder = PartialContextBuilder::new(ctx_arr);
        let op_ctx = partial_context_builder
            .get_partial_context(op.required_variables())
            .unwrap();
        let mut prog = SubProgram::with_opcode(op, op_ctx.clone(), op_ctx.clone());
        assert!(evaluate_prog(&mut prog, &syn_ctx, worker_ctx));
        prog
    }

    pub fn get_composite_prog(
        op: Arc<dyn ExprOpcode>,
        children: Vec<Arc<SubProgram>>,
        syn_ctx: &SynthesizerContext,
        worker_ctx: &mut SynthesizerWorkerContext,
    ) -> Arc<SubProgram> {
        let mut pre = children[0].pre_ctx().clone();
        let mut post = children[0].post_ctx().clone();

        for child in children.iter().skip(1) {
            (pre, post) =
                merge_context_arrays(&pre, &post, child.pre_ctx(), child.post_ctx()).unwrap();
        }

        let mut prog = SubProgram::with_opcode_and_children(op, children, pre, post);
        assert!(evaluate_prog(&mut prog, &syn_ctx, worker_ctx));
        prog
    }

    pub struct OpChain {
        pub name: String,
        pub op: Arc<dyn ExprOpcode>,
        pub children_ops: Vec<OpChain>,
    }

    #[macro_export]
    macro_rules! op_chain {
        ($name:expr, $op:expr) => {
            $crate::test_helpers::OpChain {
                name: $name.to_owned(),
                op: $op.clone(),
                children_ops: vec![]
            }
        };
        ($name:expr, $op:expr; $($children:expr),*) => {
            $crate::test_helpers::OpChain {
                name: $name.to_owned(),
                op: $op.clone(),
                children_ops: vec![$($children),*]
            }
        };
    }

    fn evaluate_chain_inner(
        chain: &OpChain,
        ctx: &ContextArray,
        syn_ctx: &SynthesizerContext,
        worker_ctx: &mut SynthesizerWorkerContext,
        results: &mut HashMap<String, Arc<SubProgram>>,
    ) -> Arc<SubProgram> {
        let prog = if chain.children_ops.is_empty() {
            get_init_prog(chain.op.clone(), ctx, syn_ctx, worker_ctx)
        } else {
            let children = chain
                .children_ops
                .iter()
                .map(|child_op| evaluate_chain_inner(child_op, ctx, syn_ctx, worker_ctx, results));
            get_composite_prog(chain.op.clone(), children.collect(), syn_ctx, worker_ctx)
        };

        results.insert(chain.name.clone(), prog.clone());
        prog
    }

    pub fn evaluate_chain(
        chain: OpChain,
        ctx: &ContextArray,
        syn_ctx: &SynthesizerContext,
        worker_ctx: &mut SynthesizerWorkerContext,
    ) -> HashMap<String, Arc<SubProgram>> {
        let mut results = HashMap::default();
        evaluate_chain_inner(&chain, ctx, syn_ctx, worker_ctx, &mut results);

        results
    }

    pub fn init_log() {
        let verbose_filter = Targets::default()
            .with_target("ruse", LevelFilter::TRACE)
            .with_default(LevelFilter::OFF);
        let console_layer = tracing_subscriber::fmt::layer()
            .compact()
            .with_file(true)
            .with_line_number(true)
            .with_thread_ids(true)
            .with_filter(verbose_filter);

        tracing_subscriber::registry().with(console_layer).init();
    }
}


#[cfg(test)]
mod context_tests {
    use crate::{context::ContextArray, test::helpers::*};
    use rand::{rngs::StdRng, SeedableRng};
    use ruse_object_graph::{root_name, vstr, ValueType};

    const SEED: u64 = 10;

    #[test]
    fn simple_context_eq_self_test() {
        let mut rng = StdRng::seed_from_u64(SEED);

        let ctx = generate_random_context(&mut rng, 4, 0);

        assert_eq!(calculate_hash(&ctx), calculate_hash(&ctx));
        assert_eq!(ctx, ctx);
    }

    #[test]
    fn simple_context_eq_test() {
        let mut rng1 = StdRng::seed_from_u64(SEED);
        let mut rng2 = StdRng::seed_from_u64(SEED);

        let ctx1 = generate_random_context(&mut rng1, 4, 0);
        let ctx2 = generate_random_context(&mut rng2, 4, 0);

        assert_eq!(calculate_hash(&ctx1), calculate_hash(&ctx2));
        assert_eq!(ctx1, ctx2);
    }

    #[test]
    fn simple_context_array_eq_test() {
        let mut rng1 = StdRng::seed_from_u64(SEED);
        let mut rng2 = StdRng::seed_from_u64(SEED);

        let ctx_arr1 = ContextArray::from(vec![
            generate_random_context(&mut rng1, 4, 0),
            generate_random_context(&mut rng1, 5, 0),
            generate_random_context(&mut rng1, 4, 0),
        ]);
        let ctx_arr2 = ContextArray::from(vec![
            generate_random_context(&mut rng2, 4, 0),
            generate_random_context(&mut rng2, 5, 0),
            generate_random_context(&mut rng2, 4, 0),
        ]);

        assert_eq!(calculate_hash(&ctx_arr1), calculate_hash(&ctx_arr2));
        assert_eq!(ctx_arr1, ctx_arr2);
    }

    #[test]
    fn context_eq_self_test() {
        let mut rng = StdRng::seed_from_u64(SEED);

        let ctx = generate_random_context(&mut rng, 4, 4);

        assert_eq!(calculate_hash(&ctx), calculate_hash(&ctx));
        assert_eq!(ctx, ctx);
    }

    #[test]
    fn context_eq_test() {
        let mut rng1 = StdRng::seed_from_u64(SEED);
        let mut rng2 = StdRng::seed_from_u64(SEED);

        let ctx1 = generate_random_context(&mut rng1, 4, 4);
        let ctx2 = generate_random_context(&mut rng2, 4, 4);

        assert_eq!(calculate_hash(&ctx1), calculate_hash(&ctx2));
        assert_eq!(ctx1, ctx2);
    }

    #[test]
    fn context_array_eq_test() {
        let mut rng1 = StdRng::seed_from_u64(SEED);
        let mut rng2 = StdRng::seed_from_u64(SEED);

        let ctx_arr1 = ContextArray::from(vec![
            generate_random_context(&mut rng1, 4, 4),
            generate_random_context(&mut rng1, 5, 5),
            generate_random_context(&mut rng1, 4, 4),
        ]);
        let ctx_arr2 = ContextArray::from(vec![
            generate_random_context(&mut rng2, 4, 4),
            generate_random_context(&mut rng2, 5, 5),
            generate_random_context(&mut rng2, 4, 4),
        ]);

        assert_eq!(calculate_hash(&ctx_arr1), calculate_hash(&ctx_arr2));
        assert_eq!(ctx_arr1, ctx_arr2);
    }

    #[test]
    fn context_with_array_value_eq_test() {
        let ctx1 = generate_context_from_array(
            root_name!("names"),
            &ValueType::String,
            ["Augusta", "Ada", "King"].iter().map(|s| vstr!(*s)),
        );

        let ctx2 = generate_context_from_array(
            root_name!("names"),
            &ValueType::String,
            ["Augusta", "Ada", "King"].iter().map(|s| vstr!(*s)),
        );

        assert_eq!(calculate_hash(&ctx1), calculate_hash(&ctx2));
        assert_eq!(ctx1, ctx2);
    }
}

#[cfg(test)]
mod embedding_tests {
    use std::sync::Arc;

    use itertools::Itertools;
    use ruse_object_graph::{
        field_name, graph_map_value::GraphMapWrap, root_name, vobj, vstr, GraphIdGenerator,
        GraphsMap, ObjectType, ValueType,
    };

    use crate::{
        context::{Context, ContextArray, Variable},
        embedding::merge_context,
        partial_context::PartialContextBuilder,
        synthesizer_context::SynthesizerContext,
    };

    #[test]
    fn simple_embedding_test1() {
        let mut graphs_map = GraphsMap::default();
        let id_gen = Arc::new(GraphIdGenerator::default());
        let obj_type = ObjectType::class_obj_type("Foo");
        let field_name = field_name!("f");

        let x_graph_id = id_gen.get_id_for_graph();
        let y_graph_id = id_gen.get_id_for_graph();
        let x_node_id = id_gen.get_id_for_node();
        let y_node_id = id_gen.get_id_for_node();

        graphs_map.ensure_graph(x_graph_id);
        graphs_map.ensure_graph(y_graph_id);

        graphs_map.add_object_from_map(
            x_graph_id,
            x_node_id,
            obj_type.clone(),
            [(field_name.clone(), vstr!("x"))],
        );
        graphs_map.add_object_from_map(
            y_graph_id,
            y_node_id,
            obj_type.clone(),
            [(field_name.clone(), vstr!("y"))],
        );

        graphs_map.set_as_root(root_name!("x"), x_graph_id, x_node_id);
        graphs_map.set_as_root(root_name!("y"), y_graph_id, y_node_id);

        let start_ctx = ContextArray::from(vec![Context::with_values(
            [
                (
                    root_name!("x"),
                    vobj!(obj_type.clone(), x_graph_id, x_node_id),
                ),
                (
                    root_name!("y"),
                    vobj!(obj_type.clone(), y_graph_id, y_node_id),
                ),
            ]
            .into(),
            graphs_map.into(),
            id_gen,
        )]);
        let variables = [
            (
                root_name!("x"),
                Variable {
                    name: root_name!("x"),
                    value_type: ValueType::Object(obj_type.clone()),
                    immutable: false,
                },
            ),
            (
                root_name!("y"),
                Variable {
                    name: root_name!("y"),
                    value_type: ValueType::Object(obj_type.clone()),
                    immutable: false,
                },
            ),
        ]
        .into();
        let syn_ctx = SynthesizerContext::from_context_array(start_ctx.clone(), variables);

        let partial_context_builder = PartialContextBuilder::new(&start_ctx);
        let x_ctx = &partial_context_builder
            .get_partial_context(&[root_name!("x")])
            .unwrap()[0];
        let y_ctx = &partial_context_builder
            .get_partial_context(&[root_name!("y")])
            .unwrap()[0];

        let (pre_merged_ctx, post_merged_ctx) = merge_context(x_ctx, x_ctx, y_ctx, y_ctx).unwrap();

        println!("merged pre: {}", pre_merged_ctx);
        println!("merged post: {}", post_merged_ctx);

        assert!(pre_merged_ctx.variable_names().contains(&root_name!("x")));
        assert!(pre_merged_ctx.variable_names().contains(&root_name!("y")));
        assert!(post_merged_ctx.variable_names().contains(&root_name!("x")));
        assert!(post_merged_ctx.variable_names().contains(&root_name!("y")));

        let x = x_ctx
            .get_var_loc_value(&root_name!("x"), syn_ctx.variables())
            .unwrap();
        let y = y_ctx
            .get_var_loc_value(&root_name!("y"), syn_ctx.variables())
            .unwrap();
        let merged_pre_x = pre_merged_ctx
            .get_var_loc_value(&root_name!("x"), syn_ctx.variables())
            .unwrap();
        let merged_pre_y = pre_merged_ctx
            .get_var_loc_value(&root_name!("y"), syn_ctx.variables())
            .unwrap();
        let merged_post_x = post_merged_ctx
            .get_var_loc_value(&root_name!("x"), syn_ctx.variables())
            .unwrap();
        let merged_post_y = post_merged_ctx
            .get_var_loc_value(&root_name!("y"), syn_ctx.variables())
            .unwrap();

        assert_eq!(
            x.wrap(&x_ctx.graphs_map),
            merged_pre_x.wrap(&pre_merged_ctx.graphs_map)
        );
        assert_eq!(
            x.wrap(&x_ctx.graphs_map),
            merged_post_x.wrap(&post_merged_ctx.graphs_map)
        );
        assert_eq!(
            y.wrap(&y_ctx.graphs_map),
            merged_pre_y.wrap(&pre_merged_ctx.graphs_map)
        );
        assert_eq!(
            y.wrap(&y_ctx.graphs_map),
            merged_post_y.wrap(&post_merged_ctx.graphs_map)
        );

        println!("x: {:?}", merged_post_x.val());
        println!("y: {:?}", merged_post_y.val());
    }

    #[test]
    fn complex_embedding_test1() {
        let mut graphs_map = GraphsMap::default();
        let id_gen = Arc::new(GraphIdGenerator::default());
        let obj_type = ObjectType::class_obj_type("Foo");
        let field_name = field_name!("f");
        let obj_type2 = ObjectType::class_obj_type("Boo");
        let field_name2 = field_name!("b");

        let graph_id = id_gen.get_id_for_graph();
        let x_node_id = id_gen.get_id_for_node();
        let y_node_id = id_gen.get_id_for_node();

        graphs_map.ensure_graph(graph_id);

        graphs_map.add_object_from_map(
            graph_id,
            x_node_id,
            obj_type.clone(),
            [(field_name.clone(), vstr!("x"))],
        );
        graphs_map.add_object_from_map(
            graph_id,
            y_node_id,
            obj_type2.clone(),
            [(
                field_name2.clone(),
                vobj!(obj_type2.clone(), graph_id, x_node_id),
            )],
        );

        graphs_map.set_as_root(root_name!("x"), graph_id, x_node_id);
        graphs_map.set_as_root(root_name!("y"), graph_id, y_node_id);

        let start_ctx = ContextArray::from(vec![Context::with_values(
            [
                (
                    root_name!("x"),
                    vobj!(obj_type.clone(), graph_id, x_node_id),
                ),
                (
                    root_name!("y"),
                    vobj!(obj_type2.clone(), graph_id, y_node_id),
                ),
            ]
            .into(),
            graphs_map.into(),
            id_gen,
        )]);
        let variables = [
            (
                root_name!("x"),
                Variable {
                    name: root_name!("x"),
                    value_type: ValueType::Object(obj_type.clone()),
                    immutable: false,
                },
            ),
            (
                root_name!("y"),
                Variable {
                    name: root_name!("y"),
                    value_type: ValueType::Object(obj_type2.clone()),
                    immutable: false,
                },
            ),
        ]
        .into();

        let syn_ctx = SynthesizerContext::from_context_array(start_ctx.clone(), variables);

        let partial_context_builder = PartialContextBuilder::new(&start_ctx);
        let x_ctx = &partial_context_builder
            .get_partial_context(&[root_name!("x")])
            .unwrap()[0];
        let y_ctx = &partial_context_builder
            .get_partial_context(&[root_name!("y")])
            .unwrap()[0];

        println!("x_ctx: {}", x_ctx);
        println!("y_ctx: {}", y_ctx);

        let (pre_merged_ctx, post_merged_ctx) = merge_context(x_ctx, x_ctx, y_ctx, y_ctx).unwrap();

        println!("merged pre: {}", pre_merged_ctx);
        println!("merged post: {}", post_merged_ctx);

        assert!(pre_merged_ctx.variable_names().contains(&root_name!("x")));
        assert!(pre_merged_ctx.variable_names().contains(&root_name!("y")));
        assert!(post_merged_ctx.variable_names().contains(&root_name!("x")));
        assert!(post_merged_ctx.variable_names().contains(&root_name!("y")));

        let x = x_ctx
            .get_var_loc_value(&root_name!("x"), syn_ctx.variables())
            .unwrap();
        let y = y_ctx
            .get_var_loc_value(&root_name!("y"), syn_ctx.variables())
            .unwrap();
        let merged_pre_x = pre_merged_ctx
            .get_var_loc_value(&root_name!("x"), syn_ctx.variables())
            .unwrap();
        let merged_pre_y = pre_merged_ctx
            .get_var_loc_value(&root_name!("y"), syn_ctx.variables())
            .unwrap();
        let merged_post_x = post_merged_ctx
            .get_var_loc_value(&root_name!("x"), syn_ctx.variables())
            .unwrap();
        let merged_post_y = post_merged_ctx
            .get_var_loc_value(&root_name!("y"), syn_ctx.variables())
            .unwrap();

        assert_eq!(
            x.wrap(&x_ctx.graphs_map),
            merged_pre_x.wrap(&pre_merged_ctx.graphs_map)
        );
        assert_eq!(
            x.wrap(&x_ctx.graphs_map),
            merged_post_x.wrap(&post_merged_ctx.graphs_map)
        );
        assert_eq!(
            y.wrap(&y_ctx.graphs_map),
            merged_pre_y.wrap(&pre_merged_ctx.graphs_map)
        );
        assert_eq!(
            y.wrap(&y_ctx.graphs_map),
            merged_post_y.wrap(&post_merged_ctx.graphs_map)
        );

        println!("x: {:?}", merged_post_x.val());
        println!("y: {:?}", merged_post_y.val());
    }
}

#[cfg(test)]
mod prog_tests {
    use std::sync::Arc;

    use crate::{
        context::{Context, ContextArray, Variable},
        dirty,
        prog::SubProgram,
        synthesizer_context::{SynthesizerContext, SynthesizerWorkerContext},
        test::helpers::*,
    };
    use ruse_object_graph::{root_name, vnum, GraphIdGenerator, GraphsMap, Number, ValueType};

    #[test]
    fn modify_post_ctx_test() {
        let graphs_map = GraphsMap::default();
        let id_gen = Arc::new(GraphIdGenerator::default());

        let pre_ctx = ContextArray::from(vec![Context::with_values(
            [(root_name!("x"), vnum!(Number::from(7u64)))].into(),
            graphs_map.into(),
            id_gen,
        )]);
        let post_ctx = pre_ctx.clone();
        let variables = [(
            root_name!("x"),
            Variable {
                name: root_name!("x"),
                value_type: ValueType::Number,
                immutable: false,
            },
        )]
        .into();

        let syn_ctx = SynthesizerContext::from_context_array(pre_ctx.clone(), variables);

        let opcode = Arc::new(UpdateVarOpcode {
            arg_types: vec![],
            id: root_name!("x"),
            new_value: vnum!(Number::from(5)),
            returns: dirty!(post_ctx[0].temp_value(vnum!(Number::from(0)))),
        });

        let mut p = SubProgram::with_opcode(opcode, pre_ctx, post_ctx);
        let mut worker_ctx = SynthesizerWorkerContext::default();
        evaluate_prog(&mut p, &syn_ctx, &mut worker_ctx);
        println!("{}", p.pre_ctx());
        println!("{}", p.out_value().wrap(p.post_ctx()));
        println!("{}", p.post_ctx());
    }
}
