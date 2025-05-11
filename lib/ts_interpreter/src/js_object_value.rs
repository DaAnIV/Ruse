use std::ops::{Deref, DerefMut};

use boa_engine::JsData;
use boa_gc::{Finalize, Trace};
use ruse_object_graph::value::ObjectValue;

#[derive(Debug, Trace, Finalize, JsData)]
pub(crate) struct JsObjectValue(#[unsafe_ignore_trace] pub ObjectValue);

impl From<ObjectValue> for JsObjectValue {
    fn from(value: ObjectValue) -> Self {
        Self(value)
    }
}

impl AsRef<ObjectValue> for JsObjectValue {
    fn as_ref(&self) -> &ObjectValue {
        &self.0
    }
}

impl Deref for JsObjectValue {
    type Target = ObjectValue;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for JsObjectValue {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Into<ObjectValue> for JsObjectValue {
    fn into(self) -> ObjectValue {
        self.0.clone()
    }
}
