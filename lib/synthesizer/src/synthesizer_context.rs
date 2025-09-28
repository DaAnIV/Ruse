use std::{collections::HashMap, fmt::Display, sync::Arc};

use downcast_rs::{impl_downcast, DowncastSync};

use crate::context::{ContextArray, ContextJsonDisplay, Variable, VariableMap, VariableName};

pub trait SynthesizerContextData: DowncastSync {}
impl_downcast!(sync SynthesizerContextData);

pub struct SynthesizerContext {
    all_variables: Arc<VariableMap>,
    pub start_context: ContextArray,
    pub data: Box<dyn SynthesizerContextData>,
}

pub trait SynthesizerWorkerContextData: DowncastSync {
    fn gc(&self) {}
}
impl_downcast!(sync SynthesizerWorkerContextData);

pub struct EmptySynthesizerData {}
impl SynthesizerContextData for EmptySynthesizerData {}
impl SynthesizerWorkerContextData for EmptySynthesizerData {}

pub struct SynthesizerWorkerContext {
    pub index: usize,
    pub data: Box<dyn SynthesizerWorkerContextData>,
}
impl Default for SynthesizerWorkerContext {
    fn default() -> Self {
        Self {
            index: 0,
            data: Box::new(EmptySynthesizerData {}),
        }
    }
}

impl SynthesizerContext {
    pub fn from_context_array(context_array: ContextArray) -> Self {
        Self::from_context_array_with_data(context_array, Box::new(EmptySynthesizerData {}))
    }
    pub fn from_context_array_with_data(
        context_array: ContextArray,
        data: Box<dyn SynthesizerContextData>,
    ) -> Self {
        Self {
            all_variables: context_array.get_variables(),
            start_context: context_array,
            data,
        }
    }
    pub fn get_variable(&self, name: &VariableName) -> Option<&Variable> {
        self.all_variables.get(name)
    }

    pub fn set_immutable(&mut self, var: &VariableName) {
        let all_variables = Arc::get_mut(&mut self.all_variables).unwrap();
        let var = all_variables.get_mut(var).unwrap();
        var.immutable = true;
    }

    pub fn variables_count(&self) -> usize {
        self.all_variables.len()
    }

    pub fn variables(&self) -> &VariableMap {
        &self.all_variables
    }
}

#[derive(serde::Serialize)]
pub struct SynthesizerContextJsonDisplay {
    all_variables: HashMap<String, String>,
    start_context: Vec<ContextJsonDisplay>,
}

impl SynthesizerContext {
    pub fn json_display(&self) -> impl Display {
        self.json_display_struct()
    }

    pub fn json_display_struct(&self) -> SynthesizerContextJsonDisplay {
        SynthesizerContextJsonDisplay {
            all_variables: self
                .all_variables
                .iter()
                .map(|(k, v)| (k.to_string(), v.value_type.to_string()))
                .collect(),
            start_context: self.start_context.json_display_struct().contexts,
        }
    }
}

impl Display for SynthesizerContextJsonDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = serde_json::to_string_pretty(self).unwrap();
        write!(f, "{}", value)
    }
}
