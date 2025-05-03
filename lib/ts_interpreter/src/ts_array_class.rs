use boa_engine::object::builtins::JsTypedArray;
use ruse_object_graph::{
    str_cached,
    value::{ObjectValue, Value},
    Cache, ClassName, ValueType,
};

use crate::{
    engine_context::EngineContext,
    js_errors::*,
    js_object_wrapper::JsArrayWrapper,
    js_value::js_value_to_value,
    ts_class::{TsBuiltinClass, TsClass},
    ts_classes::TsClasses,
};

#[derive(Debug)]
pub struct BuiltinArrayClass {
    class_name: ClassName,
    id: u64,
}

impl BuiltinArrayClass {
    pub const CLASS_NAME: &'static str = "Array";

    pub(crate) fn new(id: u64, cache: &Cache) -> Self {
        Self {
            class_name: str_cached!(cache; Self::CLASS_NAME),
            id,
        }
    }

    fn get_from_js_array(
        &self,
        classes: &TsClasses,
        js_array: &boa_engine::object::builtins::JsArray,
        engine_ctx: &mut EngineContext<'_>,
        cache: &Cache,
    ) -> Result<ObjectValue, boa_engine::JsError> {
        let arr_len = js_array.length(engine_ctx).unwrap() as i64;
        if arr_len == 0 {
            return engine_ctx.create_array_object(vec![], &ValueType::Null);
        }

        let elements: Vec<Value> = (0..arr_len)
            .map(|i| {
                let js_elem = js_array.at(i, engine_ctx).unwrap();
                let elem = js_value_to_value(classes, &js_elem, engine_ctx, cache);
                elem
            })
            .collect();

        let elem_type = elements[0].val_type();
        engine_ctx.create_array_object(elements, &elem_type)
    }

    fn get_from_js_typed_array(
        &self,
        classes: &TsClasses,
        js_typed_array: &JsTypedArray,
        engine_ctx: &mut EngineContext<'_>,
        cache: &Cache,
    ) -> Result<ObjectValue, boa_engine::JsError> {
        let arr_len = js_typed_array.length(engine_ctx).unwrap() as i64;
        if arr_len == 0 {
            return engine_ctx.create_array_object(vec![], &ValueType::Number);
        }

        let elements: Vec<Value> = (0..arr_len)
            .map(|i| {
                let js_elem = js_typed_array.at(i, engine_ctx).unwrap();
                let elem = js_value_to_value(classes, &js_elem, engine_ctx, cache);
                assert!(elem.number_value().is_some());
                elem
            })
            .collect();

        engine_ctx.create_array_object(elements, &ValueType::Number)
    }
}

impl TsClass for BuiltinArrayClass {
    fn obj_type(&self, template_types: Option<Vec<ValueType>>) -> ValueType {
        let template_types = template_types.unwrap();
        assert!(template_types.len() == 1);
        ValueType::array_value_type(&template_types[0])
    }

    fn wrap_as_js_object(
        &self,
        obj: ObjectValue,
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsObject {
        JsArrayWrapper::wrap_object(&obj, engine_ctx).unwrap()
    }

    fn is_parametrized(&self) -> bool {
        false
    }

    fn get_class_name(&self) -> &ClassName {
        &self.class_name
    }

    fn get_class_id(&self) -> u64 {
        self.id
    }
}

impl TsBuiltinClass for BuiltinArrayClass {
    fn is_builtin_object(&self, value: &boa_engine::JsObject) -> bool {
        value.is_array() || value.is::<boa_engine::builtins::typed_array::TypedArray>()
    }

    fn get_from_js_obj(
        &self,
        classes: &TsClasses,
        value: &boa_engine::JsObject,
        engine_ctx: &mut EngineContext<'_>,
        cache: &Cache,
    ) -> Result<ObjectValue, boa_engine::JsError> {
        if let Ok(js_array) = boa_engine::object::builtins::JsArray::from_object(value.clone()) {
            self.get_from_js_array(classes, &js_array, engine_ctx, cache)
        } else if let Ok(typed_array) =
            boa_engine::object::builtins::JsTypedArray::from_object(value.clone())
        {
            self.get_from_js_typed_array(classes, &typed_array, engine_ctx, cache)
        } else {
            Err(js_error_not_builtin_array())
        }
    }
}
