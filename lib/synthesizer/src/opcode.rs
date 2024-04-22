use std::fmt::Debug;

use crate::context::Context;

use super::value::{LocValue, ValueType};
use ruse_object_graph::Cache;

pub trait ExprAst {
    fn to_string(&self) -> String;
}

pub trait SynthesizerExprOpcode<T>: Debug
where T: ExprAst {
    fn arg_types(&self) -> &[ValueType];

    // ctx is an in-out value. It should contain the pre context but eval can change it 
    // For example: Think about the triplet - {x -> 3} ++x (4, {x -> 4})
    fn eval(&self, ctx: &mut Context, args: &[&LocValue], cache: &Cache) -> LocValue;
    fn to_ast(&self, children: &Vec<T>) -> T;
}
