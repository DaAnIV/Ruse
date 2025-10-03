#[cfg(feature = "test_helpers")]
#[allow(dead_code)]
pub mod ts_op_helpers {
    use std::sync::Arc;

    use ruse_object_graph::{root_name, str_cached, Number};

    use crate::{opcode, ts_class::MethodKind, ts_user_class::TsUserClass};

    pub fn id_op(id: &str) -> Arc<opcode::IdentOp> {
        Arc::new(opcode::IdentOp::new(root_name!(id)))
    }

    pub fn class_method_op(class: &TsUserClass, method_name: &str) -> Arc<opcode::ClassMethodOp> {
        Arc::new(opcode::ClassMethodOp::new(
            class.description.class_name.clone(),
            &class.description.methods[&(method_name.to_string(), MethodKind::Method)],
            class.description.methods[&(method_name.to_string(), MethodKind::Method)].param_types
                [0]
            .clone(),
        ))
    }

    pub fn class_getter_op(class: &TsUserClass, method_name: &str) -> Arc<opcode::ClassMethodOp> {
        Arc::new(opcode::ClassMethodOp::new(
            class.description.class_name.clone(),
            &class.description.methods[&(method_name.to_string(), MethodKind::Getter)],
            class.description.methods[&(method_name.to_string(), MethodKind::Getter)].param_types
                [0]
            .clone(),
        ))
    }

    pub fn class_setter_op(class: &TsUserClass, method_name: &str) -> Arc<opcode::ClassMethodOp> {
        Arc::new(opcode::ClassMethodOp::new(
            class.description.class_name.clone(),
            &class.description.methods[&(method_name.to_string(), MethodKind::Setter)],
            class.description.methods[&(method_name.to_string(), MethodKind::Setter)].param_types
                [0]
            .clone(),
        ))
    }

    pub fn lit_number_op(val: usize) -> Arc<opcode::LitOp> {
        Arc::new(opcode::LitOp::Num(Number::from(val)))
    }

    pub fn lit_str_op(val: &str) -> Arc<opcode::LitOp> {
        Arc::new(opcode::LitOp::Str(str_cached!(val)))
    }
}

#[cfg(test)]
mod ts_simple_opcodes_tests {
    use std::sync::Arc;

    use context::{Context, ContextArray};
    use graph_map_value::GraphMapWrap;
    use ruse_object_graph::location::Location;
    use ruse_object_graph::*;
    use ruse_synthesizer::context::{Variable, VariableMap};
    use ruse_synthesizer::opcode::ExprOpcode;
    use ruse_synthesizer::test::helpers::generate_context_from_array;
    use swc_ecma_ast as ast;
    use synthesizer_context::SynthesizerContext;

    use crate::js_worker_context::create_js_worker_context;
    use crate::opcode::*;
    use crate::test::ts_op_helpers::*;
    use ruse_object_graph::Number;
    use ruse_synthesizer::*;

    #[test]
    fn add_numbers() {
        let graphs_map = GraphsMap::default();
        let ctx_arr = ContextArray::default();
        let syn_ctx = SynthesizerContext::from_context_array(ctx_arr, VariableMap::default());
        let mut worker_ctx = create_js_worker_context(0);

        let ctx = &syn_ctx.start_context[0].clone();
        let mut out_ctx = ctx.clone();
        let evaluator = BinOp::new(ast::BinaryOp::Add, ValueType::Number, ValueType::Number);

        let args = [
            &ctx.temp_value(vnum!(Number::from(3u64))),
            &ctx.temp_value(vnum!(Number::from(4u64))),
        ];
        let out = evaluator
            .eval(&args, &mut out_ctx, &syn_ctx, &mut worker_ctx)
            .unwrap();
        assert_eq!(
            out.val().wrap(&graphs_map),
            vnum!(Number::from(7u64)).wrap(&graphs_map)
        );
    }

    #[test]
    fn add_strings() {
        let graphs_map = GraphsMap::default();
        let ctx_arr = ContextArray::default();
        let syn_ctx = SynthesizerContext::from_context_array(ctx_arr, VariableMap::default());
        let mut worker_ctx = create_js_worker_context(0);

        let ctx = &syn_ctx.start_context[0];
        let mut out_ctx = ctx.clone();
        let evaluator = BinOp::new(ast::BinaryOp::Add, ValueType::String, ValueType::String);

        let args = [&ctx.temp_value(vstr!("a")), &ctx.temp_value(vstr!("b"))];

        let out = evaluator
            .eval(&args, &mut out_ctx, &syn_ctx, &mut worker_ctx)
            .unwrap();
        assert_eq!(out.val().wrap(&graphs_map), vstr!("ab").wrap(&graphs_map));
    }

    #[test]
    fn ident() {
        let graphs_map = GraphsMap::default();
        let id_gen = Arc::new(GraphIdGenerator::default());
        let ctx_arr = ContextArray::from(vec![Context::with_values(
            [(root_name!("x"), vnum!(Number::from(7u64)))].into(),
            graphs_map.into(),
            id_gen,
        )]);
        let variables = [(
            root_name!("x"),
            Variable {
                name: root_name!("x"),
                value_type: ValueType::Number,
                immutable: false,
            },
        )]
        .into();
        let syn_ctx = SynthesizerContext::from_context_array(ctx_arr, variables);
        let mut worker_ctx = create_js_worker_context(0);

        let ctx = &syn_ctx.start_context[0];
        let mut out_ctx = ctx.clone();
        let evaluator = id_op("x");
        let out = evaluator
            .eval(&[], &mut out_ctx, &syn_ctx, &mut worker_ctx)
            .unwrap();
        assert_eq!(
            out.val().wrap(&out_ctx.graphs_map),
            vnum!(Number::from(7u64)).wrap(&out_ctx.graphs_map)
        );
    }

    #[test]
    fn prefix_plus_plus() {
        let graphs_map = GraphsMap::default();
        let id_gen = Arc::new(GraphIdGenerator::default());
        let ctx_arr = ContextArray::from(vec![Context::with_values(
            [(root_name!("x"), vnum!(Number::from(7u64)))].into(),
            graphs_map.into(),
            id_gen,
        )]);
        let variables = [(
            root_name!("x"),
            Variable {
                name: root_name!("x"),
                value_type: ValueType::Number,
                immutable: false,
            },
        )]
        .into();
        let syn_ctx = SynthesizerContext::from_context_array(ctx_arr, variables);
        let mut worker_ctx = create_js_worker_context(0);

        let ctx = &syn_ctx.start_context[0];
        let id = id_op("x");
        let op = UpdateOp::new(ast::UpdateOp::PlusPlus, true);
        let mut id_out_ctx = ctx.clone();
        let x_val = id
            .eval(&[], &mut id_out_ctx, &syn_ctx, &mut worker_ctx)
            .unwrap();

        let mut update_out_ctx = id_out_ctx.clone();
        let out = op
            .eval(
                &[&x_val.output],
                &mut update_out_ctx,
                &syn_ctx,
                &mut worker_ctx,
            )
            .unwrap();
        assert_eq!(
            ctx.get_var_loc_value(&id.name, syn_ctx.variables())
                .expect("Didn't find var")
                .val()
                .wrap(&ctx.graphs_map),
            vnum!(Number::from(7u64)).wrap(&ctx.graphs_map)
        );
        assert_eq!(
            x_val.val().wrap(&ctx.graphs_map),
            vnum!(Number::from(7u64)).wrap(&ctx.graphs_map)
        );
        assert!(matches!(
            x_val.loc(),
            Location::Root(x) if x.root == id.name.clone()));
        assert_eq!(
            out.val().wrap(&update_out_ctx.graphs_map),
            vnum!(Number::from(8u64)).wrap(&update_out_ctx.graphs_map)
        );
        assert!(matches!(out.loc(), Location::Temp));
        assert_eq!(
            update_out_ctx
                .get_var_loc_value(&id.name, syn_ctx.variables())
                .expect("Didn't find var")
                .val()
                .wrap(&update_out_ctx.graphs_map),
            vnum!(Number::from(8u64)).wrap(&update_out_ctx.graphs_map)
        );
    }

    #[test]
    fn postfix_plus_plus() {
        let graphs_map = GraphsMap::default();
        let id_gen = Arc::new(GraphIdGenerator::default());
        let ctx_arr = ContextArray::from(vec![Context::with_values(
            [(root_name!("x"), vnum!(Number::from(7u64)))].into(),
            graphs_map.into(),
            id_gen,
        )]);
        let variables = [(
            root_name!("x"),
            Variable {
                name: root_name!("x"),
                value_type: ValueType::Number,
                immutable: false,
            },
        )]
        .into();
        let syn_ctx = SynthesizerContext::from_context_array(ctx_arr, variables);
        let mut worker_ctx = create_js_worker_context(0);

        let ctx = &syn_ctx.start_context[0];
        let id = id_op("x");
        let op = UpdateOp::new(ast::UpdateOp::PlusPlus, false);
        let mut id_out_ctx = ctx.clone();
        let x_val = id
            .eval(&[], &mut id_out_ctx, &syn_ctx, &mut worker_ctx)
            .unwrap();

        let mut update_out_ctx = id_out_ctx.clone();
        let out = op
            .eval(&[&x_val], &mut update_out_ctx, &syn_ctx, &mut worker_ctx)
            .unwrap();
        assert_eq!(
            ctx.get_var_loc_value(&id.name, syn_ctx.variables())
                .expect("Didn't find var")
                .val()
                .wrap(&ctx.graphs_map),
            vnum!(Number::from(7u64)).wrap(&ctx.graphs_map)
        );
        assert_eq!(
            x_val.val().wrap(&ctx.graphs_map),
            vnum!(Number::from(7u64)).wrap(&ctx.graphs_map)
        );
        assert!(matches!(
            x_val.loc(),
            Location::Root(x) if x.root == id.name.clone()));
        assert_eq!(
            out.val().wrap(&update_out_ctx.graphs_map),
            vnum!(Number::from(7u64)).wrap(&update_out_ctx.graphs_map)
        );
        assert!(matches!(out.loc(), Location::Temp));
        assert_eq!(
            update_out_ctx
                .get_var_loc_value(&id.name, syn_ctx.variables())
                .expect("Didn't find var")
                .val()
                .wrap(&update_out_ctx.graphs_map),
            vnum!(Number::from(8u64)).wrap(&update_out_ctx.graphs_map)
        );
    }

    #[test]
    fn array_push() {
        let ctx = generate_context_from_array(root_name!("x"), &ValueType::Number, []);
        let ctx_arr = ContextArray::from(vec![ctx]);
        let variables = [(
            root_name!("x"),
            Variable {
                name: root_name!("x"),
                value_type: ValueType::Object(ObjectType::Array(Box::new(ValueType::Number))),
                immutable: false,
            },
        )]
        .into();
        let syn_ctx = SynthesizerContext::from_context_array(ctx_arr, variables);
        let mut worker_ctx = create_js_worker_context(0);

        let ctx = &syn_ctx.start_context[0];
        let id = id_op("x");
        let op = ArrayPushOp::new(&ValueType::Number);

        let mut id_out_ctx = ctx.clone();
        let x_val = id
            .eval(&[], &mut id_out_ctx, &syn_ctx, &mut worker_ctx)
            .unwrap();

        let mut update_out_ctx = id_out_ctx.clone();
        let num_to_push = update_out_ctx.temp_value(vnum!(Number::from(1)));
        let out = op
            .eval(
                &[&x_val.output, &num_to_push],
                &mut update_out_ctx,
                &syn_ctx,
                &mut worker_ctx,
            )
            .unwrap();

        let orig_array = ctx
            .get_var_loc_value(&id.name, syn_ctx.variables())
            .expect("Didn't find var")
            .val()
            .obj()
            .unwrap()
            .clone();
        let updated_array = update_out_ctx
            .get_var_loc_value(&id.name, syn_ctx.variables())
            .expect("Didn't find var")
            .val()
            .obj()
            .unwrap()
            .clone();

        assert_eq!(orig_array.total_field_count(&ctx.graphs_map), 0);
        assert_eq!(
            updated_array.total_field_count(&update_out_ctx.graphs_map),
            1
        );
        assert_eq!(
            updated_array
                .get_primitive_field_value(&field_name!("0"), &update_out_ctx.graphs_map)
                .unwrap()
                .wrap(&update_out_ctx.graphs_map),
            vnum!(Number::from(1)).wrap(&update_out_ctx.graphs_map)
        );

        assert!(matches!(out.loc(), Location::Temp));
        assert_eq!(
            out.val().wrap(&update_out_ctx.graphs_map),
            vnum!(Number::from(1u64)).wrap(&update_out_ctx.graphs_map)
        );
    }

    #[test]
    fn test_array_reverse() {
        let ctx = generate_context_from_array(
            root_name!("x"),
            &ValueType::Number,
            [
                vnum!(Number::from(1)),
                vnum!(Number::from(5)),
                vnum!(Number::from(3)),
            ],
        );
        let ctx_arr = ContextArray::from(vec![ctx]);
        let variables = [(
            root_name!("x"),
            Variable {
                name: root_name!("x"),
                value_type: ValueType::Object(ObjectType::Array(Box::new(ValueType::Number))),
                immutable: false,
            },
        )]
        .into();
        let syn_ctx = SynthesizerContext::from_context_array(ctx_arr, variables);
        let mut worker_ctx = create_js_worker_context(0);

        let ctx = &syn_ctx.start_context[0];
        let id = id_op("x");
        let op = ArrayReverseOp::new(&ValueType::Number);

        let mut id_out_ctx = ctx.clone();
        let x_val = id
            .eval(&[], &mut id_out_ctx, &syn_ctx, &mut worker_ctx)
            .unwrap();

        let mut update_out_ctx = id_out_ctx.clone();
        let num_to_push = update_out_ctx.temp_value(vnum!(Number::from(1)));
        let out = op
            .eval(
                &[&x_val.output, &num_to_push],
                &mut update_out_ctx,
                &syn_ctx,
                &mut worker_ctx,
            )
            .unwrap();

        let orig_array = ctx
            .get_var_loc_value(&id.name, syn_ctx.variables())
            .expect("Didn't find var")
            .val()
            .obj()
            .unwrap()
            .clone();
        let updated_array = update_out_ctx
            .get_var_loc_value(&id.name, syn_ctx.variables())
            .expect("Didn't find var")
            .val()
            .obj()
            .unwrap()
            .clone();

        assert_eq!(out.val().obj().unwrap().graph_id, updated_array.graph_id);
        assert_eq!(out.val().obj().unwrap().node, updated_array.node);
        assert_eq!(out.val().obj().unwrap().obj_type, updated_array.obj_type);

        let total_field_count = orig_array.total_field_count(&ctx.graphs_map);
        assert_eq!(
            orig_array.total_field_count(&ctx.graphs_map),
            updated_array.total_field_count(&update_out_ctx.graphs_map)
        );

        for i in 0..total_field_count {
            assert_eq!(
                updated_array
                    .get_primitive_field_value(
                        &field_name!(i.to_string()),
                        &update_out_ctx.graphs_map
                    )
                    .unwrap()
                    .number_value()
                    .unwrap(),
                orig_array
                    .get_primitive_field_value(
                        &field_name!((total_field_count - i - 1).to_string()),
                        &ctx.graphs_map
                    )
                    .unwrap()
                    .number_value()
                    .unwrap()
            );
        }
    }

    #[test]
    fn test_array_sort() {
        let values = vec![
            vnum!(Number::from(1)),
            vnum!(Number::from(5)),
            vnum!(Number::from(3)),
        ];
        let sorted_values = vec![
            vnum!(Number::from(1)),
            vnum!(Number::from(3)),
            vnum!(Number::from(5)),
        ];

        let ctx = generate_context_from_array(root_name!("x"), &ValueType::Number, values);
        let ctx_arr = ContextArray::from(vec![ctx]);
        let variables = [(
            root_name!("x"),
            Variable {
                name: root_name!("x"),
                value_type: ValueType::Object(ObjectType::Array(Box::new(ValueType::Number))),
                immutable: false,
            },
        )]
        .into();
        let syn_ctx = SynthesizerContext::from_context_array(ctx_arr, variables);
        let mut worker_ctx = create_js_worker_context(0);

        let ctx = &syn_ctx.start_context[0];
        let id = id_op("x");
        let op = ArraySortOp::new(&ValueType::Number);

        let mut id_out_ctx = ctx.clone();
        let x_val = id
            .eval(&[], &mut id_out_ctx, &syn_ctx, &mut worker_ctx)
            .unwrap();

        let mut update_out_ctx = id_out_ctx.clone();
        let num_to_push = update_out_ctx.temp_value(vnum!(Number::from(1)));
        let out = op
            .eval(
                &[&x_val.output, &num_to_push],
                &mut update_out_ctx,
                &syn_ctx,
                &mut worker_ctx,
            )
            .unwrap();

        let orig_array = ctx
            .get_var_loc_value(&id.name, syn_ctx.variables())
            .expect("Didn't find var")
            .val()
            .obj()
            .unwrap()
            .clone();
        let updated_array = update_out_ctx
            .get_var_loc_value(&id.name, syn_ctx.variables())
            .expect("Didn't find var")
            .val()
            .obj()
            .unwrap()
            .clone();

        assert_eq!(out.val().obj().unwrap().graph_id, updated_array.graph_id);
        assert_eq!(out.val().obj().unwrap().node, updated_array.node);
        assert_eq!(out.val().obj().unwrap().obj_type, updated_array.obj_type);

        let total_field_count = orig_array.total_field_count(&ctx.graphs_map);
        assert_eq!(
            orig_array.total_field_count(&ctx.graphs_map),
            updated_array.total_field_count(&update_out_ctx.graphs_map)
        );

        for i in 0..total_field_count {
            assert_eq!(
                updated_array
                    .get_primitive_field_value(
                        &field_name!(i.to_string()),
                        &update_out_ctx.graphs_map
                    )
                    .unwrap()
                    .number_value()
                    .unwrap(),
                sorted_values[i].number_value().unwrap()
            );
        }
    }
}

#[cfg(test)]
mod ts_class_tests {
    use std::{collections::HashMap, sync::Arc};

    use boa_engine::JsValue;
    use boa_engine::{js_string, property::Attribute};
    use graph_map_value::GraphMapWrap;
    use ruse_object_graph::{value::*, *};
    use ruse_synthesizer::context::{Context, ContextArray, ValuesMap, Variable};
    use ruse_synthesizer::synthesizer_context::SynthesizerContext;
    use ruse_synthesizer::test::helpers::{get_composite_prog, get_init_prog};

    use crate::engine_context::EngineContext;
    use crate::js_value::{TryFromJs, TryIntoJs};
    use crate::js_worker_context::create_js_worker_context;
    use crate::test::ts_op_helpers::*;
    use crate::{
        ts_class::TsClass,
        ts_classes::{TsClasses, TsClassesBuilder},
    };

    #[test]
    fn generate_object() {
        let code = "export class User {
            constructor(public name: string, 
                        public surname: string) {}
        }";
        let mut graphs_map = GraphsMap::default();
        let id_gen = Arc::new(GraphIdGenerator::default());
        let mut builder = TsClassesBuilder::new();

        builder.add_classes(code).expect("Failed to add User class");
        let classes = builder.finalize().unwrap();

        let user_class_name = class_name!("User");
        let graph_id = id_gen.get_id_for_graph();
        graphs_map.ensure_graph(graph_id);
        let user = classes
            .get_user_class(&user_class_name)
            .unwrap()
            .generate_object(
                HashMap::from([
                    (field_name!("surname"), vstr!("Doe")),
                    (field_name!("name"), vstr!("John")),
                ]),
                &mut graphs_map,
                graph_id,
                &id_gen,
            );

        let name_field = user
            .get_field_value(&field_name!("name"), &graphs_map)
            .unwrap();
        assert_eq!(
            name_field.wrap(&graphs_map),
            vstr!("John").wrap(&graphs_map)
        )
    }

    #[test]
    fn member_opcodes() {
        let code = "export class User {
            constructor(public name: string, 
                        public surname: string) {}
        }";
        let mut builder = TsClassesBuilder::new();

        builder.add_classes(code).expect("Failed to add User class");
        let user_class_name = class_name!("User");

        let classes = builder.finalize().unwrap();

        let user_class = classes.get_user_class(&user_class_name).unwrap();
        let opcodes = &user_class.member_opcodes;
        assert_eq!(opcodes.len(), 2);
        assert!(opcodes.iter().all(|op| {
            op.arg_types().len() == 1
                && op.arg_types()[0] == ValueType::class_value_type(user_class_name.clone())
        }));
        // Need to check the opcodes are correct?
    }

    #[test]
    fn object_fields() {
        let code1 = "export class Student {
            constructor(public name: string, 
                        public surname: string,
                        public age: number,
                        public grades: number[]) {}
        }";
        let code2 = "export class Class {
            constructor(public students: Student[]) {}
        }";

        let mut builder = TsClassesBuilder::new();

        builder
            .add_classes(code1)
            .expect("Failed to add Student class");
        builder
            .add_classes(code2)
            .expect("Failed to add Class class");

        let classes = builder.finalize().unwrap();

        let student_class = classes.get_user_class(&class_name!("Student")).unwrap();
        let class_class = classes.get_user_class(&class_name!("Class")).unwrap();

        assert!(student_class.description.fields.get("name").is_some());
        assert!(student_class.description.fields.get("surname").is_some());
        assert!(student_class.description.fields.get("age").is_some());
        assert!(student_class.description.fields.get("grades").is_some());
        assert!(class_class.description.fields.get("students").is_some());

        assert_eq!(
            student_class.description.fields["name"].value_type,
            Some(ValueType::String)
        );
        assert_eq!(
            student_class.description.fields["surname"].value_type,
            Some(ValueType::String)
        );
        assert_eq!(
            student_class.description.fields["age"].value_type,
            Some(ValueType::Number)
        );
        assert_eq!(
            student_class.description.fields["grades"].value_type,
            Some(ValueType::array_value_type(&ValueType::Number))
        );
        assert_eq!(
            class_class.description.fields["students"].value_type,
            Some(ValueType::array_value_type(&student_class.value_type(None)))
        );

        assert!(class_class
            .description
            .fields
            .values()
            .all(|field| !field.is_private && !field.is_static && !field.is_readonly))
    }

    #[test]
    fn simple_js_object_eval() {
        let code = "export class User {
            constructor(public name: string, 
                        public surname: string) {}
        }";

        let mut graphs_map = GraphsMap::default();
        let id_gen = Arc::new(GraphIdGenerator::default());
        let mut builder = TsClassesBuilder::new();

        builder.add_classes(code).expect("Failed to add User class");
        let user_class_name = class_name!("User");

        let classes = builder.finalize().unwrap();

        let user_class = classes.get_user_class(&user_class_name).unwrap();
        let graph_id = id_gen.get_id_for_graph();
        graphs_map.ensure_graph(graph_id);
        let user = user_class.generate_object(
            HashMap::from([
                (field_name!("surname"), vstr!("Doe")),
                (field_name!("name"), vstr!("John")),
            ]),
            &mut graphs_map,
            graph_id,
            &id_gen,
        );

        let mut ctx = Context::with_values([].into(), graphs_map.into(), id_gen);
        let mut engine_context = EngineContext::create_engine_ctx(&classes);
        engine_context.reset_with_mut_context(&mut ctx, &classes);

        let js_user = user_class
            .wrap_as_js_object(user, &mut engine_context)
            .unwrap();
        engine_context
            .register_global_property(js_string!("u"), js_user, Attribute::all())
            .expect("Failed to register p");

        let js_code = boa_engine::Source::from_bytes("u.name + \" \" + u.surname");
        let res = engine_context.eval(js_code).unwrap();
        assert!(res.is_string());
        assert_eq!(res.as_string().unwrap(), &js_string!("John Doe"));
    }

    #[test]
    fn complex_js_object_eval() {
        let code1 = "export class User {
            constructor(public name: string, 
                        public surname: string,
                        public age: number,
                        protected is_admin: bool,
                        public grades: number[]) {}
        }";
        let code2 = "export class UserPair {
            constructor(public user1: User, 
                        public user2: User) {}
        }";

        let mut graphs_map = GraphsMap::default();
        let id_gen = Arc::new(GraphIdGenerator::default());
        let mut builder = TsClassesBuilder::new();

        builder
            .add_classes(code1)
            .expect("Failed to add User class");
        builder
            .add_classes(code2)
            .expect("Failed to add UserPair class");

        let classes = builder.finalize().unwrap();

        let user_class = classes.get_user_class(&class_name!("User")).unwrap();
        let user_class_pair = classes.get_user_class(&class_name!("UserPair")).unwrap();

        let user1_graph_id = id_gen.get_id_for_graph();
        graphs_map.ensure_graph(user1_graph_id);
        let user1 = user_class.generate_object(
            HashMap::from([
                (field_name!("surname"), vstr!("Doe")),
                (field_name!("name"), vstr!("John")),
            ]),
            &mut graphs_map,
            user1_graph_id,
            &id_gen,
        );
        graphs_map.set_as_root(root_name!("student1"), user1.graph_id, user1.node);

        let user2_graph_id = id_gen.get_id_for_graph();
        graphs_map.ensure_graph(user2_graph_id);
        let user2 = user_class.generate_object(
            HashMap::from([
                (field_name!("name"), vstr!("Paul")),
                (field_name!("surname"), vstr!("Simon")),
            ]),
            &mut graphs_map,
            user1_graph_id,
            &id_gen,
        );
        graphs_map.set_as_root(root_name!("student2"), user2.graph_id, user2.node);

        let complex_user_graph_id = id_gen.get_id_for_graph();
        graphs_map.ensure_graph(complex_user_graph_id);
        let complex_user = user_class_pair.generate_object(
            HashMap::from([
                (field_name!("user1"), Value::Object(user1)),
                (field_name!("user2"), Value::Object(user2)),
            ]),
            &mut graphs_map,
            complex_user_graph_id,
            &id_gen,
        );
        graphs_map.set_as_root(
            root_name!("student_pair"),
            complex_user.graph_id,
            complex_user.node,
        );

        let mut ctx = Context::with_values([].into(), graphs_map.into(), id_gen);
        let mut engine_context = EngineContext::create_engine_ctx(&classes);
        engine_context.reset_with_mut_context(&mut ctx, &classes);

        let js_obj = user_class_pair
            .wrap_as_js_object(complex_user, &mut engine_context)
            .unwrap();
        engine_context
            .register_global_property(js_string!("up"), js_obj, Attribute::all())
            .expect("Failed to register p");

        let js_code = boa_engine::Source::from_bytes("up.user1.name + \" \" + up.user2.name");
        let res = engine_context.eval(js_code).unwrap();
        assert!(res.is_string());
        assert_eq!(res.as_string().unwrap(), &js_string!("John Paul"));
    }

    #[test]
    fn js_object_eval_set() {
        let code = "export class User {
            constructor(public name: string, 
                        public surname: string) {}

            
        }";

        let mut graphs_map = GraphsMap::default();
        let id_gen = Arc::new(GraphIdGenerator::default());
        let mut builder = TsClassesBuilder::new();

        builder.add_classes(code).expect("Failed to add User class");
        let user_class_name = class_name!("User");

        let classes = builder.finalize().unwrap();

        let graph_id = id_gen.get_id_for_graph();
        graphs_map.ensure_graph(graph_id);
        let user = {
            let user_class = classes.get_user_class(&user_class_name).unwrap();
            user_class.generate_object(
                HashMap::from([
                    (field_name!("surname"), vstr!("Doe")),
                    (field_name!("name"), vstr!("John")),
                ]),
                &mut graphs_map,
                graph_id,
                &id_gen,
            )
        };
        graphs_map.set_as_root(root_name!("u"), user.graph_id, user.node);

        let mut values = ValuesMap::default();
        values.insert(root_name!("u"), Value::Object(user.clone()));

        let mut ctx = Context::with_values(values, graphs_map.into(), id_gen);
        let variables = [(
            root_name!("u"),
            Variable {
                name: root_name!("u"),
                value_type: ValueType::class_value_type(user_class_name.clone()),
                immutable: false,
            },
        )]
        .into();
        let syn_ctx = SynthesizerContext::from_context_array_with_data(
            ContextArray::from(vec![ctx.clone()]),
            variables,
            classes,
        );
        let classes_ref = syn_ctx.data.downcast_ref::<TsClasses>().unwrap();
        let user_class = classes_ref.get_user_class(&user_class_name).unwrap();

        let mut engine_ctx = EngineContext::create_engine_ctx(classes_ref);
        engine_ctx.reset_with_mut_context(&mut ctx, classes_ref);

        ctx.get_var_loc_value(&root_name!("u"), syn_ctx.variables())
            .unwrap()
            .val()
            .try_into_js(&mut engine_ctx)
            .unwrap();

        let js_user = user_class
            .wrap_as_js_object(user.clone(), &mut engine_ctx)
            .unwrap();
        engine_ctx
            .register_global_property(js_string!("u"), js_user, Attribute::all())
            .expect("Failed to register p");

        let js_code = boa_engine::Source::from_bytes("u.name = \"abc\"");
        let _res = engine_ctx.eval(js_code).unwrap();

        let user_after = ctx
            .get_var_loc_value(&root_name!("u"), syn_ctx.variables())
            .unwrap();
        let user_name_after = user_after
            .val()
            .obj()
            .unwrap()
            .get_field_value(&field_name!("name"), &ctx.graphs_map);
        assert_eq!(
            user_name_after
                .unwrap()
                .primitive()
                .unwrap()
                .string()
                .unwrap()
                .as_str(),
            "abc"
        );
    }

    #[test]
    fn js_object_method_opcode() {
        let code = "export class User {
            constructor(public name: string, 
                        public surname: string) {}

            test(x: string) {
                this.name = \"Name \" + x;
                this.surname = \"Surname \" + x;
                return 0;
            }
        }";

        let mut graphs_map = GraphsMap::default();
        let id_gen = Arc::new(GraphIdGenerator::default());
        let mut builder = TsClassesBuilder::new();

        builder.add_classes(code).expect("Failed to add User class");
        let user_class_name = class_name!("User");

        let classes = builder.finalize().unwrap();

        {
            let user_class = classes.get_user_class(&user_class_name).unwrap();

            assert_eq!(user_class.method_opcodes.len(), 1);
            println!("{}", user_class.method_opcodes[0].op_name());
            for arg in user_class.method_opcodes[0].arg_types() {
                print!("{}, ", arg);
            }
            println!("");
        }

        let graph_id = id_gen.get_id_for_graph();
        graphs_map.ensure_graph(graph_id);
        let user = classes
            .get_user_class(&user_class_name)
            .unwrap()
            .generate_object(
                HashMap::from([
                    (field_name!("surname"), vstr!("Doe")),
                    (field_name!("name"), vstr!("John")),
                ]),
                &mut graphs_map,
                graph_id,
                &id_gen,
            );
        graphs_map.set_as_root(root_name!("user"), user.graph_id, user.node);

        let mut values = ValuesMap::default();
        values.insert(root_name!("user"), Value::Object(user));

        let ctx = Context::with_values(values, graphs_map.into(), id_gen);
        let ctx_arr = ContextArray::from(vec![ctx]);
        let variables = [(
            root_name!("user"),
            Variable {
                name: root_name!("user"),
                value_type: ValueType::class_value_type(user_class_name.clone()),
                immutable: false,
            },
        )]
        .into();
        let syn_ctx =
            SynthesizerContext::from_context_array_with_data(ctx_arr.clone(), variables, classes);
        let mut worker_ctx = create_js_worker_context(0);

        let classes_ref = syn_ctx.data.downcast_ref::<TsClasses>().unwrap();
        let user_class = classes_ref.get_user_class(&user_class_name).unwrap();

        let user_op = id_op("user");
        let user_prog = get_init_prog(user_op, &ctx_arr, &syn_ctx, &mut worker_ctx);
        println!("{}\n", user_prog);

        let str_lit_op = lit_str_op("Lit");
        let str_lit_prog = get_init_prog(str_lit_op, &ctx_arr, &syn_ctx, &mut worker_ctx);
        println!("{}\n", str_lit_prog);

        let test_op = user_class.method_opcodes[0].clone();
        let test_prog = get_composite_prog(
            test_op,
            vec![user_prog.clone(), str_lit_prog.clone()],
            &syn_ctx,
            &mut worker_ctx,
        );
        println!("{}\n", test_prog);
        let user_after = test_prog.post_ctx()[0]
            .get_var_loc_value(&root_name!("user"), syn_ctx.variables())
            .unwrap();
        let user_name_after = user_after
            .val()
            .obj()
            .unwrap()
            .get_field_value(&field_name!("name"), &test_prog.post_ctx()[0].graphs_map);
        assert_eq!(
            user_name_after
                .unwrap()
                .primitive()
                .unwrap()
                .string()
                .unwrap()
                .as_str(),
            "Name Lit"
        );
        let user_surname_after = user_after
            .val()
            .obj()
            .unwrap()
            .get_field_value(&field_name!("surname"), &test_prog.post_ctx()[0].graphs_map);
        assert_eq!(
            user_surname_after
                .unwrap()
                .primitive()
                .unwrap()
                .string()
                .unwrap()
                .as_str(),
            "Surname Lit"
        );
    }

    #[test]
    fn call_constructor() {
        let code = "export class User {
            public fullname: string;

            constructor(public name: string, 
                        public surname: string) {
                this.fullname = name + surname;            
            }
        }";

        let mut graphs_map = GraphsMap::default();
        let id_gen = Arc::new(GraphIdGenerator::default());
        let mut builder = TsClassesBuilder::new();

        builder.add_classes(code).expect("Failed to add User class");
        let user_class_name = class_name!("User");

        let classes = builder.finalize().unwrap();

        let graph_id = id_gen.get_id_for_graph();
        graphs_map.ensure_graph(graph_id);

        let mut engine_ctx = EngineContext::create_engine_ctx(&classes);
        engine_ctx.reset_with_graph(graph_id, &mut graphs_map, &classes, &id_gen);

        let user_class = classes.get_user_class(&user_class_name).unwrap();
        let js_user = user_class
            .call_constructor(&[vstr!("a"), vstr!("b")], &mut engine_ctx)
            .unwrap();
        let name_field = js_user.get_field_value(&field_name!("name"), &graphs_map);
        let surname_field = js_user.get_field_value(&field_name!("surname"), &graphs_map);
        let fullname_field = js_user.get_field_value(&field_name!("fullname"), &graphs_map);

        print!("{}", js_user.wrap(&graphs_map));

        assert!(name_field.is_some());
        assert_eq!(
            name_field.unwrap().wrap(&graphs_map),
            vstr!("a").wrap(&graphs_map)
        );
        assert!(surname_field.is_some());
        assert_eq!(
            surname_field.unwrap().wrap(&graphs_map),
            vstr!("b").wrap(&graphs_map)
        );
        assert!(fullname_field.is_some());
        assert_eq!(
            fullname_field.unwrap().wrap(&graphs_map),
            vstr!("ab").wrap(&graphs_map)
        );
    }

    #[test]
    fn eval_new() {
        let code = "export class User {
            public fullname: string;

            constructor(public name: string, 
                        public surname: string) {
                this.fullname = name + surname;            
            }
        }";

        let mut graphs_map = GraphsMap::default();
        let id_gen = Arc::new(GraphIdGenerator::default());
        let mut builder = TsClassesBuilder::new();

        builder.add_classes(code).expect("Failed to add User class");

        let classes = builder.finalize().unwrap();

        let graph_id = id_gen.get_id_for_graph();
        graphs_map.ensure_graph(graph_id);
        let mut engine_ctx = EngineContext::create_engine_ctx(&classes);
        engine_ctx.reset_with_graph(graph_id, &mut graphs_map, &classes, &id_gen);

        let res = engine_ctx
            .eval(boa_engine::Source::from_bytes("new User(\"a\", \"b\")"))
            .unwrap();
        let js_user = ObjectValue::try_from_js(&res, &mut engine_ctx).unwrap();

        let name_field = js_user.get_field_value(&field_name!("name"), &graphs_map);
        let surname_field = js_user.get_field_value(&field_name!("surname"), &graphs_map);
        let fullname_field = js_user.get_field_value(&field_name!("fullname"), &graphs_map);

        print!("{}", js_user.wrap(&graphs_map));

        assert!(name_field.is_some());
        assert_eq!(
            name_field.unwrap().wrap(&graphs_map),
            vstr!("a").wrap(&graphs_map)
        );
        assert!(surname_field.is_some());
        assert_eq!(
            surname_field.unwrap().wrap(&graphs_map),
            vstr!("b").wrap(&graphs_map)
        );
        assert!(fullname_field.is_some());
        assert_eq!(
            fullname_field.unwrap().wrap(&graphs_map),
            vstr!("ab").wrap(&graphs_map)
        );
    }

    #[test]
    fn eval_func() {
        let mut boa_ctx = EngineContext::new_boa_ctx();
        let res = boa_ctx
            .eval(boa_engine::Source::from_bytes(
                "function func(a, b, c) { return a + b + c; }\nfunc",
            ))
            .unwrap();
        let a = res.as_callable().unwrap();
        let func_res = a
            .call(
                &boa_engine::JsValue::null(),
                &[JsValue::new(1), JsValue::new(2), JsValue::new(3)],
                &mut boa_ctx,
            )
            .unwrap();
        let func_res_number = func_res.to_i32(&mut boa_ctx).unwrap();
        assert_eq!(func_res_number, 1 + 2 + 3);
    }

    #[test]
    fn eval_max() {
        let mut boa_ctx = EngineContext::new_boa_ctx();
        let res = boa_ctx
            .eval(boa_engine::Source::from_bytes(
                "function func(a, b) { return Math.max(a, b); }\nfunc",
            ))
            .unwrap();
        let a = res.as_callable().unwrap();
        let func_res = a
            .call(
                &boa_engine::JsValue::null(),
                &[JsValue::new(1), JsValue::new(5)],
                &mut boa_ctx,
            )
            .unwrap();
        let func_res_number = func_res.to_i32(&mut boa_ctx).unwrap();
        assert_eq!(func_res_number, 5);
    }
}

#[cfg(test)]
mod specific_bugs_tests {
    use std::sync::Arc;

    use ruse_object_graph::{
        graph_map_value::GraphMapWrap, root_name, vnum, vstr, GraphIdGenerator, GraphsMap, Number,
        ObjectType, ValueType,
    };
    use ruse_synthesizer::{
        context::{Context, ContextArray, ValuesMap},
        embedding::merge_context_arrays,
        op_chain,
        synthesizer_context::SynthesizerContext,
        test::helpers::{evaluate_chain, get_composite_prog, get_init_prog},
    };

    use crate::{
        js_worker_context::create_js_worker_context,
        opcode::{ArrayIndexOp, ArrayLengthOp, ArrayPushOp, ArraySpliceOp},
        test::ts_op_helpers::*,
    };

    #[test]
    fn bug_1() {
        let ctx = ruse_synthesizer::test::helpers::generate_context_from_array(
            root_name!("names"),
            &ValueType::String,
            ["Augusta", "Ada", "King"].iter().map(|s| vstr!(*s)),
        );
        let ctx_arr = ContextArray::from(vec![ctx]);
        let variables = [(
            root_name!("names"),
            ruse_synthesizer::context::Variable {
                name: root_name!("names"),
                value_type: ValueType::Object(ObjectType::Array(Box::new(ValueType::String))),
                immutable: false,
            },
        )]
        .into();
        let syn_ctx = SynthesizerContext::from_context_array(ctx_arr.clone(), variables);
        let mut worker_ctx = create_js_worker_context(0);

        let id_op = id_op("names");
        let one_op = lit_number_op(1);
        let splice_op = Arc::new(ArraySpliceOp::new(&ValueType::String, false));
        let len_op = Arc::new(ArrayLengthOp::new(&ValueType::String));

        let names_prog = get_init_prog(id_op, &ctx_arr, &syn_ctx, &mut worker_ctx);
        let one_prog = get_init_prog(one_op, &ctx_arr, &syn_ctx, &mut worker_ctx);
        let splice_prog = get_composite_prog(
            splice_op,
            vec![names_prog.clone(), one_prog.clone()],
            &syn_ctx,
            &mut worker_ctx,
        );
        let len_prog =
            get_composite_prog(len_op, vec![names_prog.clone()], &syn_ctx, &mut worker_ctx);

        println!("{}", splice_prog);
        println!("");
        println!("{}", len_prog);

        let res = merge_context_arrays(
            splice_prog.pre_ctx(),
            splice_prog.post_ctx(),
            len_prog.pre_ctx(),
            len_prog.post_ctx(),
        );
        assert!(res.is_err());
        // This isn't really the bug..
        // The bug was in the iterator, but I'll keep this anyway
        // The bug occured as if I need 4 children
        // [1, 2, 3, 4]
        // the iterator advanced
        // [1, 2', 3', 4']
        // Setting the merged contexts failed at 2.
        // Now we are contiue to the next set of children
        // [1, 2', 3', 4'']
        // Now we set the context only for 4
    }

    #[test]
    fn bug_2() {
        let ctx = ruse_synthesizer::test::helpers::generate_context_from_array(
            root_name!("arr"),
            &ValueType::Number,
            [8, 9, 7].iter().map(|s| vnum!(Number::from(*s))),
        );
        let ctx_arr = ContextArray::from(vec![ctx]);
        let variables = [(
            root_name!("arr"),
            ruse_synthesizer::context::Variable {
                name: root_name!("arr"),
                value_type: ValueType::Object(ObjectType::Array(Box::new(ValueType::Number))),
                immutable: false,
            },
        )]
        .into();
        let syn_ctx = SynthesizerContext::from_context_array(ctx_arr.clone(), variables);
        let mut worker_ctx = create_js_worker_context(0);

        let arr_op = id_op("arr");
        let zero_op = lit_number_op(0);
        let one_op = lit_number_op(1);
        let push_op = Arc::new(ArrayPushOp::new(&ValueType::Number));
        let splice_op = Arc::new(ArraySpliceOp::new(&ValueType::Number, true));
        let array_index_op = Arc::new(ArrayIndexOp::new(&ValueType::Number));

        // In arr.push(x) x is evaluated first,
        let progs = evaluate_chain(
            op_chain!("final", &push_op;
                op_chain!("arr2", &arr_op),
                op_chain!("spliced[0]", &array_index_op;
                    op_chain!("spliced", &splice_op;
                        op_chain!("arr1", &arr_op),
                        op_chain!("1", &one_op),
                        op_chain!("1", &one_op)
                    ),
                    op_chain!("zero", &zero_op)
                )
            ),
            &ctx_arr,
            &syn_ctx,
            &mut worker_ctx,
        );

        let expected_ctx = ruse_synthesizer::test::helpers::generate_context_from_array(
            root_name!("arr"),
            &ValueType::Number,
            [8, 7, 9].iter().map(|s| vnum!(Number::from(*s))),
        );
        let final_arr = progs["final"].post_ctx()[0]
            .get_var_loc_value(&root_name!("arr"), syn_ctx.variables())
            .unwrap();
        let expected_arr = expected_ctx
            .get_var_loc_value(&root_name!("arr"), syn_ctx.variables())
            .unwrap();

        println!(
            "{}\n",
            final_arr
                .val()
                .wrap(&progs["final"].post_ctx()[0].graphs_map)
        );
        println!("{}\n", expected_arr.val().wrap(&expected_ctx.graphs_map));

        assert_eq!(
            final_arr.wrap(&progs["final"].post_ctx()[0].graphs_map),
            expected_arr.wrap(&expected_ctx.graphs_map)
        );
    }

    #[test]
    fn bug_3() {
        let graph_id_gen: Arc<GraphIdGenerator> = Arc::new(GraphIdGenerator::default());
        let mut graphs_map = GraphsMap::default();
        let mut values = ValuesMap::default();

        ruse_synthesizer::test::helpers::add_array_to_values(
            &mut values,
            &mut graphs_map,
            &graph_id_gen,
            root_name!("arr"),
            &ValueType::Number,
            [8, 9, 7].iter().map(|s| vnum!(Number::from(*s))),
        );
        values.insert(root_name!("i"), vnum!(Number::from(1)));

        let ctx = Context::with_values(values, graphs_map.into(), graph_id_gen);
        let ctx_arr = ContextArray::from(vec![ctx]);
        let variables = [(
            root_name!("arr"),
            ruse_synthesizer::context::Variable {
                name: root_name!("arr"),
                value_type: ValueType::Object(ObjectType::Array(Box::new(ValueType::Number))),
                immutable: false,
            },
        ),(
            root_name!("i"),
            ruse_synthesizer::context::Variable {
                name: root_name!("i"),
                value_type: ValueType::Number,
                immutable: false,
            },
        )].into();
        let syn_ctx = SynthesizerContext::from_context_array(ctx_arr.clone(), variables);
        let mut worker_ctx = create_js_worker_context(0);

        let arr_op = id_op("arr");
        let i_op = id_op("i");
        let zero_op = lit_number_op(0);
        let one_op = lit_number_op(1);
        let splice_op = Arc::new(ArraySpliceOp::new(&ValueType::Number, true));
        let array_index_op = Arc::new(ArrayIndexOp::new(&ValueType::Number));
        let push_op = Arc::new(ArrayPushOp::new(&ValueType::Number));

        let progs = evaluate_chain(
            op_chain!("final", &push_op;
                op_chain!("arr2", &arr_op),
                op_chain!("spliced(i, 1)[0]", &array_index_op;
                    op_chain!("spliced", &splice_op;
                        op_chain!("arr1", &arr_op),
                        op_chain!("i", &i_op),
                        op_chain!("1", &one_op)
                    ),
                    op_chain!("zero", &zero_op)
                )
            ),
            &ctx_arr,
            &syn_ctx,
            &mut worker_ctx,
        );

        let expected_ctx = ruse_synthesizer::test::helpers::generate_context_from_array(
            root_name!("arr"),
            &ValueType::Number,
            [8, 7, 9].iter().map(|s| vnum!(Number::from(*s))),
        );
        let final_arr = progs["final"].post_ctx()[0]
            .get_var_loc_value(&root_name!("arr"), syn_ctx.variables())
            .unwrap();
        let expected_arr = expected_ctx
            .get_var_loc_value(&root_name!("arr"), syn_ctx.variables())
            .unwrap();

        assert_eq!(
            final_arr.wrap(&progs["final"].post_ctx()[0].graphs_map),
            expected_arr.wrap(&expected_ctx.graphs_map)
        );
    }
}

// #[cfg(test)]
// mod swc_parser {
//     use std::path::Path;
//     use std::sync::Arc;

//     use swc_common::errors::{ColorConfig, Handler};
//     use swc_common::{Mark, SourceMap};
//     use swc_ecma_ast::{self as ast, Pass};
//     use swc_ecma_parser::{Syntax, TsSyntax};
//     use swc_ecma_visit::{VisitMutWith, VisitWith};

//     use crate::dts_visitor::DtsVisitor;

//     #[test]
//     fn test_dts_parser_dts_file() {
//         let cm = Arc::<SourceMap>::default();
//         let handler = Handler::with_tty_emitter(ColorConfig::Auto, true, false, Some(cm.clone()));
//         let fm = cm
//             .load_file(Path::new(
//                 "../../benchmarks/tasks/fromFrangel/classes/linear/matrix_test.d.ts",
//             ))
//             .expect("failed to load file");

//         let c = swc::Compiler::new(cm.clone());

//         for file in cm.files().iter() {
//             println!("{}", file.name);
//         }

//         let dts_prog = swc_common::GLOBALS.set(&Default::default(), || {
//             c.parse_js(
//                 fm,
//                 &handler,
//                 ast::EsVersion::Es2022,
//                 Syntax::Typescript(TsSyntax {
//                     tsx: false,
//                     decorators: false,
//                     dts: true,
//                     no_early_errors: false,
//                     disallow_ambiguous_jsx_like: false,
//                 }),
//                 swc::config::IsModule::Bool(true),
//                 None,
//             ).unwrap()
//         });

//         let mut dts_visitor = DtsVisitor::default();
//         dts_prog.visit_with(&mut dts_visitor);
//         println!("{:#?}", dts_visitor.classes);
//         println!("{:#?}", dts_visitor.functions);
//         println!("{:#?}", dts_visitor.globals);
//     }

//     #[test]
//     fn test_dts_parser() {
//         println!("{}", std::env::current_dir().unwrap().display());
//         let cm = Arc::<SourceMap>::default();
//         let handler = Handler::with_tty_emitter(ColorConfig::Auto, true, false, Some(cm.clone()));
//         let fm1 = cm
//             .load_file(Path::new(
//                 "../../benchmarks/tasks/fromFrangel/classes/linear/matrix.ts",
//             ))
//             .expect("failed to load file");
//         let fm2 = cm
//             .load_file(Path::new(
//                 "../../benchmarks/tasks/fromFrangel/classes/linear/svd.ts",
//             ))
//             .expect("failed to load file");
//         let fm3 = cm
//             .load_file(Path::new(
//                 "../../benchmarks/tasks/fromFrangel/classes/linear/matrix.d.ts",
//             ))
//             .expect("failed to load file");

//         let c = swc::Compiler::new(cm.clone());

//         for file in cm.files().iter() {
//             println!("{}", file.name);
//         }

//         let (optimized, dts_prog) = swc_common::GLOBALS.set(&Default::default(), || {
//             let mut program = c
//                 .parse_js(
//                     fm1.clone(),
//                     &handler,
//                     ast::EsVersion::Es2022,
//                     Syntax::Typescript(TsSyntax {
//                         tsx: false,
//                         decorators: false,
//                         dts: false,
//                         no_early_errors: false,
//                         disallow_ambiguous_jsx_like: false,
//                     }),
//                     swc::config::IsModule::Bool(true),
//                     None,
//                 )
//                 .unwrap();

//             let unresolved_mark = Mark::from_u32(2048);
//             let top_level_mark = Mark::from_u32(1024);

//             let mut optimized = c.run_transform(&handler, false, || {
//                 program.mutate(&mut swc_ecma_transforms::resolver(
//                     unresolved_mark,
//                     top_level_mark,
//                     true,
//                 ));

//                 program.visit_mut_with(&mut swc_ecma_transforms::hygiene::hygiene_with_config(
//                     swc_ecma_transforms::hygiene::Config {
//                         top_level_mark: top_level_mark,
//                         keep_class_names: false,
//                         ..Default::default()
//                     },
//                 ));
//                 program
//             });

//             let mut dts_prog = optimized.clone();
//             let mut fast_dts = swc_typescript::fast_dts::FastDts::new(
//                 fm1.name.clone(),
//                 unresolved_mark,
//                 Default::default(),
//             );
//             let issues = fast_dts.transform(&mut dts_prog);

//             for issue in issues {
//                 handler
//                     .struct_span_err(issue.range.span, &issue.message)
//                     .emit();
//             }

//             let mut stripper =
//                 swc_ecma_transforms::typescript::strip(unresolved_mark, top_level_mark);
//             stripper.process(&mut optimized);

//             let mut simplifier =
//                 swc_ecma_transforms::optimization::simplifier(unresolved_mark, Default::default());
//             simplifier.process(&mut optimized);

//             (optimized, dts_prog)
//         });

//         let mut dts_visitor = DtsVisitor::default();
//         dts_prog.visit_with(&mut dts_visitor);
//         println!("{:#?}", dts_visitor.classes);
//         println!("{:#?}", dts_visitor.functions);
//         println!("{:#?}", dts_visitor.globals);
//         // optimized.visit_with(&mut DtsVisitor);

//         // println!("{}", to_code(&optimized));
//         // println!("{}", to_code(&dts_prog));
//     }
// }
