use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::ops::{self, Deref};
use std::sync::Arc;
use std::{any::Any, fmt::Debug};

use crate::context::{Context, SynthesizerContext, SynthesizerWorkerContext, VariableName};

use crate::location::LocValue;
use itertools::Itertools;
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArgTypesList(Vec<ValueType>);

impl ArgTypesList {
    pub fn empty() -> Self {
        Self(vec![])
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<&[ValueType]> for ArgTypesList {
    fn from(arg_types: &[ValueType]) -> Self {
        Self(arg_types.to_vec())
    }
}

impl From<Vec<ValueType>> for ArgTypesList {
    fn from(arg_types: Vec<ValueType>) -> Self {
        Self(arg_types)
    }
}

impl Ord for ArgTypesList {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.len().cmp(&other.0.len()).then(self.0.cmp(&other.0))
    }
}

impl PartialOrd for ArgTypesList {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl ops::Deref for ArgTypesList {
    type Target = [ValueType];

    #[inline]
    fn deref(&self) -> &[ValueType] {
        &self.0
    }
}

impl std::fmt::Display for ArgTypesList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}]", self.0.iter().map(|x| x.to_string()).join(", "))
    }
}

pub type OpcodesList = Vec<Arc<dyn ExprOpcode>>;

#[derive(Default, Debug)]
pub struct OpcodesMap(BTreeMap<ArgTypesList, OpcodesList>);

impl OpcodesMap {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn to_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        for (k, v) in self.0.iter() {
            let list = serde_json::Value::Array(v.iter().map(|x| x.op_name().into()).collect());
            map.insert(k.to_string(), list);
        }
        map.into()
    }

    pub fn init_opcodes(&self) -> impl Iterator<Item = &Arc<dyn ExprOpcode>> {
        self.0[&ArgTypesList::empty()].iter()
    }

    pub fn composite_opcodes(&self) -> impl Iterator<Item = (&ArgTypesList, &OpcodesList)> {
        self.0.iter().filter(|(arg_types, _)| !arg_types.is_empty())
    }
}

pub fn sort_opcodes(opcodes: OpcodesList) -> OpcodesMap {
    let mut sorted_opcodes: OpcodesMap = OpcodesMap::default();
    for op in opcodes {
        if let Some(ops_list) = sorted_opcodes.0.get_mut(&op.arg_types().into()) {
            ops_list.push(op);
        } else {
            sorted_opcodes.0.insert(op.arg_types().into(), vec![op]);
        }
    }

    for (_, ops_list) in sorted_opcodes.0.iter_mut() {
        ops_list.sort_by(|x, y| x.op_name().cmp(y.op_name()));
    }

    sorted_opcodes
}
