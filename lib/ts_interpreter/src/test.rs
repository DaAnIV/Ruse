#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use ruse_synthesizer::opcode::SynthesizerExprOpcode;
    use ruse_synthesizer::value::ValueType;
    use swc_ecma_ast as ast;

    use crate::opcode::*;
    use ruse_synthesizer::*;
    use ruse_object_graph::{str_cached, Cache};
    use ruse_object_graph::Number;

    #[test]
    fn add_numbers() {
        let mut cache = Cache::new();
        let context = context::Context::new(Default::default());
        let evaluator = BinOp {
            op: ast::BinaryOp::Add,
            arg_types: [ValueType::Number, ValueType::Number]
        };
        let (_, out) = evaluator.eval(
            &context,
            &vec![&temp_val!(vnum!(Number::from(3u64))), &temp_val!(vnum!(Number::from(4u64)))],
            &mut cache,
        );
        assert_eq!(out.val, vnum!(Number::from(7u64)));
    }

    #[test]
    fn add_strings() {
        let mut cache = Cache::new();
        let context = context::Context::new(Default::default());
        let evaluator = BinOp {
            op: ast::BinaryOp::Add,
            arg_types: [ValueType::String, ValueType::String]
        };
        let (_, out) = evaluator.eval(
            &context,
            &vec![&temp_val!(vstr!(cache; "a")), &temp_val!(vstr!(cache; "b"))],
            &mut cache,
        );
        assert_eq!(out.val, vstr!(cache; "ab"));
    }

    #[test]
    fn ident() {
        let mut cache = Cache::new();
        let context = context::Context::new(HashMap::from([
            (str_cached!(cache; "x"), vnum!(Number::from(7u64)))
        ]));
        let evaluator = IdentOp {
            name: str_cached!(cache; "x")
        };
        let (_, out) = evaluator.eval(
            &context,
            &vec!(),
            &mut cache,
        );
        assert_eq!(out.val, vnum!(Number::from(7u64)));
    }

    #[test]
    fn prefix_plus_plus() {
        let mut cache = Cache::new();
        let context = context::Context::new(HashMap::from([
            (str_cached!(cache; "x"), vnum!(Number::from(7u64)))
        ]));
        let id = IdentOp {
            name: str_cached!(cache; "x")
        };
        let op = UpdateOp {
            op: ast::UpdateOp::PlusPlus,
            prefix: true
        };
        let (context, x_val) = id.eval(
            &context,
            &vec!(),
            &mut cache,
        );
        let (new_context, out) = op.eval(
            &context,
            &vec![&x_val],
            &mut cache,
        );
        assert_eq!(context.get_var_value(&id.name), vnum!(Number::from(7u64)));
        assert_eq!(out.val, vnum!(Number::from(8u64)));
        assert_eq!(new_context.get_var_value(&id.name), vnum!(Number::from(8u64)));
    }

    #[test]
    fn postfix_plus_plus() {
        let mut cache = Cache::new();
        let context = context::Context::new(HashMap::from([
            (str_cached!(cache; "x"), vnum!(Number::from(7u64)))
        ]));
        let id = IdentOp {
            name: str_cached!(cache; "x")
        };
        let op = UpdateOp {
            op: ast::UpdateOp::PlusPlus,
            prefix: false
        };
        let (context, x_val) = id.eval(
            &context,
            &vec!(),
            &mut cache,
        );
        let (new_context, out) = op.eval(
            &context,
            &vec![&x_val],
            &mut cache,
        );
        assert_eq!(context.get_var_value(&id.name), vnum!(Number::from(7u64)));
        assert_eq!(out.val, vnum!(Number::from(7u64)));
        assert_eq!(new_context.get_var_value(&id.name), vnum!(Number::from(8u64)));
    }
}
