use std::collections::HashMap;

use boa_engine::{
    js_string, property::Attribute, string::StaticJsStrings, JsResult, JsSymbol, JsValue,
};
use ruse_object_graph::{
    class_name, field_name,
    value::{ObjectValue, Value},
    ClassName, FieldName, ObjectType, PrimitiveValue, ValueType,
};

use crate::{
    engine_context::{EngineContext, RuseJsGlobalObject},
    js_errors::*,
    js_object_value::JsObjectValue,
    js_value::{TryFromJs, TryIntoJs},
    jsfn_wrap,
    ts_class::{BuiltinClassWrapper, TsBuiltinClass, TsClass},
};

use super::jsiterator::{JsObjectIterator, JsObjectIteratorKind};

#[derive(Debug)]
pub struct BuiltinMapClass {
    class_name: ClassName,
    id: u64,
}

impl BuiltinMapClass {
    pub const CLASS_NAME: &'static str = "Map";

    pub(crate) fn new(id: u64) -> Self {
        Self {
            class_name: class_name!(Self::CLASS_NAME),
            id,
        }
    }
}

impl TsClass for BuiltinMapClass {
    fn obj_type(&self, template_types: Option<Vec<ValueType>>) -> ValueType {
        let template_types = template_types.unwrap();
        assert!(template_types.len() == 2);
        ValueType::map_value_type(&template_types[0], &template_types[1])
    }

    fn wrap_as_js_object(
        &self,
        obj: ObjectValue,
        engine_ctx: &mut EngineContext,
    ) -> JsResult<boa_engine::JsObject> {
        JsMapWrapper::wrap_object(&obj, engine_ctx)
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

impl TsBuiltinClass for BuiltinMapClass {
    fn is_builtin_object(&self, value: &boa_engine::JsObject) -> bool {
        value.is::<boa_engine::builtins::map::ordered_map::OrderedMap<JsValue>>()
    }

    fn get_from_js_obj(
        &self,
        value: &boa_engine::JsObject,
        engine_ctx: &mut EngineContext,
    ) -> JsResult<ObjectValue> {
        if let Ok(js_map) = boa_engine::object::builtins::JsMap::from_object(value.clone()) {
            if js_map.get_size(engine_ctx)? == 0.into() {
                return engine_ctx.create_map_object(
                    vec![],
                    &ValueType::String,
                    &ValueType::Number,
                );
            }
            let mut key_type = ValueType::String;
            let mut value_type = ValueType::Number;

            let mut entries = HashMap::new();
            let js_keys = js_map.keys(engine_ctx)?;
            while let Ok(js_key) = js_keys.next(engine_ctx) {
                let js_value = js_map.get(js_key.clone(), engine_ctx)?;

                let key = PrimitiveValue::try_from_js(&js_key, engine_ctx)?;
                let value = Value::try_from_js(&js_value, engine_ctx)?;

                key_type = key.val_type();
                value_type = value.val_type();

                entries.insert(key, value);
            }
            engine_ctx.create_map_object(entries, &key_type, &value_type)
        } else {
            Err(js_error_not_builtin_map())
        }
    }
}

pub(crate) struct JsMapWrapper {}

impl JsMapWrapper {
    fn getter_for_field_name(field_name: FieldName) -> Option<boa_engine::NativeFunction> {
        Some(unsafe {
            boa_engine::native_function::NativeFunction::from_closure(move |this, _, boa_ctx| {
                RuseJsGlobalObject::get_field(this, &field_name, &mut boa_ctx.into())
            })
        })
    }

    fn setter_for_field_name(field_name: FieldName) -> Option<boa_engine::NativeFunction> {
        Some(unsafe {
            boa_engine::native_function::NativeFunction::from_closure(move |this, args, boa_ctx| {
                RuseJsGlobalObject::set_field(this, &field_name, &args[0], &mut boa_ctx.into())?;
                Ok(boa_engine::JsValue::undefined())
            })
        })
    }
}

impl BuiltinClassWrapper for JsMapWrapper {
    fn wrap_object(
        map_obj: &ObjectValue,
        engine_ctx: &mut EngineContext,
    ) -> boa_engine::JsResult<boa_engine::JsObject> {
        assert!(map_obj.obj_type.is_map_obj_type());

        let global_obj = engine_ctx.global_object();
        let global_ctx = RuseJsGlobalObject::from_object(&global_obj)?;

        let proto = global_ctx.constructors().map_prototype(engine_ctx)?;

        let mut builder = boa_engine::object::ObjectInitializer::with_native_data_and_proto(
            JsObjectValue(map_obj.clone()),
            proto,
            engine_ctx,
        );
        let (_key_type, value_type) = match &map_obj.obj_type {
            ObjectType::Map(key_type, value_type) => (key_type.as_ref(), value_type.as_ref()),
            _ => unreachable!(),
        };

        let graphs_map = global_ctx.graphs_map()?;

        if value_type.is_primitive() {
            for (key, _) in map_obj.fields(graphs_map) {
                let getter = Self::getter_for_field_name(key.clone())
                    .map(|x| x.to_js_function(builder.context().realm()));
                let setter = Self::setter_for_field_name(key.clone())
                    .map(|x| x.to_js_function(builder.context().realm()));
                builder.accessor(
                    js_string!(key.as_str()),
                    getter,
                    setter,
                    boa_engine::property::Attribute::WRITABLE,
                );
            }
        } else {
            for (key, _) in map_obj.neighbors(graphs_map) {
                let getter = Self::getter_for_field_name(key.clone())
                    .map(|x| x.to_js_function(builder.context().realm()));
                let setter = Self::setter_for_field_name(key.clone())
                    .map(|x| x.to_js_function(builder.context().realm()));
                builder.accessor(
                    js_string!(key.as_str()),
                    getter,
                    setter,
                    boa_engine::property::Attribute::WRITABLE,
                );
            }
        }

        Ok(builder.build())
    }

    fn build_standard_constructor(
        engine_ctx: &mut EngineContext,
    ) -> JsResult<boa_engine::context::intrinsics::StandardConstructor> {
        let get_size = boa_engine::object::FunctionObjectBuilder::new(
            engine_ctx.realm(),
            jsfn_wrap!(Self::get_size),
        )
        .name(js_string!("get size"))
        .build();

        let entries_function = boa_engine::object::FunctionObjectBuilder::new(
            engine_ctx.realm(),
            jsfn_wrap!(Self::entries),
        )
        .name(js_string!("entries"))
        .build();
        let mut builder =
            boa_engine::object::ConstructorBuilder::new(engine_ctx, jsfn_wrap!(Self::constructor));
        builder
            .name("Map")
            .property(
                js_string!("entries"),
                entries_function.clone(),
                Attribute::WRITABLE | Attribute::NON_ENUMERABLE | Attribute::CONFIGURABLE,
            )
            .property(
                JsSymbol::iterator(),
                entries_function,
                Attribute::WRITABLE | Attribute::NON_ENUMERABLE | Attribute::CONFIGURABLE,
            )
            .property(
                JsSymbol::to_string_tag(),
                StaticJsStrings::MAP,
                Attribute::READONLY | Attribute::NON_ENUMERABLE | Attribute::CONFIGURABLE,
            )
            .method(jsfn_wrap!(Self::clear), js_string!("clear"), 0)
            .method(jsfn_wrap!(Self::delete), js_string!("delete"), 1)
            .method(jsfn_wrap!(Self::for_each), js_string!("forEach"), 1)
            .method(jsfn_wrap!(Self::get), js_string!("get"), 1)
            .method(jsfn_wrap!(Self::has), js_string!("has"), 1)
            .method(jsfn_wrap!(Self::keys), js_string!("keys"), 0)
            .method(jsfn_wrap!(Self::set), js_string!("set"), 2)
            .method(jsfn_wrap!(Self::values), js_string!("values"), 0)
            .accessor(
                js_string!("size"),
                Some(get_size),
                None,
                Attribute::CONFIGURABLE,
            );

        Ok(builder.build())
    }
}

impl JsMapWrapper {
    pub(crate) fn constructor(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn clear(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn delete(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn for_each(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn get(
        this: &JsValue,
        args: &[JsValue],
        ctx: &mut EngineContext,
    ) -> JsResult<JsValue> {
        let obj = ObjectValue::try_from_js(this, ctx)?;
        let global_obj = ctx.global_object();
        let global_ctx = RuseJsGlobalObject::from_object(&global_obj)?;
        let graphs_map = global_ctx.graphs_map()?;

        if let ObjectType::Map(key_type, _value_type) = &obj.obj_type {
            let js_key = args.get(0).ok_or_else(|| js_error_missing_arg(0))?;
            let key = PrimitiveValue::try_from_js(js_key, ctx)?;
            if &key.val_type() != key_type.as_ref() {
                return Err(js_error_unexpected_arg_type(0));
            }
            let field_name = field_name!(key.to_string());
            if let Some(value) = obj.get_field_value(&field_name, graphs_map) {
                value.try_into_js(ctx)
            } else {
                Ok(JsValue::undefined())
            }
        } else {
            Err(js_error_this_is_not_obj_map())
        }
    }

    pub(crate) fn has(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn keys(
        this: &JsValue,
        _: &[JsValue],
        ctx: &mut EngineContext,
    ) -> JsResult<JsValue> {
        let obj = ObjectValue::try_from_js(this, ctx)?;
        if let ObjectType::Map(key_type, _value_type) = &obj.obj_type {
            JsObjectIterator::create_object_iterator(
                obj.clone(),
                JsObjectIteratorKind::Field,
                key_type.as_ref().clone(),
                ctx,
            )
        } else {
            Err(js_error_this_is_not_obj_map())
        }
    }

    pub(crate) fn set(
        _this: &JsValue,
        _args: &[JsValue],
        _context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        todo!()
    }

    pub(crate) fn values(
        this: &JsValue,
        _: &[JsValue],
        ctx: &mut EngineContext,
    ) -> JsResult<JsValue> {
        let obj = ObjectValue::try_from_js(this, ctx)?;
        if let ObjectType::Map(key_type, _value_type) = &obj.obj_type {
            JsObjectIterator::create_object_iterator(
                obj.clone(),
                JsObjectIteratorKind::Value,
                key_type.as_ref().clone(),
                ctx,
            )
        } else {
            Err(js_error_this_is_not_obj_map())
        }
    }

    pub(crate) fn get_size(
        this: &JsValue,
        _args: &[JsValue],
        engine_ctx: &mut EngineContext,
    ) -> JsResult<JsValue> {
        let global_obj = engine_ctx.global_object();
        let global_ctx = RuseJsGlobalObject::from_object(&global_obj)?;
        let graphs_map = global_ctx.graphs_map()?;

        let obj = ObjectValue::try_from_js(this, engine_ctx)?;
        Ok(obj.total_field_count(graphs_map).into())
    }

    pub(crate) fn entries(
        this: &JsValue,
        _: &[JsValue],
        ctx: &mut EngineContext,
    ) -> JsResult<JsValue> {
        let obj = ObjectValue::try_from_js(this, ctx)?;
        if let ObjectType::Map(key_type, _value_type) = &obj.obj_type {
            JsObjectIterator::create_object_iterator(
                obj.clone(),
                JsObjectIteratorKind::FieldValue,
                key_type.as_ref().clone(),
                ctx,
            )
        } else {
            Err(js_error_this_is_not_obj_map())
        }
    }
}
