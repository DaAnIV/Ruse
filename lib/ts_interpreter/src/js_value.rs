use ruse_object_graph::value::*;
use ruse_object_graph::{scached, Cache, GraphsMap, Number, PrimitiveValue};

use crate::js_object_wrapper::{EngineContext, JsWrapped};
use crate::ts_class::TsClasses;

pub fn primitive_value_to_js_value(value: &PrimitiveValue) -> boa_engine::JsValue {
    match value {
        PrimitiveValue::Number(n) => boa_engine::JsValue::rational(n.0),
        PrimitiveValue::Bool(b) => boa_engine::JsValue::new(*b),
        PrimitiveValue::String(s) => boa_engine::JsValue::new(boa_engine::js_string!(s.as_str())),
    }
}

pub fn object_value_to_js_value(
    classes: &TsClasses,
    value: &ObjectValue,
    engine_ctx: &mut EngineContext<'_>,
) -> boa_engine::JsValue {
    let class = classes.get_class(&value.obj_type).unwrap();
    let js_obj = class.wrap_as_js_object(value.clone(), engine_ctx);
    boa_engine::JsValue::new(js_obj)
}

pub fn value_to_js_value(
    classes: &TsClasses,
    value: &Value,
    engine_ctx: &mut EngineContext<'_>,
    graphs_map: &GraphsMap,
) -> boa_engine::JsValue {
    match value {
        Value::Primitive(p) => primitive_value_to_js_value(p),
        Value::Object(o) => object_value_to_js_value(classes, o, engine_ctx),
        Value::Null => boa_engine::JsValue::null(),
    }
}

pub fn js_object_to_value(
    _classes: &TsClasses,
    value: &boa_engine::JsObject,
    _engine_ctx: &mut EngineContext<'_>,
    _cache: &Cache,
) -> Value {
    let wrapped_obj = JsWrapped::<ObjectValue>::get_from_js_obj(value).unwrap();
    Value::Object(wrapped_obj.clone())
}

pub fn js_value_to_value(
    classes: &TsClasses,
    value: &boa_engine::JsValue,
    engine_ctx: &mut EngineContext<'_>,
    cache: &Cache,
) -> Value {
    match value.variant() {
        boa_engine::JsVariant::Null => Value::Null,
        boa_engine::JsVariant::Undefined => Value::Null,
        boa_engine::JsVariant::Boolean(b) => Value::Primitive(PrimitiveValue::Bool(b)),
        boa_engine::JsVariant::String(s) => Value::Primitive(PrimitiveValue::String(
            scached!(cache; s.to_std_string().unwrap()),
        )),
        boa_engine::JsVariant::Float64(n) => {
            Value::Primitive(PrimitiveValue::Number(Number::from(n)))
        }
        boa_engine::JsVariant::Integer32(n) => {
            Value::Primitive(PrimitiveValue::Number(Number::from(n)))
        }
        boa_engine::JsVariant::BigInt(_) => todo!(),
        boa_engine::JsVariant::Object(o) => js_object_to_value(classes, o, engine_ctx, cache),
        boa_engine::JsVariant::Symbol(_) => todo!(),
    }
}

pub fn js_value_to_primitive_value(
    value: &boa_engine::JsValue,
    cache: &Cache,
) -> Option<PrimitiveValue> {
    match value.variant() {
        boa_engine::JsVariant::Null => None,
        boa_engine::JsVariant::Undefined => None,
        boa_engine::JsVariant::Boolean(b) => Some(PrimitiveValue::Bool(b)),
        boa_engine::JsVariant::String(s) => Some(PrimitiveValue::String(
            scached!(cache; s.to_std_string().unwrap()),
        )),
        boa_engine::JsVariant::Float64(n) => Some(PrimitiveValue::Number(Number::from(n))),
        boa_engine::JsVariant::Integer32(n) => Some(PrimitiveValue::Number(Number::from(n))),
        boa_engine::JsVariant::BigInt(_) => todo!(),
        boa_engine::JsVariant::Object(_) => None,
        boa_engine::JsVariant::Symbol(_) => None,
    }
}
