#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use ruse_synthesizer::opcode::SynthesizerExprOpcode;
    use ruse_synthesizer::value::{Location, ValueType, VarLoc};
    use swc_ecma_ast as ast;

    use crate::opcode::*;
    use ruse_synthesizer::*;
    use ruse_object_graph::{str_cached, Cache};
    use ruse_object_graph::Number;

    #[test]
    fn add_numbers() {
        let cache = Cache::new();
        let mut ctx = context::Context::with_values(Default::default());
        let evaluator = BinOp {
            op: ast::BinaryOp::Add,
            arg_types: [ValueType::Number, ValueType::Number]
        };
        
        let args = [&ctx.temp_value(vnum!(Number::from(3u64))), &ctx.temp_value(vnum!(Number::from(4u64)))];
        let out = evaluator.eval(
            &mut ctx,
            &args,
            &cache,
        );
        assert_eq!(out.val(), &vnum!(Number::from(7u64)));
    }

    #[test]
    fn add_strings() {
        let cache = Cache::new();
        let mut ctx = context::Context::with_values(Default::default());
        let evaluator = BinOp {
            op: ast::BinaryOp::Add,
            arg_types: [ValueType::String, ValueType::String]
        };

        let args = [&ctx.temp_value(vstr!(cache; "a")), &ctx.temp_value(vstr!(cache; "b"))];

        let out = evaluator.eval(
            &mut ctx,
            &args,
            &cache,
        );
        assert_eq!(out.val(), &vstr!(cache; "ab"));
    }

    #[test]
    fn ident() {
        let cache = Cache::new();
        let mut ctx = context::Context::with_values(HashMap::from([
            (str_cached!(cache; "x"), vnum!(Number::from(7u64)))
        ]));
        let evaluator = IdentOp {
            name: str_cached!(cache; "x")
        };
        let out = evaluator.eval(
            &mut ctx,
            &[],
            &cache,
        );
        assert_eq!(out.val(), &vnum!(Number::from(7u64)));
    }

    #[test]
    fn prefix_plus_plus() {
        let cache = Cache::new();
        let mut ctx = context::Context::with_values(HashMap::from([
            (str_cached!(cache; "x"), vnum!(Number::from(7u64)))
        ]));
        let id = IdentOp {
            name: str_cached!(cache; "x")
        };
        let op = UpdateOp {
            op: ast::UpdateOp::PlusPlus,
            prefix: true
        };
        let x_val = id.eval(
            &mut ctx,
            &[],
            &cache,
        );

        let mut new_ctx = ctx.clone();
        let out = op.eval(
            &mut new_ctx,
            &[&x_val],
            &cache,
        );
        assert_eq!(ctx.get_var_loc_value(&id.name).val(), &vnum!(Number::from(7u64)));
        assert_eq!(x_val.val(), &vnum!(Number::from(7u64)));
        assert_eq!(x_val.loc(), &Location::Var(VarLoc { var: id.name.clone() }));
        assert_eq!(out.val(), &vnum!(Number::from(8u64)));
        assert_eq!(out.loc(), &Location::Temp);
        assert_eq!(new_ctx.get_var_loc_value(&id.name).val(), &vnum!(Number::from(8u64)));
    }

    #[test]
    fn postfix_plus_plus() {
        let cache = Cache::new();
        let mut ctx = context::Context::with_values(HashMap::from([
            (str_cached!(cache; "x"), vnum!(Number::from(7u64)))
        ]));
        let id = IdentOp {
            name: str_cached!(cache; "x")
        };
        let op = UpdateOp {
            op: ast::UpdateOp::PlusPlus,
            prefix: false
        };
        let x_val = id.eval(
            &mut ctx,
            &[],
            &cache,
        );

        let mut new_ctx = ctx.clone();
        let out = op.eval(
            &mut new_ctx,
            &[&x_val],
            &cache,
        );
        assert_eq!(ctx.get_var_loc_value(&id.name).val(), &vnum!(Number::from(7u64)));
        assert_eq!(x_val.val(), &vnum!(Number::from(7u64)));
        assert_eq!(x_val.loc(), &Location::Var(VarLoc { var: id.name.clone() }));
        assert_eq!(out.val(), &vnum!(Number::from(7u64)));
        assert_eq!(out.loc(), &Location::Temp);
        assert_eq!(new_ctx.get_var_loc_value(&id.name).val(), &vnum!(Number::from(8u64)));
    }
}
