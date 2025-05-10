use ruse_object_graph::ObjectType;

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

pub(crate) fn js_error_no_global_static_graph() -> boa_engine::JsError {
    boa_engine::JsNativeError::typ()
        .with_message("Global class has not static graph")
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

pub(crate) fn js_error_not_class_value(class_name: &str) -> boa_engine::JsError {
    boa_engine::JsNativeError::typ()
        .with_message(format!("Not a user class {} value", class_name))
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

pub(crate) fn js_error_not_builtin_map() -> boa_engine::JsError {
    boa_engine::JsNativeError::typ()
        .with_message("Not a builtin map")
        .into()
}

pub(crate) fn js_error_this_is_not_obj_map() -> boa_engine::JsError {
    boa_engine::JsNativeError::typ()
        .with_message("'this' is not a wrapped Map")
        .into()
}

pub(crate) fn js_error_class_not_found(obj_type: &ObjectType) -> boa_engine::JsError {
    boa_engine::JsNativeError::typ()
        .with_message(format!("Class {} not found", obj_type))
        .into()
}

pub(crate) fn js_error_global_class_not_found() -> boa_engine::JsError {
    boa_engine::JsNativeError::typ()
        .with_message("Global class not found")
        .into()
}

pub(crate) fn js_error_user_class_not_found(class_name: &str) -> boa_engine::JsError {
    boa_engine::JsNativeError::typ()
        .with_message(format!("User Class {} not found", class_name))
        .into()
}

pub(crate) fn js_error_cannot_convert_js_object_to_value(
    value: &boa_engine::JsObject,
) -> boa_engine::JsError {
    boa_engine::JsNativeError::typ()
        .with_message(format!("Cannot convert JS object to value. {:?}", value))
        .into()
}

pub(crate) fn js_error_not_primitive_value(value: &boa_engine::JsValue) -> boa_engine::JsError {
    boa_engine::JsNativeError::typ()
        .with_message(format!("Not a primitive value. {:?}", value))
        .into()
}

pub(crate) fn js_error_global_field_not_found(field_name: &str) -> boa_engine::JsError {
    boa_engine::JsNativeError::typ()
        .with_message(format!("Global field {} not found", field_name))
        .into()
}
