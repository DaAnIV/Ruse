use std::{any::Any, fmt::Debug};

use crate::context::{Context, SynthesizerContext};

use super::value::{LocValue, ValueType};
use ruse_object_graph::CachedString;

pub trait ExprAst: Any {
    fn to_string(&self) -> String;
    fn as_any(&self) -> &dyn Any;
}

#[derive(Debug, Clone)]
pub enum EvalResult {
    None,
    DirtyContext(LocValue),
    NoModification(LocValue),
}

impl EvalResult {
    const fn unwrap_failed() -> ! {
        panic!("called `EvalResult::unwrap()` on a `None` value")
    }
    
    pub fn unwrap(self) -> LocValue {
        match self {
            EvalResult::None => EvalResult::unwrap_failed(),
            EvalResult::DirtyContext(val) => val,
            EvalResult::NoModification(val) => val,
        }
    }
}

impl From<Option<LocValue>> for EvalResult {
    fn from(value: Option<LocValue>) -> Self {
        match value {
            Some(v) => Self::NoModification(v),
            None => Self::None,
        }
    }
}

const NO_REQUIRED_VARIABLES: [CachedString; 0] = [];

pub trait ExprOpcode: Debug + Sync + Send {
    fn arg_types(&self) -> &[ValueType];

    // post_ctx contains the post context of the last argument or the pre context if there are no arguments.
    // It can be changed on mutating opcodes.
    // For example: Think about the triplet - {x -> 3} ++x (4, {x -> 4})
    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        syn_ctx: &SynthesizerContext,
    ) -> EvalResult;
    fn to_ast(&self, children: &Vec<Box<dyn ExprAst>>) -> Box<dyn ExprAst>;

    fn required_variables(&self) -> &[CachedString] {
        return &NO_REQUIRED_VARIABLES;
    }
}
