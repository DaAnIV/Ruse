use boa_engine::JsResult;
use ruse_object_graph::{str_cached, value::*, vbool, vnum, vstr};
use ruse_object_graph::{Number, PrimitiveValue};

use crate::engine_context::EngineContext;
use crate::js_errors::*;
use crate::js_wrapped::JsWrapped;
use crate::ts_classes::TsClasses;

pub fn primitive_value_to_js_value(value: &PrimitiveValue) -> JsResult<boa_engine::JsValue> {
    match value {
        PrimitiveValue::Number(n) => Ok(boa_engine::JsValue::new(n.0)),
        PrimitiveValue::Bool(b) => Ok(boa_engine::JsValue::new(*b)),
        PrimitiveValue::String(s) => {
            Ok(boa_engine::JsValue::new(boa_engine::js_string!(s.as_str())))
        }
    }
}

pub fn object_value_to_js_value(
    classes: &TsClasses,
    value: &ObjectValue,
    engine_ctx: &mut EngineContext<'_>,
) -> JsResult<boa_engine::JsValue> {
    if let Some(class) = classes.get_class(&value.obj_type) {
        let js_obj = class.wrap_as_js_object(value.clone(), engine_ctx)?;
        Ok(boa_engine::JsValue::new(js_obj))
    } else {
        Err(js_error_class_not_found(&value.obj_type))
    }
}

pub fn value_to_js_value(
    classes: &TsClasses,
    value: &Value,
    engine_ctx: &mut EngineContext<'_>,
) -> JsResult<boa_engine::JsValue> {
    match value {
        Value::Primitive(p) => primitive_value_to_js_value(p),
        Value::Object(o) => object_value_to_js_value(classes, o, engine_ctx),
        Value::Null => Ok(boa_engine::JsValue::null()),
    }
}

pub fn js_object_to_value(
    classes: &TsClasses,
    value: &boa_engine::JsObject,
    engine_ctx: &mut EngineContext<'_>,
) -> JsResult<Value> {
    if let Ok(wrapped_obj) = JsWrapped::<ObjectValue>::get_from_js_obj(value) {
        return Ok(Value::Object(wrapped_obj.clone()));
    }
    for class in classes.builtin_classes() {
        if class.is_builtin_object(value) {
            return Ok(Value::Object(
                class.get_from_js_obj(classes, value, engine_ctx).unwrap(),
            ));
        }
    }

    Err(js_error_cannot_convert_js_object_to_value(value))
}

pub fn js_value_to_value(
    classes: &TsClasses,
    value: &boa_engine::JsValue,
    engine_ctx: &mut EngineContext<'_>,
) -> JsResult<Value> {
    match value {
        boa_engine::JsValue::Null => Ok(Value::Null),
        boa_engine::JsValue::Undefined => Ok(Value::Null),
        boa_engine::JsValue::Boolean(b) => Ok(vbool!(*b)),
        boa_engine::JsValue::String(s) => Ok(vstr!(s.to_std_string().unwrap())),
        boa_engine::JsValue::Rational(n) => Ok(vnum!((*n).into())),
        boa_engine::JsValue::Integer(n) => Ok(vnum!((*n).into())),
        boa_engine::JsValue::BigInt(_) => todo!(),
        boa_engine::JsValue::Object(o) => js_object_to_value(classes, o, engine_ctx),
        boa_engine::JsValue::Symbol(_) => todo!(),
    }
}

pub fn js_value_to_primitive_value(value: &boa_engine::JsValue) -> JsResult<PrimitiveValue> {
    match value {
        boa_engine::JsValue::Boolean(b) => Ok(PrimitiveValue::Bool(*b)),
        boa_engine::JsValue::String(s) => Ok(PrimitiveValue::String(str_cached!(s
            .to_std_string()
            .unwrap()))),
        boa_engine::JsValue::Rational(n) => Ok(PrimitiveValue::Number(Number::from(*n))),
        boa_engine::JsValue::Integer(n) => Ok(PrimitiveValue::Number(Number::from(*n))),
        _ => Err(js_error_not_primitive_value(value)),
    }
}

pub fn args_to_js_args<'a, I>(
    params: I,
    classes: &TsClasses,
    engine_ctx: &mut EngineContext<'_>,
) -> JsResult<Vec<boa_engine::JsValue>>
where
    I: IntoIterator<Item = &'a Value>,
{
    params
        .into_iter()
        .map(|x| value_to_js_value(classes, x, engine_ctx))
        .collect()
}
