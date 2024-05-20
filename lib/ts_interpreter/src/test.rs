#[cfg(test)]
mod ts_simple_opcodes_tests {
    use std::collections::HashMap;
    use std::sync::Arc;

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
        let ctx = context::Context::with_values(Default::default());
        let mut out_ctx = ctx.clone();
        let evaluator = BinOp {
            op: ast::BinaryOp::Add,
            arg_types: [ValueType::Number, ValueType::Number],
        };

        let args = [
            &ctx.temp_value(vnum!(Number::from(3u64))),
            &ctx.temp_value(vnum!(Number::from(4u64))),
        ];
        let out = evaluator.eval(&args, &mut out_ctx, &cache).unwrap();
        assert_eq!(out.val(), &vnum!(Number::from(7u64)));
    }

    #[test]
    fn add_strings() {
        let cache = Arc::new(Cache::new());
        let ctx = context::Context::with_values(Default::default());
        let mut out_ctx = ctx.clone();
        let evaluator = BinOp {
            op: ast::BinaryOp::Add,
            arg_types: [ValueType::String, ValueType::String],
        };

        let args = [
            &ctx.temp_value(vstr!(cache; "a")),
            &ctx.temp_value(vstr!(cache; "b")),
        ];

        let out = evaluator.eval(&args, &mut out_ctx, &cache).unwrap();
        assert_eq!(out.val(), &vstr!(cache; "ab"));
    }

    #[test]
    fn ident() {
        let cache = Arc::new(Cache::new());
        let ctx = context::Context::with_values(HashMap::from([(
            str_cached!(cache; "x"),
            vnum!(Number::from(7u64)),
        )]));
        let mut out_ctx = ctx.clone();
        let evaluator = IdentOp {
            name: str_cached!(cache; "x"),
        };
        let out = evaluator.eval(&[], &mut out_ctx, &cache).unwrap();
        assert_eq!(out.val(), &vnum!(Number::from(7u64)));
    }

    #[test]
    fn prefix_plus_plus() {
        let cache = Arc::new(Cache::new());
        let ctx = context::Context::with_values(HashMap::from([(
            str_cached!(cache; "x"),
            vnum!(Number::from(7u64)),
        )]));
        let id = IdentOp {
            name: str_cached!(cache; "x"),
        };
        let op = UpdateOp {
            op: ast::UpdateOp::PlusPlus,
            prefix: true,
        };
        let mut id_out_ctx = ctx.clone();
        let x_val = id.eval(&[], &mut id_out_ctx, &cache).unwrap();

        let mut update_out_ctx = id_out_ctx.clone();
        let out = op.eval(&[&x_val], &mut update_out_ctx, &cache).unwrap();
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
        let ctx = context::Context::with_values(HashMap::from([(
            str_cached!(cache; "x"),
            vnum!(Number::from(7u64)),
        )]));
        let id = IdentOp {
            name: str_cached!(cache; "x"),
        };
        let op = UpdateOp {
            op: ast::UpdateOp::PlusPlus,
            prefix: false,
        };
        let mut id_out_ctx = ctx.clone();
        let x_val = id.eval(&[], &mut id_out_ctx, &cache).unwrap();

        let mut update_out_ctx = id_out_ctx.clone();
        let out = op.eval(&[&x_val], &mut update_out_ctx, &cache).unwrap();
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
    use ruse_synthesizer::{value::ValueType, vstr};

    use crate::ts_class::TsClasses;

    #[test]
    fn generate_object() {
        let code = "class User {
            constructor(public name: string, 
                        public surname: string) {}
        }";
        let cache = Arc::new(Cache::new());
        let mut classes = TsClasses::new();
        let user_class_name = classes
            .add_class(code.to_string(), &cache)
            .expect("Failed to add User class");
        let user = classes
            .generate_object(
                &user_class_name,
                str_cached!(cache; "student"),
                HashMap::from([
                    (str_cached!(cache; "surname"), vstr!(cache; "Doe")),
                    (str_cached!(cache; "name"), vstr!(cache; "John")),
                ]),
            )
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
        let mut classes = TsClasses::new();
        let user_class_name = classes
            .add_class(code.to_string(), &cache)
            .expect("Failed to add User class");

        let opcodes = classes.class_members_opcodes(&user_class_name);
        assert_eq!(opcodes.len(), 2);
        assert!(opcodes.iter().all(|op| {
            op.arg_types().len() == 1
                && op.arg_types()[0] == ValueType::Object(user_class_name.clone())
        }));
        // Need to check the opcodes are correct?
    }

    #[test]
    fn simple_js_object_eval() {
        let code = "class User {
            constructor(public name: string, 
                        public surname: string) {}
        }";

        let mut boa_ctx = boa_engine::Context::default();
        let cache = Arc::new(Cache::new());
        let mut classes = TsClasses::new();
        let user_class_name = classes
            .add_class(code.to_string(), &cache)
            .expect("Failed to add User class");
        let user = classes
            .generate_object(
                &user_class_name,
                str_cached!(cache; "student"),
                HashMap::from([
                    (str_cached!(cache; "surname"), vstr!(cache; "Doe")),
                    (str_cached!(cache; "name"), vstr!(cache; "John")),
                ]),
            )
            .obj()
            .unwrap()
            .clone();
        let js_user = classes.generate_js_object(&user_class_name, user, &mut boa_ctx, &cache);
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
                        protected is_admin: bool) {}
        }";
        let code2 = "class UserPair {
            constructor(public user1: User, 
                        public user2: User) {}
        }";

        let mut boa_ctx = boa_engine::Context::default();
        let cache = Arc::new(Cache::new());
        let mut classes = TsClasses::new();
        let user_class_name = classes.add_class(code1.to_string(), &cache).unwrap();
        let user_pair_class_name = classes.add_class(code2.to_string(), &cache).unwrap();

        let user1 = classes.generate_object(
            &user_class_name,
            str_cached!(cache; "student1"),
            HashMap::from([
                (str_cached!(cache; "surname"), vstr!(cache; "Doe")),
                (str_cached!(cache; "name"), vstr!(cache; "John")),
            ]),
        );

        let user2 = classes.generate_object(
            &user_class_name,
            str_cached!(cache; "student2"),
            HashMap::from([
                (str_cached!(cache; "name"), vstr!(cache; "Paul")),
                (str_cached!(cache; "surname"), vstr!(cache; "Simon")),
            ]),
        );

        let complex_user = classes
            .generate_object(
                &user_pair_class_name,
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
            classes.generate_js_object(&user_pair_class_name, complex_user, &mut boa_ctx, &cache);
        boa_ctx
            .register_global_property(js_string!("up"), js_obj, Attribute::all())
            .expect("Failed to register p");

        let js_code = boa_engine::Source::from_bytes("up.user1.name + \" \" + up.user2.name");
        let res = boa_ctx.eval(js_code).unwrap();
        assert!(res.is_string());
        assert_eq!(res.as_string().unwrap(), &js_string!("John Paul"));
    }
}
