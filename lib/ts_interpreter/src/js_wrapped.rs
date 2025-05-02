use std::ops::{Deref, DerefMut};

use boa_engine::JsError;
use ruse_object_graph::value::ObjectValue;

use crate::js_errors::{js_error_not_js_object, js_error_not_js_wrapped};

pub(crate) struct JsWrapped<T: 'static>(pub T);

impl<T> boa_engine::gc::Finalize for JsWrapped<T> {}
unsafe impl<T> boa_engine::gc::Trace for JsWrapped<T> {
    boa_engine::gc::empty_trace!();
}
impl<T> boa_engine::JsData for JsWrapped<T> {}

impl boa_engine::class::DynamicClassData for JsWrapped<ObjectValue> {}

impl<T> JsWrapped<T> {
    pub fn get_from_js_obj(
        js_obj: &boa_engine::JsObject,
    ) -> boa_engine::JsResult<boa_engine::gc::GcRef<'_, Self>> {
        js_obj
            .downcast_ref::<Self>()
            .ok_or(js_error_not_js_wrapped())
    }

    pub fn mut_from_js_obj(
        js_obj: &boa_engine::JsObject,
    ) -> Result<boa_engine::gc::GcRefMut<'_, boa_engine::object::ErasedObject, Self>, JsError> {
        js_obj
            .downcast_mut::<Self>()
            .ok_or(js_error_not_js_wrapped())
    }

    pub fn get_from_js_val(
        js_val: &boa_engine::JsValue,
    ) -> boa_engine::JsResult<boa_engine::gc::GcRef<'_, Self>> {
        let js_obj = js_val.as_object().ok_or(js_error_not_js_object())?;
        Self::get_from_js_obj(js_obj)
    }
}

impl<T> From<T> for JsWrapped<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T> AsRef<T> for JsWrapped<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T> Deref for JsWrapped<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for JsWrapped<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
