pub(crate) fn js_error_null_this() -> boa_engine::JsError {
    boa_engine::JsNativeError::typ()
        .with_message("this can't be null when calling class method")
        .into()
}

pub(crate) fn js_error_not_initialized() -> boa_engine::JsError {
    boa_engine::JsNativeError::typ()
        .with_message("Global object is not initialized")
        .into()
}

pub(crate) fn js_error_immutable_context() -> boa_engine::JsError {
    boa_engine::JsNativeError::typ()
        .with_message(format!("Context is not mutable"))
        .into()
}

pub(crate) fn js_error_not_js_wrapped() -> boa_engine::JsError {
    boa_engine::JsNativeError::typ()
        .with_message(format!("JsObject is not of type JsWrapped"))
        .into()
}

pub(crate) fn js_error_not_js_object() -> boa_engine::JsError {
    boa_engine::JsNativeError::typ()
        .with_message(format!("JsValue is not of type JsObject"))
        .into()
}

pub(crate) fn js_error_no_static_graph(class_name: &str) -> boa_engine::JsError {
    boa_engine::JsNativeError::typ()
        .with_message(format!("Class {} has not static graph", class_name))
        .into()
}
