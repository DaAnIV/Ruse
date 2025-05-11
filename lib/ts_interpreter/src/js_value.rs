use boa_engine::{JsResult, JsValue};
use ruse_object_graph::{str_cached, value::*, vbool, vnum, vstr};
use ruse_object_graph::{Number, PrimitiveValue};

use crate::engine_context::{EngineContext, RuseJsGlobalObject};
use crate::js_errors::*;
use crate::js_object_value::JsObjectValue;

pub trait TryIntoJs: Sized {
    /// This function tries to convert a `Self` into [`JsValue`].
    fn try_into_js(&self, context: &mut EngineContext) -> JsResult<JsValue>;
}

pub trait TryFromJs: Sized {
    fn try_from_js(value: &JsValue, context: &mut EngineContext) -> JsResult<Self>;
}

impl TryIntoJs for ObjectValue {
    fn try_into_js(&self, context: &mut EngineContext) -> JsResult<JsValue> {
        let global_obj = context.global_object();
        let global_ctx = RuseJsGlobalObject::from_object(&global_obj)?;
        let classes = global_ctx.classes()?;

        if let Some(class) = classes.get_class(&self.obj_type) {
            let js_obj = class.wrap_as_js_object(self.clone(), context)?;
            Ok(boa_engine::JsValue::new(js_obj))
        } else {
            Err(js_error_class_not_found(&self.obj_type))
        }
    }
}

impl TryFromJs for ObjectValue {
    fn try_from_js(value: &JsValue, context: &mut EngineContext) -> JsResult<Self> {
        let js_obj = value.as_object().ok_or_else(|| js_error_not_js_object())?;
        if let Some(js_obj_value) = js_obj.downcast_ref::<JsObjectValue>() {
            return Ok(js_obj_value.clone());
        }

        let global_obj = context.global_object();
        let global_ctx = RuseJsGlobalObject::from_object(&global_obj)?;
        let classes = global_ctx.classes()?;

        for class in classes.builtin_classes() {
            if class.is_builtin_object(js_obj) {
                return Ok(class.get_from_js_obj(js_obj, context)?);
            }
        }

        Err(js_error_cannot_convert_js_object_to_value(js_obj))
    }
}

impl TryIntoJs for PrimitiveValue {
    fn try_into_js(&self, _context: &mut EngineContext) -> JsResult<JsValue> {
        match self {
            PrimitiveValue::Number(n) => Ok(boa_engine::JsValue::new(n.0)),
            PrimitiveValue::Bool(b) => Ok(boa_engine::JsValue::new(*b)),
            PrimitiveValue::String(s) => {
                Ok(boa_engine::JsValue::new(boa_engine::js_string!(s.as_str())))
            }
        }
    }
}

impl TryFromJs for PrimitiveValue {
    fn try_from_js(value: &JsValue, _context: &mut EngineContext) -> JsResult<Self> {
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
}

impl TryIntoJs for Value {
    fn try_into_js(&self, context: &mut EngineContext) -> JsResult<JsValue> {
        match self {
            Value::Primitive(p) => p.try_into_js(context),
            Value::Object(o) => o.try_into_js(context),
            Value::Null => Ok(boa_engine::JsValue::null()),
        }
    }
}

impl TryFromJs for Value {
    fn try_from_js(value: &JsValue, context: &mut EngineContext) -> JsResult<Self> {
        match value {
            boa_engine::JsValue::Null => Ok(Value::Null),
            boa_engine::JsValue::Undefined => Ok(Value::Null),
            boa_engine::JsValue::Boolean(b) => Ok(vbool!(*b)),
            boa_engine::JsValue::String(s) => Ok(vstr!(s.to_std_string().unwrap())),
            boa_engine::JsValue::Rational(n) => Ok(vnum!((*n).into())),
            boa_engine::JsValue::Integer(n) => Ok(vnum!((*n).into())),
            boa_engine::JsValue::BigInt(_) => todo!(),
            boa_engine::JsValue::Object(_) => {
                ObjectValue::try_from_js(value, context).map(Value::Object)
            }
            boa_engine::JsValue::Symbol(_) => todo!(),
        }
    }
}

pub(crate) fn args_to_js_args<'a, I>(
    args: I,
    engine_ctx: &mut EngineContext,
) -> JsResult<Vec<JsValue>>
where
    I: IntoIterator<Item = &'a Value>,
{
    args.into_iter()
        .map(|arg| arg.try_into_js(engine_ctx))
        .collect()
}
