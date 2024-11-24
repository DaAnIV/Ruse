#[cfg(test)]
mod ts_simple_opcodes_tests {
    use std::sync::Arc;

    use context::{Context, ContextArray, GraphIdGenerator, SynthesizerContext};
    use graph_map_value::GraphMapWrap;
    use ruse_object_graph::{value::*, *};
    use ruse_synthesizer::location::{Location, VarLoc};
    use ruse_synthesizer::opcode::ExprOpcode;
    use swc_ecma_ast as ast;

    use crate::opcode::*;
    use ruse_object_graph::Number;
    use ruse_object_graph::{str_cached, Cache};
    use ruse_synthesizer::*;

    #[test]
    fn add_numbers() {
        let cache = Arc::new(Cache::new());
        let graphs_map = GraphsMap::default();
        let ctx_arr = ContextArray::default();
        let syn_ctx = SynthesizerContext::from_context_array(ctx_arr, cache.clone());
        let ctx = &syn_ctx.start_context[0].clone();
        let mut out_ctx = ctx.clone();
        let evaluator = BinOp {
            op: ast::BinaryOp::Add,
            arg_types: [ValueType::Number, ValueType::Number],
        };

        let args = [
            &ctx.temp_value(vnum!(Number::from(3u64))),
            &ctx.temp_value(vnum!(Number::from(4u64))),
        ];
        let out = evaluator.eval(&args, &mut out_ctx, &syn_ctx).unwrap();
        assert_eq!(
            out.val().wrap(&graphs_map),
            vnum!(Number::from(7u64)).wrap(&graphs_map)
        );
    }

    #[test]
    fn add_strings() {
        let cache = Arc::new(Cache::new());
        let graphs_map = GraphsMap::default();
        let ctx_arr = ContextArray::default();
        let syn_ctx = SynthesizerContext::from_context_array(ctx_arr, cache.clone());
        let ctx = &syn_ctx.start_context[0];
        let mut out_ctx = ctx.clone();
        let evaluator = BinOp {
            op: ast::BinaryOp::Add,
            arg_types: [ValueType::String, ValueType::String],
        };

        let args = [
            &ctx.temp_value(vstr!(cache; "a")),
            &ctx.temp_value(vstr!(cache; "b")),
        ];

        let out = evaluator.eval(&args, &mut out_ctx, &syn_ctx).unwrap();
        assert_eq!(
            out.val().wrap(&graphs_map),
            vstr!(cache; "ab").wrap(&graphs_map)
        );
    }

    #[test]
    fn ident() {
        let cache = Arc::new(Cache::new());
        let graphs_map = GraphsMap::default();
        let id_gen = Arc::new(GraphIdGenerator::default());
        let ctx_arr = ContextArray::from(vec![Context::with_values(
            [(str_cached!(cache; "x"), vnum!(Number::from(7u64)))].into(),
            graphs_map.into(),
            id_gen,
        )]);
        let syn_ctx = SynthesizerContext::from_context_array(ctx_arr, cache.clone());
        let ctx = &syn_ctx.start_context[0];
        let mut out_ctx = ctx.clone();
        let evaluator = IdentOp::new(str_cached!(cache; "x"));
        let out = evaluator.eval(&[], &mut out_ctx, &syn_ctx).unwrap();
        assert_eq!(
            out.val().wrap(&out_ctx.graphs_map),
            vnum!(Number::from(7u64)).wrap(&out_ctx.graphs_map)
        );
    }

    #[test]
    fn prefix_plus_plus() {
        let cache = Arc::new(Cache::new());
        let graphs_map = GraphsMap::default();
        let id_gen = Arc::new(GraphIdGenerator::default());
        let ctx_arr = ContextArray::from(vec![Context::with_values(
            [(str_cached!(cache; "x"), vnum!(Number::from(7u64)))].into(),
            graphs_map.into(),
            id_gen,
        )]);
        let syn_ctx = SynthesizerContext::from_context_array(ctx_arr, cache.clone());
        let ctx = &syn_ctx.start_context[0];
        let id = IdentOp::new(str_cached!(cache; "x"));
        let op = UpdateOp {
            op: ast::UpdateOp::PlusPlus,
            prefix: true,
        };
        let mut id_out_ctx = ctx.clone();
        let x_val = id.eval(&[], &mut id_out_ctx, &syn_ctx).unwrap();

        let mut update_out_ctx = id_out_ctx.clone();
        let out = op.eval(&[&x_val], &mut update_out_ctx, &syn_ctx).unwrap();
        assert_eq!(
            ctx.get_var_loc_value(&id.name)
                .expect("Didn't find var")
                .val()
                .wrap(&ctx.graphs_map),
            vnum!(Number::from(7u64)).wrap(&ctx.graphs_map)
        );
        assert_eq!(
            x_val.val().wrap(&ctx.graphs_map),
            vnum!(Number::from(7u64)).wrap(&ctx.graphs_map)
        );
        assert_eq!(
            x_val.loc(),
            &Location::Var(VarLoc {
                var: id.name.clone()
            })
        );
        assert_eq!(
            out.val().wrap(&update_out_ctx.graphs_map),
            vnum!(Number::from(8u64)).wrap(&update_out_ctx.graphs_map)
        );
        assert_eq!(out.loc(), &Location::Temp);
        assert_eq!(
            update_out_ctx
                .get_var_loc_value(&id.name)
                .expect("Didn't find var")
                .val()
                .wrap(&update_out_ctx.graphs_map),
            vnum!(Number::from(8u64)).wrap(&update_out_ctx.graphs_map)
        );
    }

    #[test]
    fn postfix_plus_plus() {
        let cache = Arc::new(Cache::new());
        let graphs_map = GraphsMap::default();
        let id_gen = Arc::new(GraphIdGenerator::default());
        let ctx_arr = ContextArray::from(vec![Context::with_values(
            [(str_cached!(cache; "x"), vnum!(Number::from(7u64)))].into(),
            graphs_map.into(),
            id_gen,
        )]);
        let syn_ctx = SynthesizerContext::from_context_array(ctx_arr, cache.clone());
        let ctx = &syn_ctx.start_context[0];
        let id = IdentOp::new(str_cached!(cache; "x"));
        let op = UpdateOp {
            op: ast::UpdateOp::PlusPlus,
            prefix: false,
        };
        let mut id_out_ctx = ctx.clone();
        let x_val = id.eval(&[], &mut id_out_ctx, &syn_ctx).unwrap();

        let mut update_out_ctx = id_out_ctx.clone();
        let out = op.eval(&[&x_val], &mut update_out_ctx, &syn_ctx).unwrap();
        assert_eq!(
            ctx.get_var_loc_value(&id.name)
                .expect("Didn't find var")
                .val()
                .wrap(&ctx.graphs_map),
            vnum!(Number::from(7u64)).wrap(&ctx.graphs_map)
        );
        assert_eq!(
            x_val.val().wrap(&ctx.graphs_map),
            vnum!(Number::from(7u64)).wrap(&ctx.graphs_map)
        );
        assert_eq!(
            x_val.loc(),
            &Location::Var(VarLoc {
                var: id.name.clone()
            })
        );
        assert_eq!(
            out.val().wrap(&update_out_ctx.graphs_map),
            vnum!(Number::from(7u64)).wrap(&update_out_ctx.graphs_map)
        );
        assert_eq!(out.loc(), &Location::Temp);
        assert_eq!(
            update_out_ctx
                .get_var_loc_value(&id.name)
                .expect("Didn't find var")
                .val()
                .wrap(&update_out_ctx.graphs_map),
            vnum!(Number::from(8u64)).wrap(&update_out_ctx.graphs_map)
        );
    }

    #[test]
    fn array_push() {
        let cache = Arc::new(Cache::new());
        let mut graphs_map = GraphsMap::default();
        let id_gen = Arc::new(GraphIdGenerator::default());
        let mut graph = ObjectGraph::new(id_gen.get_id_for_graph());
        let arr = ObjectValue {
            graph_id: graph.id,
            node: graph.add_array_object(id_gen.get_id_for_node(), &ValueType::Number, [], &cache),
        };
        graphs_map.insert_graph(graph.into());
        let ctx_arr = ContextArray::from(vec![Context::with_values(
            [(str_cached!(cache; "x"), Value::Object(arr))].into(),
            graphs_map.into(),
            id_gen,
        )]);
        let syn_ctx = SynthesizerContext::from_context_array(ctx_arr, cache.clone());
        let ctx = &syn_ctx.start_context[0];
        let id = IdentOp::new(str_cached!(cache; "x"));
        let op = ArrayPushOp::new(&ValueType::Number, &syn_ctx.cache);

        let mut id_out_ctx = ctx.clone();
        let x_val = id.eval(&[], &mut id_out_ctx, &syn_ctx).unwrap();

        let mut update_out_ctx = id_out_ctx.clone();
        let num_to_push = update_out_ctx.temp_value(vnum!(Number::from(1)));
        let out = op
            .eval(&[&x_val, &num_to_push], &mut update_out_ctx, &syn_ctx)
            .unwrap();

        let orig_array = ctx
            .get_var_loc_value(&id.name)
            .expect("Didn't find var")
            .val()
            .obj()
            .unwrap()
            .clone();
        let updated_array = update_out_ctx
            .get_var_loc_value(&id.name)
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
                .get_primitive_field_value(&syn_ctx.cached_string("0"), &update_out_ctx.graphs_map)
                .unwrap()
                .wrap(&update_out_ctx.graphs_map),
            vnum!(Number::from(1)).wrap(&update_out_ctx.graphs_map)
        );

        assert_eq!(out.loc(), &Location::Temp);
        assert_eq!(
            out.val().wrap(&update_out_ctx.graphs_map),
            vnum!(Number::from(1u64)).wrap(&update_out_ctx.graphs_map)
        );
    }
}

#[cfg(test)]
mod ts_class_tests {
    use std::{collections::HashMap, sync::Arc};

    use boa_engine::{js_string, property::Attribute};
    use graph_map_value::GraphMapWrap;
    use ruse_object_graph::{str_cached, Cache};
    use ruse_object_graph::{value::*, *};
    use ruse_synthesizer::context::{Context, GraphIdGenerator};

    use crate::ts_class::TsClasses;

    #[test]
    fn generate_object() {
        let code = "class User {
            constructor(public name: string, 
                        public surname: string) {}
        }";
        let cache = Arc::new(Cache::new());
        let mut graphs_map = GraphsMap::default();
        let id_gen = Arc::new(GraphIdGenerator::default());
        let classes = TsClasses::new();
        let user_class_name = classes
            .add_class(code, &cache)
            .expect("Failed to add User class");
        let mut graph = ObjectGraph::new(id_gen.get_id_for_graph());
        let user = classes
            .get_class(&user_class_name)
            .unwrap()
            .generate_object(
                HashMap::from([
                    (str_cached!(cache; "surname"), vstr!(cache; "Doe")),
                    (str_cached!(cache; "name"), vstr!(cache; "John")),
                ]),
                &mut graph,
                &id_gen,
            );
        graphs_map.insert_graph(graph.into());

        let name_field = user
            .get_field_value(&str_cached!(cache; "name"), &graphs_map)
            .unwrap();
        assert_eq!(
            name_field.wrap(&graphs_map),
            vstr!(cache; "John").wrap(&graphs_map)
        )
    }

    #[test]
    fn member_opcodes() {
        let code = "class User {
            constructor(public name: string, 
                        public surname: string) {}
        }";
        let cache = Arc::new(Cache::new());
        let classes = TsClasses::new();
        let user_class_name = classes
            .add_class(code, &cache)
            .expect("Failed to add User class");

        let user_class = classes.get_class(&user_class_name).unwrap();
        let opcodes = &user_class.member_opcodes;
        assert_eq!(opcodes.len(), 2);
        assert!(opcodes.iter().all(|op| {
            op.arg_types().len() == 1
                && op.arg_types()[0] == ValueType::Object(user_class_name.clone())
        }));
        // Need to check the opcodes are correct?
    }

    #[test]
    fn object_fields() {
        let code1 = "class Student {
            constructor(public name: string, 
                        public surname: string,
                        public age: number,
                        public grades: number[]) {}
        }";
        let code2 = "class Class {
            constructor(public students: Student[]) {}
        }";

        let cache = Arc::new(Cache::new());
        let classes = TsClasses::new();

        let student_class_name = classes.add_class(code1, &cache).unwrap();
        let class_class_name = classes.add_class(code2, &cache).unwrap();
        let student_class = classes.get_class(&student_class_name).unwrap();
        let class_class = classes.get_class(&class_class_name).unwrap();

        assert!(student_class
            .fields
            .get(&str_cached!(cache; "name"))
            .is_some());
        assert!(student_class
            .fields
            .get(&str_cached!(cache; "surname"))
            .is_some());
        assert!(student_class
            .fields
            .get(&str_cached!(cache; "age"))
            .is_some());
        assert!(student_class
            .fields
            .get(&str_cached!(cache; "grades"))
            .is_some());
        assert!(class_class
            .fields
            .get(&str_cached!(cache; "students"))
            .is_some());

        assert_eq!(
            student_class.fields[&str_cached!(cache; "name")],
            ValueType::String
        );
        assert_eq!(
            student_class.fields[&str_cached!(cache; "surname")],
            ValueType::String
        );
        assert_eq!(
            student_class.fields[&str_cached!(cache; "age")],
            ValueType::Number
        );
        assert_eq!(
            student_class.fields[&str_cached!(cache; "grades")],
            ValueType::array_value_type(&ValueType::Number, &cache)
        );
        assert_eq!(
            class_class.fields[&str_cached!(cache; "students")],
            ValueType::array_value_type(&student_class.obj_type(), &cache)
        );
    }

    #[test]
    fn simple_js_object_eval() {
        let code = "class User {
            constructor(public name: string, 
                        public surname: string) {}
        }";

        let cache = Arc::new(Cache::new());
        let mut graphs_map = GraphsMap::default();
        let id_gen = Arc::new(GraphIdGenerator::default());
        let classes = TsClasses::new();
        let user_class_name = classes
            .add_class(code, &cache)
            .expect("Failed to add User class");
        let user_class = classes.get_class(&user_class_name).unwrap();
        let mut graph = ObjectGraph::new(id_gen.get_id_for_graph());
        let user = user_class.generate_object(
            HashMap::from([
                (str_cached!(cache; "surname"), vstr!(cache; "Doe")),
                (str_cached!(cache; "name"), vstr!(cache; "John")),
            ]),
            &mut graph,
            &id_gen,
        );
        graphs_map.insert_graph(graph.into());

        let mut ctx = Context::with_values([].into(), graphs_map.into(), id_gen);
        let mut boa_ctx = classes.get_boa_ctx(&mut ctx, &cache);

        let js_user = user_class.generate_js_object(&classes, user, &mut boa_ctx, &cache);
        boa_ctx
            .register_global_property(js_string!("u"), js_user, Attribute::all())
            .expect("Failed to register p");

        let js_code = boa_engine::Source::from_bytes("u.name + \" \" + u.surname");
        let res = boa_ctx.eval(js_code).unwrap();
        assert!(res.is_string());
        assert_eq!(res.as_string().unwrap(), &js_string!("John Doe"));
    }

    #[test]
    fn complex_js_object_eval() {
        let code1 = "class User {
            constructor(public name: string, 
                        public surname: string,
                        public age: number,
                        protected is_admin: bool,
                        public grades: number[]) {}
        }";
        let code2 = "class UserPair {
            constructor(public user1: User, 
                        public user2: User) {}
        }";

        let cache = Arc::new(Cache::new());
        let mut graphs_map = GraphsMap::default();
        let id_gen = Arc::new(GraphIdGenerator::default());

        let classes = TsClasses::new();
        let user_class_name = classes.add_class(code1, &cache).unwrap();
        let user_pair_class_name = classes.add_class(code2, &cache).unwrap();
        let user_class = classes.get_class(&user_class_name).unwrap();
        let user_class_pair = classes.get_class(&user_pair_class_name).unwrap();

        let mut user1_graph = ObjectGraph::new(id_gen.get_id_for_graph());
        let user1 = user_class.generate_rooted_object(
            str_cached!(cache; "student1"),
            HashMap::from([
                (str_cached!(cache; "surname"), vstr!(cache; "Doe")),
                (str_cached!(cache; "name"), vstr!(cache; "John")),
            ]),
            &mut user1_graph,
            &id_gen,
        );
        graphs_map.insert_graph(user1_graph.into());

        let mut user2_graph = ObjectGraph::new(id_gen.get_id_for_graph());
        let user2 = user_class.generate_rooted_object(
            str_cached!(cache; "student2"),
            HashMap::from([
                (str_cached!(cache; "name"), vstr!(cache; "Paul")),
                (str_cached!(cache; "surname"), vstr!(cache; "Simon")),
            ]),
            &mut user2_graph,
            &id_gen,
        );
        graphs_map.insert_graph(user2_graph.into());

        let mut complex_user_graph = ObjectGraph::new(id_gen.get_id_for_graph());
        let complex_user = user_class_pair.generate_rooted_object(
            str_cached!(cache; "student_pair"),
            HashMap::from([
                (str_cached!(cache; "user1"), Value::Object(user1)),
                (str_cached!(cache; "user2"), Value::Object(user2)),
            ]),
            &mut complex_user_graph,
            &id_gen,
        );
        graphs_map.insert_graph(complex_user_graph.into());

        let mut ctx = Context::with_values([].into(), graphs_map.into(), id_gen);
        let mut boa_ctx = classes.get_boa_ctx(&mut ctx, &cache);

        let js_obj =
            user_class_pair.generate_js_object(&classes, complex_user, &mut boa_ctx, &cache);
        boa_ctx
            .register_global_property(js_string!("up"), js_obj, Attribute::all())
            .expect("Failed to register p");

        let js_code = boa_engine::Source::from_bytes("up.user1.name + \" \" + up.user2.name");
        let res = boa_ctx.eval(js_code).unwrap();
        assert!(res.is_string());
        assert_eq!(res.as_string().unwrap(), &js_string!("John Paul"));
    }
}

#[cfg(test)]
mod specific_bugs_tests {
    use std::sync::Arc;

    use object_graph::str_cached;
    use ruse_object_graph::{self as object_graph, value::ValueType, vstr, Cache, Number};
    use ruse_synthesizer::{
        context::{ContextArray, SynthesizerContext},
        embedding::merge_context_arrays,
        opcode::ExprOpcode,
        prog::SubProgram,
    };

    use crate::opcode::{ArrayLengthOp, ArraySpliceOp, IdentOp, LitOp};

    #[test]
    fn bug_1() {
        let cache = Arc::new(Cache::new());

        let ctx = ruse_synthesizer::test::helpers::generate_context_from_array(
            str_cached!(cache; "names"),
            &ValueType::String,
            ["Augusta", "Ada", "King"].iter().map(|s| vstr!(cache; s)),
            &cache,
        );
        let ctx_arr = ContextArray::from(vec![ctx]);
        let syn_ctx = SynthesizerContext::from_context_array(ctx_arr.clone(), cache);

        let id_op = Arc::new(IdentOp::new(syn_ctx.cached_string("names")));
        let mut names_prog = SubProgram::with_opcode(id_op, ctx_arr.clone(), ctx_arr.clone());
        assert!(Arc::get_mut(&mut names_prog).unwrap().evaluate(&syn_ctx));
        println!("{}", names_prog);

        let one_op = Arc::new(LitOp::Num(Number::from(1)));
        let one_ctx = ctx_arr
            .get_partial_context(one_op.required_variables())
            .unwrap();
        let mut one_prog = SubProgram::with_opcode(one_op, one_ctx.clone(), one_ctx.clone());
        assert!(Arc::get_mut(&mut one_prog).unwrap().evaluate(&syn_ctx));
        println!("{}", one_prog);
        println!("");

        let splice_op = Arc::new(ArraySpliceOp::new(&ValueType::String, 0, &syn_ctx.cache));
        let mut splice_prog = SubProgram::with_opcode_and_children(
            splice_op,
            vec![names_prog.clone(), one_prog.clone()],
            ctx_arr.clone(),
            ctx_arr.clone(),
        );
        assert!(Arc::get_mut(&mut splice_prog).unwrap().evaluate(&syn_ctx));
        println!("{}", splice_prog);
        println!("");

        let len_op = Arc::new(ArrayLengthOp::new(&ValueType::String, &syn_ctx.cache));
        let mut len_prog = SubProgram::with_opcode_and_children(
            len_op,
            vec![names_prog.clone()],
            ctx_arr.clone(),
            ctx_arr.clone(),
        );
        assert!(Arc::get_mut(&mut len_prog).unwrap().evaluate(&syn_ctx));
        println!("{}", len_prog);
        println!("");

        println!("{}", one_prog);

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
}
