use std::{collections::HashMap, sync::Arc};

use boa_engine::{
    context::intrinsics::StandardConstructor, js_string, object::PROTOTYPE, value::JsValue, JsArgs,
    JsNativeError, JsObject, JsResult,
};
use ruse_object_graph::{class_name, value::ObjectValue, ClassName, FieldsMap, ObjectType};

use crate::{
    engine_context::{EngineContext, RuseJsGlobalObject},
    js_errors::*,
    js_object_value::JsObjectValue,
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
    pub fn new(desc: Arc<TsClassDescription>, engine_ctx: &mut EngineContext) -> JsResult<Self> {
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
        let args_str = if method.has_rest_param {
            let (last, rest) = method.param_names.split_last().unwrap();
            if rest.is_empty() {
                format!("...{}", last)
            } else {
                format!("{}, ...{}", &rest.join(", "), last,)
            }
        } else {
            method.param_names.join(", ")
        };
        let js_source_str = format!(
            "function __func({}) {{{}}}\n__func",
            args_str, &method.body_code,
        );
        let js_source = boa_engine::Source::from_bytes(&js_source_str);
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
        _args: &[boa_engine::JsValue],
        engine_ctx: &mut EngineContext,
    ) -> boa_engine::JsResult<ObjectValue> {
        let global_obj = engine_ctx.global_object();
        let global_ctx = RuseJsGlobalObject::from_object(&global_obj)?;

        let id_gen = global_ctx.id_gen()?;

        let graph_id = global_ctx.get_graph_id_for_new_object()?;
        let node_id = id_gen.get_id_for_node();

        let graphs_map = global_ctx.mut_graphs_map()?;

        graphs_map.construct_node(
            graph_id,
            node_id,
            ObjectType::Class(self.desc.class_name.clone()),
            FieldsMap::default(),
        );

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
        engine_ctx: &mut EngineContext,
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
        engine_ctx: &mut EngineContext,
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
        engine_ctx: &mut EngineContext,
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
        engine_ctx: &mut EngineContext,
    ) -> JsResult<StandardConstructor> {
        let mut builder =
            boa_engine::object::ConstructorBuilder::new(engine_ctx, jsfn_wrap!(Self::construct));
        builder
            .name(desc.class_name.as_str())
            .length(desc.constructor.param_types.len())
            .has_prototype_property(true);

        let constructor_body_func = Self::method_js_function(&desc.constructor, builder.context());
        builder.method(
            constructor_body_func,
            js_string!(Self::constructor_body_name()),
            desc.constructor.param_types.len(),
        );

        for field in desc.fields.values() {
            let key = boa_engine::js_string!(field.name.as_str());
            if !field.is_static {
                let getter = Self::getter_for_field(field)
                    .map(|x| x.to_js_function(builder.context().realm()));
                let setter = Self::setter_for_field(field)
                    .map(|x| x.to_js_function(builder.context().realm()));
                let attribute = boa_engine::property::Attribute::WRITABLE
                    | boa_engine::property::Attribute::PERMANENT
                    | boa_engine::property::Attribute::ENUMERABLE;
                builder.accessor(key, getter, setter, attribute);
            } else {
                let getter = Self::getter_for_static_field(&desc.class_name, field)
                    .map(|x| x.to_js_function(builder.context().realm()));
                let setter = Self::setter_for_static_field(&desc.class_name, field)
                    .map(|x| x.to_js_function(builder.context().realm()));
                let attribute = boa_engine::property::Attribute::WRITABLE
                    | boa_engine::property::Attribute::PERMANENT
                    | boa_engine::property::Attribute::ENUMERABLE;
                builder.static_accessor(key, getter, setter, attribute);
            }
        }

        let mut getter_setters = HashMap::new();

        for method in desc.methods.values() {
            let func_name = match method.kind {
                MethodKind::GlobalFunction => unreachable!(),
                MethodKind::Method => js_string!(method.name.as_str()),
                MethodKind::Getter => js_string!(Self::getter_name(method.name.as_str())),
                MethodKind::Setter => js_string!(Self::setter_name(method.name.as_str())),
            };
            let func = Self::method_js_function(method, builder.context());
            if method.is_static {
                builder.static_method(func.clone(), func_name, method.param_types.len());
            } else {
                builder.method(func.clone(), func_name, method.param_types.len());
            }

            if method.kind == MethodKind::Getter || method.kind == MethodKind::Setter {
                let val = getter_setters.entry(method.name.clone()).or_insert((
                    method.is_static,
                    None,
                    None,
                ));

                match method.kind {
                    MethodKind::Getter => {
                        val.1 = Some(func.to_js_function(builder.context().realm()))
                    }
                    MethodKind::Setter => {
                        val.2 = Some(func.to_js_function(builder.context().realm()))
                    }
                    _ => unreachable!(),
                }
            }
        }

        for (key, (is_static, getter, setter)) in getter_setters {
            let attribute = match setter.is_none() {
                true => boa_engine::property::Attribute::READONLY,
                false => boa_engine::property::Attribute::WRITABLE,
            };
            if is_static {
                builder.static_accessor(js_string!(key), getter, setter, attribute);
            } else {
                builder.accessor(js_string!(key), getter, setter, attribute);
            }
        }

        Ok(builder.build())
    }

    pub fn wrap_object(
        &self,
        obj: ObjectValue,
        engine_ctx: &mut EngineContext,
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
