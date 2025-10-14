use std::sync::Arc;

use ruse_object_graph::ValueType;
use ruse_synthesizer::{
    context::{ContextArray, ContextSubsetResult},
    prog::SubProgram,
    value_array::ValueArray,
};
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone)]
pub(crate) struct SubProgramKey(Arc<SubProgram>);

impl SubProgramKey {
    fn out_type(&self) -> &ValueType {
        self.0.out_type()
    }
    fn out_value(&self) -> &ValueArray {
        self.0.out_value()
    }
    fn effect(&self) -> Option<&ContextArray> {
        if self.0.num_mutations() > 0 {
            Some(self.0.post_ctx())
        } else {
            None
        }
    }
}

impl From<Arc<SubProgram>> for SubProgramKey {
    fn from(value: Arc<SubProgram>) -> Self {
        Self(value)
    }
}

impl Eq for SubProgramKey {}

impl PartialEq for SubProgramKey {
    fn eq(&self, other: &Self) -> bool {
        self.out_type() == other.out_type()
            && self
                .out_value()
                .eq(self.0.post_ctx(), other.out_value(), other.0.post_ctx())
            && self.effect() == other.effect()
    }
}

impl Hash for SubProgramKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.out_type().hash(state);
        self.out_value().wrap(self.0.post_ctx()).hash(state);
        self.effect().hash(state);
    }
}

pub(crate) fn subsumption_partial_cmp(a: &SubProgram, b: &SubProgram) -> Option<std::cmp::Ordering> {
    match a.pre_ctx().subset(b.pre_ctx()) {
        ContextSubsetResult::Subset => Some(std::cmp::Ordering::Less),
        ContextSubsetResult::Equal => Some(a.size().cmp(&b.size())),
        ContextSubsetResult::NotSubset => None,
    }
}
