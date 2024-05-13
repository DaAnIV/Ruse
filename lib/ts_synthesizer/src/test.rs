#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use object_graph::str_cached;
    use ruse_object_graph as object_graph;
    use ruse_synthesizer::{
        context::Context,
        value::{Location, ValueType},
        vstr,
    };
    use ruse_ts_interpreter::ts_class::TsClass;

    use crate::*;

    use tokio;

    #[tokio::test(flavor = "multi_thread")]
    async fn add_struct_fields() {
        let code = "class User {
            constructor(public name: string, 
                        public surname: string) {}
        }";
        let cache = Arc::new(object_graph::Cache::new());
        let user_class = TsClass::from_code(code.to_string(), &cache).unwrap();

        let user1 = user_class.generate_object(
            str_cached!(cache; "student"),
            HashMap::from([
                (str_cached!(cache; "surname"), vstr!(cache; "Doe")),
                (str_cached!(cache; "name"), vstr!(cache; "John")),
            ]),
        );
        let user2 = user_class.generate_object(
            str_cached!(cache; "student"),
            HashMap::from([
                (str_cached!(cache; "surname"), vstr!(cache; "Simon")),
                (str_cached!(cache; "name"), vstr!(cache; "Paul")),
            ]),
        );

        let mut opcodes = construct_opcode_list(
            &[str_cached!(cache; "x")],
            &[],
            &[str_cached!(cache; " ")],
            false,
        );
        add_str_opcodes(&mut opcodes, &ALL_BIN_STR_OPCODES);
        opcodes.extend(user_class.member_opcodes().clone());

        let ctx = Arc::new([
            Context::with_values([(str_cached!(cache; "x"), user1)].into()),
            Context::with_values([(str_cached!(cache; "x"), user2)].into()),
        ]);

        let cache_clone = cache.clone();
        let mut synthesizer = TsSynthesizer::new(
            ctx.clone(),
            opcodes,
            Box::new(move |p| {
                let expected_outputs = [
                    str_cached!(cache_clone; "John Doe"),
                    str_cached!(cache_clone; "Paul Simon"),
                ];
                if p.out_type() != ValueType::String {
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
            Box::new(|_p| true),
            3,
        );

        for _ in 1..=5 {
            let res = synthesizer.run_iteration(&cache).await;
            if let Some(p) = res {
                assert_eq!(p.get_code(), "(x.name + \" \") + x.surname");
                return;
            }
        }

        assert!(false, "Failed to find program")
    }
}
