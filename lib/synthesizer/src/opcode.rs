use std::collections::BTreeMap;
use std::sync::Arc;
use std::{any::Any, fmt::Debug};

use crate::context::{Context, SynthesizerContext};

use crate::location::LocValue;
use ruse_object_graph::{CachedString, value::ValueType};

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
    fn op_name(&self) -> &str;
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
    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst>;

    fn required_variables(&self) -> &[CachedString] {
        &NO_REQUIRED_VARIABLES
    }

    fn is_terminal(&self) -> bool {
        false
    }
}

pub type OpcodesList = Vec<Arc<dyn ExprOpcode>>;
pub type OpcodesMap = BTreeMap<Vec<ValueType>, Arc<OpcodesList>>;

pub fn sort_opcodes(opcodes: OpcodesList) -> OpcodesMap {
    let mut sorted_opcodes: OpcodesMap = OpcodesMap::default();
    for op in opcodes {
        if let Some(arc_list) = sorted_opcodes.get_mut(op.arg_types()) {
            let list = Arc::get_mut(arc_list).unwrap();
            list.push(op);
        } else {
            sorted_opcodes.insert(op.arg_types().to_vec(), Arc::new(vec![op]));
        }
    }

    for (_, arc_list) in sorted_opcodes.iter_mut() {
        let list = Arc::get_mut(arc_list).unwrap();
        list.sort_by(|x, y| x.op_name().cmp(y.op_name()));
    }

    sorted_opcodes
}
