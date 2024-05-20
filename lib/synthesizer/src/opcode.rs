use std::{any::Any, fmt::Debug, sync::Arc};

use crate::context::Context;

use super::value::{LocValue, ValueType};
use ruse_object_graph::Cache;

pub trait ExprAst: Any {
    fn to_string(&self) -> String;
    fn as_any(&self) -> &dyn Any;
}

pub trait ExprOpcode: Debug + Sync + Send {
    fn arg_types(&self) -> &[ValueType];

    // post_ctx contains the post context of the last argument or the pre context if there are no arguments.
    // It can be changed on mutating opcodes.
    // For example: Think about the triplet - {x -> 3} ++x (4, {x -> 4})
    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        cache: &Arc<Cache>,
    ) -> Option<LocValue>;
    fn to_ast(&self, children: &Vec<Box<dyn ExprAst>>) -> Box<dyn ExprAst>;
}
