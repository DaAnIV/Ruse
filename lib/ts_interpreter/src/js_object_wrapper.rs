use std::{collections::HashMap, sync::Arc};

use boa_engine::{js_str, js_string, value::JsValue};
use itertools::Itertools;
use ruse_object_graph::{
    scached, value::ObjectValue, Attributes, Cache, CachedString, FieldName, FieldsMap, ObjectType,
    PrimitiveField,
};

use crate::{
    engine_context::{EngineContext, RuseJsGlobalObject},
    js_value::{js_value_to_primitive_value, js_value_to_value},
    js_wrapped::JsWrapped,
    ts_class::{JsFieldDescription, MethodDescription, MethodKind, TsClassDescription},
};

#[derive(Debug, Clone)]
pub struct JsObjectWrapper {
    desc: Arc<TsClassDescription>,
}

impl JsObjectWrapper {
    pub fn new(desc: Arc<TsClassDescription>) -> Self {
        JsObjectWrapper { desc }
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
                RuseJsGlobalObject::set_field(this, &field_name, &args[0], &mut boa_ctx.into())?;
                Ok(boa_engine::JsValue::undefined())
            })
        })
    }

    fn getter_for_static_field(
        class_name: &CachedString,
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
        class_name: &CachedString,
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
                    &args[0],
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

        let func = js_func.into_object().unwrap();
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
    ) -> boa_engine::JsResult<JsWrapped<ObjectValue>> {
        let global_obj = engine_ctx.global_object();
        let global_ctx = JsWrapped::<RuseJsGlobalObject>::get_from_js_obj(&global_obj)?;

        let cache = global_ctx.cache()?;
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
                    let value = field.get_primitive_value(cache)?;
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
                        let field_name: FieldName = param.name.clone();
                        let value = js_value_to_primitive_value(arg, cache)?;
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
                let obj_val = js_value_to_value(global_ctx.classes()?, arg, engine_ctx, cache);
                graphs_map.set_field(param.name.clone(), graph_id, node_id, &obj_val);
            }
        }

        Ok(JsWrapped(ObjectValue {
            obj_type: ObjectType::Class(self.desc.class_name.clone()),
            graph_id: graph_id,
            node: node_id,
        }))
    }

    pub fn constructor_name() -> &'static str {
        "__constructor__"
    }

    pub fn getter_name(accessor_name: &str) -> String {
        format!("__get_{}", accessor_name)
    }

    pub fn setter_name(accessor_name: &str) -> String {
        format!("__set_{}", accessor_name)
    }

    fn call_constructor_body(
        &self,
        instance: &boa_engine::JsObject,
        args: &[boa_engine::JsValue],
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsResult<()> {
        self.call_method_body(
            Self::constructor_name(),
            &boa_engine::JsValue::new(instance.clone()),
            args,
            engine_ctx,
        )?;

        Ok(())
    }

    pub fn call_method_body(
        &self,
        method_name: &str,
        this: &boa_engine::JsValue,
        args: &[boa_engine::JsValue],
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsResult<JsValue> {
        let func = engine_ctx
            .get_global_dynamic_class(self)
            .unwrap()
            .prototype()
            .get(js_string!(method_name), engine_ctx)?;

        func.as_callable().unwrap().call(this, args, engine_ctx)
    }

    pub fn call_static_method_body(
        &self,
        method_name: &str,
        this: &boa_engine::JsValue,
        args: &[boa_engine::JsValue],
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsResult<JsValue> {
        let func = engine_ctx
            .get_global_dynamic_class(self)
            .unwrap()
            .constructor()
            .get(js_string!(method_name), engine_ctx)?;

        func.as_callable().unwrap().call(this, args, engine_ctx)
    }
}

impl boa_engine::class::DynamicClassBuilder<JsWrapped<ObjectValue>> for JsObjectWrapper {
    fn name(&self) -> &str {
        &self.desc.class_name
    }

    fn length(&self) -> usize {
        self.desc.constructor.params.len()
    }

    fn id(&self) -> u64 {
        self.desc.id
    }

    fn init(&self, builder: &mut boa_engine::class::ClassBuilder<'_>) -> boa_engine::JsResult<()> {
        for field in self.desc.fields.values() {
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
                let getter = Self::getter_for_static_field(&self.desc.class_name, field)
                    .map(|x| x.to_js_function(builder.context().realm()));
                let setter = Self::setter_for_static_field(&self.desc.class_name, field)
                    .map(|x| x.to_js_function(builder.context().realm()));
                let attribute = boa_engine::property::Attribute::WRITABLE
                    | boa_engine::property::Attribute::PERMANENT;
                builder.static_accessor(key, getter, setter, attribute);
            }
        }

        let mut getter_setters = HashMap::new();

        let constructor_func = Self::method_js_function(&self.desc.constructor, builder.context());
        builder.method(
            js_string!(Self::constructor_name()),
            self.desc.constructor.params.len(),
            constructor_func,
        );

        for method in self.desc.methods.values() {
            let func_name = js_string!(method.name.as_str());
            let func = Self::method_js_function(method, builder.context());
            if method.kind == MethodKind::Method {
                if method.is_static {
                    builder.static_method(func_name, method.params.len(), func);
                } else {
                    builder.method(func_name, method.params.len(), func);
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
                            js_string!(Self::getter_name(method.name.as_str())),
                            method.params.len(),
                            func.clone(),
                        );
                        val.0 = Some(func.to_js_function(builder.context().realm()))
                    }
                    MethodKind::Setter => {
                        builder.method(
                            js_string!(Self::setter_name(method.name.as_str())),
                            method.params.len(),
                            func.clone(),
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

        Ok(())
    }

    fn data_constructor(
        &self,
        new_target: &boa_engine::JsValue,
        args: &[boa_engine::JsValue],
        boa_ctx: &mut boa_engine::Context,
    ) -> boa_engine::JsResult<JsWrapped<ObjectValue>> {
        self.create_new_object(new_target, args, &mut boa_ctx.into())
    }

    fn object_constructor(
        &self,
        instance: &boa_engine::JsObject,
        args: &[boa_engine::JsValue],
        boa_ctx: &mut boa_engine::Context,
    ) -> boa_engine::JsResult<()> {
        if args.is_empty() {
            return Ok(());
        }
        self.call_constructor_body(instance, args, &mut boa_ctx.into())
    }
}

pub(crate) struct JsArrayWrapper {}

impl JsArrayWrapper {
    pub fn wrap_object(
        array_obj: &ObjectValue,
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsResult<boa_engine::JsObject> {
        let global_obj = engine_ctx.global_object();
        let global_ctx: boa_engine::gc::GcRef<'_, JsWrapped<RuseJsGlobalObject>> =
            JsWrapped::<RuseJsGlobalObject>::get_from_js_obj(&global_obj)?;
        let graphs_map = global_ctx.graphs_map()?;
        let cache = global_ctx.cache()?;

        let mut builder = boa_engine::object::ObjectInitializer::with_native_data(
            JsWrapped(array_obj.clone()),
            engine_ctx,
        );
        builder.property(
            js_str!("length"),
            array_obj.total_field_count(graphs_map),
            boa_engine::property::Attribute::READONLY,
        );

        for i in 0..array_obj.total_field_count(graphs_map) {
            let getter = Self::getter_for_index(i as u64, cache)
                .map(|x| x.to_js_function(builder.context().realm()));
            let setter = Self::setter_for_index(i as u64, cache)
                .map(|x| x.to_js_function(builder.context().realm()));
            builder.accessor(i, getter, setter, boa_engine::property::Attribute::WRITABLE);
        }

        Ok(builder.build())
    }

    fn getter_for_index(index: u64, cache: &Cache) -> Option<boa_engine::NativeFunction> {
        let field_name = scached!(cache; index.to_string());
        Some(unsafe {
            boa_engine::native_function::NativeFunction::from_closure(move |this, _, boa_ctx| {
                RuseJsGlobalObject::get_field(this, &field_name, &mut boa_ctx.into())
            })
        })
    }

    fn setter_for_index(index: u64, cache: &Cache) -> Option<boa_engine::NativeFunction> {
        let field_name = scached!(cache; index.to_string());
        Some(unsafe {
            boa_engine::native_function::NativeFunction::from_closure(move |this, args, boa_ctx| {
                RuseJsGlobalObject::set_field(this, &field_name, &args[0], &mut boa_ctx.into())?;
                Ok(boa_engine::JsValue::undefined())
            })
        })
    }
}
