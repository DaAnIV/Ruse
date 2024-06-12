#[cfg(test)]
mod ts_simple_opcodes_tests {
    use std::sync::Arc;

    use context::SynthesizerContext;
    use ruse_synthesizer::opcode::ExprOpcode;
    use ruse_synthesizer::value::{Location, ValueType, VarLoc};
    use swc_ecma_ast as ast;

    use crate::opcode::*;
    use ruse_object_graph::Number;
    use ruse_object_graph::{str_cached, Cache};
    use ruse_synthesizer::*;

    #[test]
    fn add_numbers() {
        let cache = Arc::new(Cache::new());
        let ctx_arr = context_array!([]);
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
        assert_eq!(out.val(), &vnum!(Number::from(7u64)));
    }

    #[test]
    fn add_strings() {
        let cache = Arc::new(Cache::new());
        let ctx_arr = context_array!([]);
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
        assert_eq!(out.val(), &vstr!(cache; "ab"));
    }

    #[test]
    fn ident() {
        let cache = Arc::new(Cache::new());
        let ctx_arr = context_array![[(str_cached!(cache; "x"), vnum!(Number::from(7u64)))]];
        let syn_ctx = SynthesizerContext::from_context_array(ctx_arr, cache.clone());
        let ctx = &syn_ctx.start_context[0];
        let mut out_ctx = ctx.clone();
        let evaluator = IdentOp {
            name: str_cached!(cache; "x"),
        };
        let out = evaluator.eval(&[], &mut out_ctx, &syn_ctx).unwrap();
        assert_eq!(out.val(), &vnum!(Number::from(7u64)));
    }

    #[test]
    fn prefix_plus_plus() {
        let cache = Arc::new(Cache::new());
        let ctx_arr = context_array![[(str_cached!(cache; "x"), vnum!(Number::from(7u64)))]];
        let syn_ctx = SynthesizerContext::from_context_array(ctx_arr, cache.clone());
        let ctx = &syn_ctx.start_context[0];
        let id = IdentOp {
            name: str_cached!(cache; "x"),
        };
        let op = UpdateOp {
            op: ast::UpdateOp::PlusPlus,
            prefix: true,
        };
        let mut id_out_ctx = ctx.clone();
        let x_val = id.eval(&[], &mut id_out_ctx, &syn_ctx).unwrap();

        let mut update_out_ctx = id_out_ctx.clone();
        let out = op.eval(&[&x_val], &mut update_out_ctx, &syn_ctx).unwrap();
        assert_eq!(
            ctx.get_var_loc_value(&id.name).val(),
            &vnum!(Number::from(7u64))
        );
        assert_eq!(x_val.val(), &vnum!(Number::from(7u64)));
        assert_eq!(
            x_val.loc(),
            &Location::Var(VarLoc {
                var: id.name.clone()
            })
        );
        assert_eq!(out.val(), &vnum!(Number::from(8u64)));
        assert_eq!(out.loc(), &Location::Temp);
        assert_eq!(
            update_out_ctx.get_var_loc_value(&id.name).val(),
            &vnum!(Number::from(8u64))
        );
    }

    #[test]
    fn postfix_plus_plus() {
        let cache = Arc::new(Cache::new());
        let ctx_arr = context_array![[(str_cached!(cache; "x"), vnum!(Number::from(7u64)))]];
        let syn_ctx = SynthesizerContext::from_context_array(ctx_arr, cache.clone());
        let ctx = &syn_ctx.start_context[0];
        let id = IdentOp {
            name: str_cached!(cache; "x"),
        };
        let op = UpdateOp {
            op: ast::UpdateOp::PlusPlus,
            prefix: false,
        };
        let mut id_out_ctx = ctx.clone();
        let x_val = id.eval(&[], &mut id_out_ctx, &syn_ctx).unwrap();

        let mut update_out_ctx = id_out_ctx.clone();
        let out = op.eval(&[&x_val], &mut update_out_ctx, &syn_ctx).unwrap();
        assert_eq!(
            ctx.get_var_loc_value(&id.name).val(),
            &vnum!(Number::from(7u64))
        );
        assert_eq!(x_val.val(), &vnum!(Number::from(7u64)));
        assert_eq!(
            x_val.loc(),
            &Location::Var(VarLoc {
                var: id.name.clone()
            })
        );
        assert_eq!(out.val(), &vnum!(Number::from(7u64)));
        assert_eq!(out.loc(), &Location::Temp);
        assert_eq!(
            update_out_ctx.get_var_loc_value(&id.name).val(),
            &vnum!(Number::from(8u64))
        );
    }
}

#[cfg(test)]
mod ts_class_tests {
    use std::{collections::HashMap, sync::Arc};

    use boa_engine::{js_string, property::Attribute};
    use ruse_object_graph::{str_cached, Cache};
    use ruse_synthesizer::{context::Context, value::ValueType, vstr};

    use crate::ts_class::TsClasses;

    #[test]
    fn generate_object() {
        let code = "class User {
            constructor(public name: string, 
                        public surname: string) {}
        }";
        let cache = Arc::new(Cache::new());
        let classes = TsClasses::new();
        let user_class_name = classes
            .add_class(code, &cache)
            .expect("Failed to add User class");
        let user = classes
            .get_class(&user_class_name)
            .unwrap()
            .generate_object(HashMap::from([
                (str_cached!(cache; "surname"), vstr!(cache; "Doe")),
                (str_cached!(cache; "name"), vstr!(cache; "John")),
            ]))
            .obj()
            .unwrap()
            .clone();

        let name_field = user.get_field_value(&str_cached!(cache; "name")).unwrap();
        assert_eq!(name_field, vstr!(cache; "John"))
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

        assert!(student_class.fields.get(&str_cached!(cache; "name")).is_some());
        assert!(student_class.fields.get(&str_cached!(cache; "surname")).is_some());
        assert!(student_class.fields.get(&str_cached!(cache; "age")).is_some());
        assert!(student_class.fields.get(&str_cached!(cache; "grades")).is_some());
        assert!(class_class.fields.get(&str_cached!(cache; "students")).is_some());

        assert_eq!(student_class.fields[&str_cached!(cache; "name")], ValueType::String);
        assert_eq!(student_class.fields[&str_cached!(cache; "surname")], ValueType::String);
        assert_eq!(student_class.fields[&str_cached!(cache; "age")], ValueType::Number);
        assert_eq!(student_class.fields[&str_cached!(cache; "grades")], ValueType::array_value_type(&ValueType::Number, &cache));
        assert_eq!(class_class.fields[&str_cached!(cache; "students")], ValueType::array_value_type(&student_class.obj_type(), &cache));
    }

    #[test]
    fn simple_js_object_eval() {
        let code = "class User {
            constructor(public name: string, 
                        public surname: string) {}
        }";

        let cache = Arc::new(Cache::new());
        let classes = TsClasses::new();
        let mut ctx = Context::with_values(Default::default());
        let mut boa_ctx = classes.get_boa_ctx(&mut ctx, &cache);
        let user_class_name = classes
            .add_class(code, &cache)
            .expect("Failed to add User class");
        let user_class = classes.get_class(&user_class_name).unwrap();
        let user = user_class
            .generate_object(HashMap::from([
                (str_cached!(cache; "surname"), vstr!(cache; "Doe")),
                (str_cached!(cache; "name"), vstr!(cache; "John")),
            ]))
            .obj()
            .unwrap()
            .clone();
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
        let classes = TsClasses::new();
        let mut ctx = Context::with_values(Default::default());
        let mut boa_ctx = classes.get_boa_ctx(&mut ctx, &cache);
        let user_class_name = classes.add_class(code1, &cache).unwrap();
        let user_pair_class_name = classes.add_class(code2, &cache).unwrap();
        let user_class = classes.get_class(&user_class_name).unwrap();
        let user_class_pair = classes.get_class(&user_pair_class_name).unwrap();

        let user1 = user_class.generate_rooted_object(
            str_cached!(cache; "student1"),
            HashMap::from([
                (str_cached!(cache; "surname"), vstr!(cache; "Doe")),
                (str_cached!(cache; "name"), vstr!(cache; "John")),
            ]),
        );

        let user2 = user_class.generate_rooted_object(
            str_cached!(cache; "student2"),
            HashMap::from([
                (str_cached!(cache; "name"), vstr!(cache; "Paul")),
                (str_cached!(cache; "surname"), vstr!(cache; "Simon")),
            ]),
        );

        let complex_user = user_class_pair
            .generate_rooted_object(
                str_cached!(cache; "student_pair"),
                HashMap::from([
                    (str_cached!(cache; "user1"), user1),
                    (str_cached!(cache; "user2"), user2),
                ]),
            )
            .obj()
            .unwrap()
            .clone();
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
