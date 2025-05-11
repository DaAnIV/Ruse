use std::{collections::HashMap, sync::Arc};

use boa_engine::{
    context::intrinsics::StandardConstructor, js_string, object::PROTOTYPE, value::JsValue, JsArgs,
    JsNativeError, JsObject, JsResult,
};
use itertools::Itertools;
use ruse_object_graph::{
    class_name, field_name,
    value::{ObjectValue, Value},
    Attributes, ClassName, FieldName, FieldsMap, ObjectType, PrimitiveField, PrimitiveValue,
};

use crate::{
    engine_context::{EngineContext, RuseJsGlobalObject},
    js_errors::*,
    js_object_value::JsObjectValue,
    js_value::TryFromJs,
    ts_class::{JsFieldDescription, MethodDescription, MethodKind, TsClassDescription},
};

#[macro_export]
macro_rules! jsfn_wrap {
    ($fn_ptr:expr) => {
        unsafe {
            boa_engine::native_function::NativeFunction::from_closure(move |this, args, boa_ctx| {
                $fn_ptr(this, args, &mut boa_ctx.into())
            })
        }
    };
}

#[derive(Debug, Clone)]
pub struct JsUserClassWrapper {
    desc: Arc<TsClassDescription>,
    constructor: StandardConstructor,
}

impl JsUserClassWrapper {
    pub fn new(
        desc: Arc<TsClassDescription>,
        engine_ctx: &mut EngineContext<'_>,
    ) -> JsResult<Self> {
        let constructor = Self::build_standard_constructor(&desc, engine_ctx)?;
        Ok(JsUserClassWrapper { desc, constructor })
    }

    pub fn constructor(&self) -> JsObject {
        self.constructor.constructor()
    }

    pub fn prototype(&self) -> JsObject {
        self.constructor.prototype()
    }

    pub fn name(&self) -> &ClassName {
        &self.desc.class_name
    }

    fn getter_for_field(field: &JsFieldDescription) -> Option<boa_engine::NativeFunction> {
        debug_assert!(!field.is_static);

        let field_name = field.name.clone();
        Some(unsafe {
            boa_engine::native_function::NativeFunction::from_closure(move |this, _, boa_ctx| {
                RuseJsGlobalObject::get_field(this, &field_name, &mut boa_ctx.into())
            })
        })
    }

    fn setter_for_field(field: &JsFieldDescription) -> Option<boa_engine::NativeFunction> {
        debug_assert!(!field.is_static);

        let field_name = field.name.clone();
        Some(unsafe {
            boa_engine::native_function::NativeFunction::from_closure(move |this, args, boa_ctx| {
                RuseJsGlobalObject::set_field(
                    this,
                    &field_name,
                    args.get_or_undefined(0),
                    &mut boa_ctx.into(),
                )?;
                Ok(boa_engine::JsValue::undefined())
            })
        })
    }

    fn getter_for_static_field(
        class_name: &ClassName,
        field: &JsFieldDescription,
    ) -> Option<boa_engine::NativeFunction> {
        debug_assert!(field.is_static);

        let class_name = class_name.clone();
        let field_name = field.name.clone();
        Some(unsafe {
            boa_engine::native_function::NativeFunction::from_closure(move |_, _, boa_ctx| {
                RuseJsGlobalObject::get_static_field(&class_name, &field_name, &mut boa_ctx.into())
            })
        })
    }

    fn setter_for_static_field(
        class_name: &ClassName,
        field: &JsFieldDescription,
    ) -> Option<boa_engine::NativeFunction> {
        debug_assert!(field.is_static);

        let class_name = class_name.clone();
        let field_name = field.name.clone();
        Some(unsafe {
            boa_engine::native_function::NativeFunction::from_closure(move |_, args, boa_ctx| {
                RuseJsGlobalObject::set_static_field(
                    &class_name,
                    &field_name,
                    args.get_or_undefined(0),
                    &mut boa_ctx.into(),
                )?;
                Ok(boa_engine::JsValue::undefined())
            })
        })
    }

    pub fn method_js_function(
        method: &MethodDescription,
        boa_ctx: &mut boa_engine::Context,
    ) -> boa_engine::NativeFunction {
        let arg_names = method
            .params
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let code = format!(
            "function func({}) {{{}}}\nfunc",
            arg_names,
            method.body.as_ref().unwrap()
        );
        let js_source = boa_engine::Source::from_bytes(code.as_str());
        let js_func = boa_ctx.eval(js_source).unwrap();

        let func = js_func.to_object(boa_ctx).unwrap();
        assert!(func.is_callable());

        unsafe {
            boa_engine::native_function::NativeFunction::from_closure(move |this, args, boa_ctx| {
                func.call(this, args, boa_ctx)
            })
        }
    }

    fn create_new_object(
        &self,
        _new_target: &boa_engine::JsValue,
        args: &[boa_engine::JsValue],
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsResult<ObjectValue> {
        let global_obj = engine_ctx.global_object();
        let global_ctx = RuseJsGlobalObject::from_object(&global_obj)?;

        let id_gen = global_ctx.id_gen()?;

        let primitive_fields =
            self.desc
                .fields
                .values()
                .filter_map(|field| {
                    if field.is_static || field.is_constructor_prop {
                        return None;
                    }

                    let field_name: FieldName = field.name.clone();
                    let value = field.get_primitive_value()?;
                    let attributes = Attributes {
                        readonly: field.is_readonly,
                    };
                    let primitive_field = PrimitiveField { value, attributes };
                    Some((field_name, primitive_field))
                })
                .chain(self.desc.constructor.params.iter().zip_eq(args).filter_map(
                    |(param, arg)| {
                        if !param.is_prop && !param.value_type.is_primitive() {
                            return None;
                        }

                        let field = &self.desc.fields[param.name.as_str()];
                        let field_name = field_name!(param.name.as_str());
                        let value = PrimitiveValue::try_from_js(&arg, engine_ctx).ok()?;
                        let attributes = Attributes {
                            readonly: field.is_readonly,
                        };
                        let primitive_field = PrimitiveField { value, attributes };
                        Some((field_name, primitive_field))
                    },
                ));

        let graph_id = global_ctx.get_graph_id_for_new_object()?;
        let node_id = id_gen.get_id_for_node();

        let graphs_map = global_ctx.mut_graphs_map()?;

        graphs_map.construct_node(
            graph_id,
            node_id,
            ObjectType::Class(self.desc.class_name.clone()),
            FieldsMap::from_iter(primitive_fields),
        );

        for (param, arg) in self.desc.constructor.params.iter().zip_eq(args) {
            if !param.is_prop || param.value_type.is_primitive() {
                continue;
            }

            if !arg.is_null_or_undefined() {
                let obj_val = Value::try_from_js(arg, engine_ctx)?;
                graphs_map.set_field(field_name!(param.name.clone()), graph_id, node_id, &obj_val);
            }
        }

        Ok(ObjectValue {
            obj_type: ObjectType::Class(self.desc.class_name.clone()),
            graph_id: graph_id,
            node: node_id,
        })
    }

    fn constructor_body_name() -> &'static str {
        "__constructor_body__"
    }

    pub fn getter_name(accessor_name: &str) -> String {
        format!("__get_{}", accessor_name)
    }

    pub fn setter_name(accessor_name: &str) -> String {
        format!("__set_{}", accessor_name)
    }

    fn call_constructor_body(
        class_name: &ClassName,
        instance: &boa_engine::JsValue,
        args: &[boa_engine::JsValue],
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsResult<JsValue> {
        Self::call_method_body(
            class_name,
            Self::constructor_body_name(),
            instance,
            args,
            engine_ctx,
        )
    }

    pub fn call_method_body(
        class_name: &ClassName,
        method_name: &str,
        this: &boa_engine::JsValue,
        args: &[boa_engine::JsValue],
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsResult<JsValue> {
        let prototype = {
            let global_obj = engine_ctx.global_object();
            let global_ctx = RuseJsGlobalObject::from_object(&global_obj)?;
            global_ctx.get_user_class(class_name)?.prototype()
        };
        let func = prototype.get(js_string!(method_name), engine_ctx)?;

        func.as_callable().unwrap().call(this, args, engine_ctx)
    }

    pub fn call_static_method_body(
        class_name: &ClassName,
        method_name: &str,
        args: &[boa_engine::JsValue],
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsResult<JsValue> {
        let constructor = {
            let global_obj = engine_ctx.global_object();
            let global_ctx = RuseJsGlobalObject::from_object(&global_obj)?;
            global_ctx.get_user_class(class_name)?.constructor()
        };
        let func = constructor.get(js_string!(method_name), engine_ctx)?;

        func.as_callable()
            .unwrap()
            .call(&constructor.into(), args, engine_ctx)
    }
}

impl JsUserClassWrapper {
    fn build_standard_constructor(
        desc: &TsClassDescription,
        engine_ctx: &mut EngineContext<'_>,
    ) -> JsResult<StandardConstructor> {
        let mut builder =
            boa_engine::object::ConstructorBuilder::new(engine_ctx, jsfn_wrap!(Self::construct));
        builder
            .name(desc.class_name.as_str())
            .length(desc.constructor.params.len())
            .has_prototype_property(true);

        let constructor_body_func = Self::method_js_function(&desc.constructor, builder.context());
        builder.method(
            constructor_body_func,
            js_string!(Self::constructor_body_name()),
            desc.constructor.params.len(),
        );

        for field in desc.fields.values() {
            let key = boa_engine::js_string!(field.name.as_str());
            if !field.is_static {
                let getter = Self::getter_for_field(field)
                    .map(|x| x.to_js_function(builder.context().realm()));
                let setter = Self::setter_for_field(field)
                    .map(|x| x.to_js_function(builder.context().realm()));
                let attribute = boa_engine::property::Attribute::WRITABLE
                    | boa_engine::property::Attribute::PERMANENT;
                builder.accessor(key, getter, setter, attribute);
            } else {
                let getter = Self::getter_for_static_field(&desc.class_name, field)
                    .map(|x| x.to_js_function(builder.context().realm()));
                let setter = Self::setter_for_static_field(&desc.class_name, field)
                    .map(|x| x.to_js_function(builder.context().realm()));
                let attribute = boa_engine::property::Attribute::WRITABLE
                    | boa_engine::property::Attribute::PERMANENT;
                builder.static_accessor(key, getter, setter, attribute);
            }
        }

        let mut getter_setters = HashMap::new();

        for method in desc.methods.values() {
            let func_name = js_string!(method.name.as_str());
            let func = Self::method_js_function(method, builder.context());
            if method.kind == MethodKind::Method {
                if method.is_static {
                    builder.static_method(func, func_name, method.params.len());
                } else {
                    builder.method(func, func_name, method.params.len());
                }
            } else {
                let val = match getter_setters.entry(func_name) {
                    std::collections::hash_map::Entry::Occupied(occupied_entry) => {
                        occupied_entry.into_mut()
                    }
                    std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                        vacant_entry.insert((None, None))
                    }
                };

                match method.kind {
                    MethodKind::Getter => {
                        builder.method(
                            func.clone(),
                            js_string!(Self::getter_name(method.name.as_str())),
                            method.params.len(),
                        );
                        val.0 = Some(func.to_js_function(builder.context().realm()))
                    }
                    MethodKind::Setter => {
                        builder.method(
                            func.clone(),
                            js_string!(Self::setter_name(method.name.as_str())),
                            method.params.len(),
                        );
                        val.1 = Some(func.to_js_function(builder.context().realm()))
                    }
                    _ => unreachable!(),
                }
            }
        }

        for (key, (getter, setter)) in getter_setters {
            let attribute = match setter.is_none() {
                true => boa_engine::property::Attribute::READONLY,
                false => boa_engine::property::Attribute::WRITABLE,
            };
            builder.accessor(key, getter, setter, attribute);
        }

        Ok(builder.build())
    }

    pub fn wrap_object(
        &self,
        obj: ObjectValue,
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsResult<boa_engine::JsObject> {
        if !obj.is_class(&self.desc.class_name) {
            return Err(js_error_not_class_value(&self.desc.class_name));
        }

        let proto = self.constructor.prototype();

        let mut builder = boa_engine::object::ObjectInitializer::with_native_data_and_proto(
            JsObjectValue(obj),
            proto,
            engine_ctx,
        );

        Ok(builder.build())
    }
}

impl JsUserClassWrapper {
    fn construct(
        new_target: &JsValue,
        args: &[JsValue],
        context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        if new_target.is_undefined() {
            return Err(JsNativeError::typ()
                .with_message(format!("cannot call constructor of user class without new"))
                .into());
        }
        let (name, js_obj) = {
            let global_obj = context.global_object();
            let global_ctx = RuseJsGlobalObject::from_object(&global_obj)?;

            let constructor = new_target.as_object().unwrap();
            let prototype = constructor
                .get(PROTOTYPE, context)?
                .as_object()
                .unwrap()
                .clone();
            let name = constructor
                .get(js_string!("name"), context)?
                .to_string(context)?;
            let obj = global_ctx.get_user_class(&class_name!(name.to_std_string().unwrap()))?;

            let data = obj.create_new_object(new_target, args, context)?;

            let mut builder = boa_engine::object::ObjectInitializer::with_native_data_and_proto(
                JsObjectValue(data),
                prototype,
                context,
            );
            (obj.desc.class_name.clone(), builder.build().into())
        };

        Self::call_constructor_body(&name, &js_obj, args, context)?;

        Ok(js_obj)
    }

    pub fn call_constructor(
        class_name: &ClassName,
        args: &[JsValue],
        context: &mut EngineContext,
    ) -> JsResult<JsValue> {
        let constructor = {
            let global_obj = context.global_object();
            let global_ctx = RuseJsGlobalObject::from_object(&global_obj)?;
            global_ctx.get_user_class(class_name)?.constructor()
        }
        .into();

        Self::construct(&constructor, args, context)
    }
}
