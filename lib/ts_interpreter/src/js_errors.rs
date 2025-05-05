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

pub(crate) fn js_error_not_array_value() -> boa_engine::JsError {
    boa_engine::JsNativeError::typ()
        .with_message("Not a array value")
        .into()
}

pub(crate) fn js_error_not_builtin_array() -> boa_engine::JsError {
    boa_engine::JsNativeError::typ()
        .with_message("Not a builtin array")
        .into()
}

pub(crate) fn js_error_unexpected_key_type() -> boa_engine::JsError {
    boa_engine::JsNativeError::typ()
        .with_message("Key unexpected type")
        .into()
}

pub(crate) fn js_error_this_is_not_obj_iterator() -> boa_engine::JsError {
    boa_engine::JsNativeError::typ()
        .with_message("`this` is not an JsObjectIterator")
        .into()
}

pub(crate) fn js_error_missing_arg(index: usize) -> boa_engine::JsError {
    boa_engine::JsNativeError::typ()
        .with_message(format!("arg index {} is missing", index))
        .into()
}

pub(crate) fn js_error_unexpected_arg_type(index: usize) -> boa_engine::JsError {
    boa_engine::JsNativeError::typ()
        .with_message(format!("arg index {} type is unexpected", index))
        .into()
}
