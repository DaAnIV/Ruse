use crate::context::Context;

use super::value::{LocValue, ValueType};
use ruse_object_graph::Cache;

pub trait ExprAst {
    fn to_string(&self) -> String;
}

pub trait SynthesizerExprOpcode<T>
where T: ExprAst {
    fn arg_types(&self) -> &[ValueType];

    fn eval(&self, ctx: &Context, args: &[&LocValue], cache: &mut Cache) -> (Context, LocValue);
    fn to_ast(&self, children: &Vec<T>) -> T;
}
