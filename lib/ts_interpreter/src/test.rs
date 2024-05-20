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
}
