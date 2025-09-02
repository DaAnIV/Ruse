use std::collections::BTreeMap;
use std::ops::Deref;
use std::sync::Arc;
use std::{any::Any, fmt::Debug};

use crate::context::{Context, SynthesizerContext, SynthesizerWorkerContext, VariableName};

use crate::location::LocValue;
use ruse_object_graph::ValueType;

pub trait ExprAst: Any {
    fn to_string(&self) -> String;
    fn as_any(&self) -> &dyn Any;
}

#[derive(Debug, Clone)]
pub struct EvalOutput {
    pub output: LocValue,
    pub dirty: bool,
}

impl Deref for EvalOutput {
    type Target = LocValue;

    fn deref(&self) -> &Self::Target {
        &self.output
    }
}

pub type EvalResult = Result<EvalOutput, ()>;

#[macro_export]
macro_rules! dirty {
    ($out:expr) => {
        Ok($crate::opcode::EvalOutput {
            output: $out,
            dirty: true,
        })
    };
}
#[macro_export]
macro_rules! pure {
    ($out:expr) => {
        Ok($crate::opcode::EvalOutput {
            output: $out,
            dirty: false,
        })
    };
}

const NO_REQUIRED_VARIABLES: [VariableName; 0] = [];

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
        worker_ctx: &mut SynthesizerWorkerContext,
    ) -> EvalResult;
    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst>;

    fn required_variables(&self) -> &[VariableName] {
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
