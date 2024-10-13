use std::sync::Arc;

use ruse_object_graph::{scached, Cache, Number, PrimitiveValue};
use ruse_object_graph::value::*;
use ruse_synthesizer::context::Context;

use crate::ts_class::{TsClasses, TsObjectValue};

pub fn primitive_value_to_js_value(value: &PrimitiveValue) -> boa_engine::JsValue {
    match value {
        PrimitiveValue::Number(n) => boa_engine::JsValue::Rational(n.0),
        PrimitiveValue::Bool(b) => boa_engine::JsValue::Boolean(*b),
        PrimitiveValue::String(s) => {
            boa_engine::JsValue::String(boa_engine::js_string!(s.as_str()))
        }
        PrimitiveValue::Null => boa_engine::JsValue::Null,
    }
}

pub fn object_value_to_js_value(
    classes: &TsClasses,
    value: &ObjectValue,
    boa_ctx: &mut boa_engine::Context,
    context: &Context,
    cache: &Arc<Cache>,
) -> boa_engine::JsValue {
    let class = classes.get_class(&value.obj_type(&context.graphs_map)).unwrap();
    let js_obj = class.generate_js_object(classes, value.clone(), boa_ctx, cache);
    boa_engine::JsValue::Object(js_obj)
}

pub fn value_to_js_value(
    classes: &TsClasses,
    value: &Value,
    boa_ctx: &mut boa_engine::Context,
    context: &Context,
    cache: &Arc<Cache>,
) -> boa_engine::JsValue {
    match value {
        Value::Primitive(p) => primitive_value_to_js_value(p),
        Value::Object(o) => object_value_to_js_value(classes, o, boa_ctx, context, cache),
    }
}

pub fn js_object_to_value(
    _classes: &TsClasses,
    value: &boa_engine::JsObject,
    _boa_ctx: &mut boa_engine::Context,
    _cache: &Arc<Cache>,
) -> Value {
    match value.downcast_ref::<TsObjectValue>() {
        Some(ts_obj) => {
            let obj_val = &*ts_obj;
            Value::Object((**obj_val).clone())
        },
        None => unimplemented!(),
    }
}

pub fn js_value_to_value(
    classes: &TsClasses,
    value: &boa_engine::JsValue,
    boa_ctx: &mut boa_engine::Context,
    cache: &Arc<Cache>,
) -> Value {
    match value {
        boa_engine::JsValue::Null => Value::Primitive(PrimitiveValue::Null),
        boa_engine::JsValue::Undefined => todo!(),
        boa_engine::JsValue::Boolean(b) => Value::Primitive(PrimitiveValue::Bool(*b)),
        boa_engine::JsValue::String(s) => Value::Primitive(PrimitiveValue::String(
            scached!(cache; s.to_std_string().unwrap()),
        )),
        boa_engine::JsValue::Rational(n) => {
            Value::Primitive(PrimitiveValue::Number(Number::from(*n)))
        }
        boa_engine::JsValue::Integer(n) => {
            Value::Primitive(PrimitiveValue::Number(Number::from(*n)))
        }
        boa_engine::JsValue::BigInt(_) => todo!(),
        boa_engine::JsValue::Object(o) => js_object_to_value(classes, o, boa_ctx, cache),
        boa_engine::JsValue::Symbol(_) => todo!(),
    }
}
