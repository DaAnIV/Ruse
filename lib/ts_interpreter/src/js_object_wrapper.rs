use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    sync::Arc,
};

use boa_engine::{js_str, js_string, JsError, JsResult, JsValue};
use itertools::Itertools;
use ruse_object_graph::{
    value::{ObjectValue, Value},
    vnull, Attributes, Cache, CachedString, FieldName, FieldsMap, GraphIndex, GraphsMap,
    PrimitiveField,
};
use ruse_synthesizer::context::{Context, GraphIdGenerator};

use crate::{
    js_value::{js_value_to_primitive_value, js_value_to_value, value_to_js_value},
    ts_class::{JsFieldDescription, MethodDescription, MethodKind, TsClassDescription, TsClasses},
};

pub fn error_null_this() -> JsError {
    boa_engine::JsNativeError::typ()
        .with_message("this can't be null when calling class method")
        .into()
}

fn error_not_initialized() -> JsError {
    boa_engine::JsNativeError::typ()
        .with_message("Global object is not initialized")
        .into()
}

fn error_no_static_graph(class_name: &str) -> JsError {
    boa_engine::JsNativeError::typ()
        .with_message(format!("Class {} has not static graph", class_name))
        .into()
}

fn error_immutable_context() -> JsError {
    boa_engine::JsNativeError::typ()
        .with_message(format!("Context is not mutable"))
        .into()
}

pub(crate) struct JsWrapped<T: 'static>(T);

impl<T> boa_engine::gc::Finalize for JsWrapped<T> {}
unsafe impl<T> boa_engine::gc::Trace for JsWrapped<T> {
    boa_engine::gc::empty_trace!();
}
impl<T> boa_engine::JsData for JsWrapped<T> {}

impl boa_engine::class::DynamicClassData for JsWrapped<ObjectValue> {}

impl<T> JsWrapped<T> {
    pub fn get_from_js_obj(
        js_obj: &boa_engine::JsObject,
    ) -> boa_engine::JsResult<boa_engine::gc::GcRef<'_, Self>> {
        js_obj
            .downcast_ref::<Self>()
            .ok_or(boa_engine::JsError::from_opaque(
                js_str!("not the expected type!").into(),
            ))
    }

    pub fn mut_from_js_obj(
        js_obj: &boa_engine::JsObject,
    ) -> Result<boa_engine::gc::GcRefMut<'_, boa_engine::object::ErasedObject, Self>, JsError> {
        js_obj
            .downcast_mut::<Self>()
            .ok_or(boa_engine::JsError::from_opaque(
                js_str!("not the expected type!").into(),
            ))
    }

    pub fn get_from_js_val(
        js_val: &boa_engine::JsValue,
    ) -> boa_engine::JsResult<boa_engine::gc::GcRef<'_, Self>> {
        let js_obj = js_val.as_object().ok_or(boa_engine::JsError::from_opaque(
            js_str!("not an object!").into(),
        ))?;
        Self::get_from_js_obj(js_obj)
    }
}

impl<T> From<T> for JsWrapped<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T> AsRef<T> for JsWrapped<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T> Deref for JsWrapped<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for JsWrapped<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

struct EngineContextHooks;

impl boa_engine::context::HostHooks for EngineContextHooks {
    fn create_global_object(
        &self,
        intrinsics: &boa_engine::context::intrinsics::Intrinsics,
    ) -> boa_engine::prelude::JsObject {
        boa_engine::JsObject::from_proto_and_data(
            intrinsics.constructors().object().prototype(),
            JsWrapped(RuseJsGlobalObject::default()),
        )
    }
}

trait GlobalObjectInner {
    fn set_field(
        &mut self,
        obj_val: &ObjectValue,
        field_name: &CachedString,
        new_value: &Value,
    ) -> boa_engine::JsResult<()>;

    fn classes(&self) -> &TsClasses;
    fn cache(&self) -> &Cache;
    fn graphs_map(&self) -> &GraphsMap;
    fn mut_graphs_map(&self) -> boa_engine::JsResult<&mut GraphsMap>;
    fn get_graph_id_for_new_object(&self) -> boa_engine::JsResult<GraphIndex>;
    fn id_gen(&self) -> &GraphIdGenerator;
    fn dirty(&self) -> bool;
    fn set_dirty(&mut self);

    fn js_value_to_value(
        &self,
        js_val: &boa_engine::JsValue,
        engine_ctx: &mut EngineContext<'_>,
    ) -> Value {
        js_value_to_value(self.classes(), js_val, engine_ctx, self.cache())
    }
    fn value_to_js_value(
        &self,
        value: &Value,
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsValue {
        value_to_js_value(self.classes(), value, engine_ctx)
    }
}

struct GraphGlobalObjectInner {
    cache: Arc<Cache>,
    id_gen: Arc<GraphIdGenerator>,
    graph_id: GraphIndex,
    graphs_map: *mut GraphsMap,
    classes: *const TsClasses,
    dirty: bool,
}

impl GlobalObjectInner for GraphGlobalObjectInner {
    fn set_field(
        &mut self,
        obj_val: &ObjectValue,
        field_name: &CachedString,
        new_value: &Value,
    ) -> boa_engine::JsResult<()> {
        self.mut_graphs_map()?.set_field(
            field_name.clone(),
            obj_val.graph_id,
            obj_val.node,
            new_value,
        );

        Ok(())
    }

    fn classes(&self) -> &TsClasses {
        unsafe { self.classes.as_ref().unwrap() }
    }
    fn cache(&self) -> &Cache {
        self.cache.as_ref()
    }
    fn graphs_map(&self) -> &GraphsMap {
        unsafe { self.graphs_map.as_ref().unwrap() }
    }
    fn mut_graphs_map(&self) -> boa_engine::JsResult<&mut GraphsMap> {
        Ok(unsafe { self.graphs_map.as_mut().unwrap() })
    }
    fn get_graph_id_for_new_object(&self) -> boa_engine::JsResult<GraphIndex> {
        Ok(self.graph_id)
    }
    fn id_gen(&self) -> &GraphIdGenerator {
        self.id_gen.as_ref()
    }

    fn dirty(&self) -> bool {
        self.dirty
    }
    fn set_dirty(&mut self) {
        self.dirty = true;
    }
}

struct MutContextGlobalObjectInner {
    cache: Arc<Cache>,
    context: *mut Context,
    classes: *const TsClasses,
    dirty: bool,
}

impl MutContextGlobalObjectInner {
    fn context(&self) -> &mut Context {
        unsafe { self.context.as_mut().unwrap() }
    }
}

impl GlobalObjectInner for MutContextGlobalObjectInner {
    fn set_field(
        &mut self,
        obj_val: &ObjectValue,
        field_name: &CachedString,
        new_value: &Value,
    ) -> boa_engine::JsResult<()> {
        self.context().set_field(
            obj_val.graph_id,
            obj_val.node,
            field_name.clone(),
            &new_value,
        );

        Ok(())
    }

    fn classes(&self) -> &TsClasses {
        unsafe { self.classes.as_ref().unwrap() }
    }
    fn cache(&self) -> &Cache {
        self.cache.as_ref()
    }
    fn graphs_map(&self) -> &GraphsMap {
        &self.context().graphs_map
    }
    fn mut_graphs_map(&self) -> boa_engine::JsResult<&mut GraphsMap> {
        Ok(Arc::make_mut(&mut self.context().graphs_map))
    }
    fn get_graph_id_for_new_object(&self) -> boa_engine::JsResult<GraphIndex> {
        Ok(self.context().create_graph_in_map())
    }
    fn id_gen(&self) -> &GraphIdGenerator {
        &self.context().graph_id_gen
    }

    fn dirty(&self) -> bool {
        self.dirty
    }
    fn set_dirty(&mut self) {
        self.dirty = true;
    }
}

struct ContextGlobalObjectInner {
    cache: Arc<Cache>,
    context: *const Context,
    classes: *const TsClasses,
}

impl ContextGlobalObjectInner {
    fn context(&self) -> &Context {
        unsafe { self.context.as_ref().unwrap() }
    }
}

impl GlobalObjectInner for ContextGlobalObjectInner {
    fn set_field(
        &mut self,
        _obj_val: &ObjectValue,
        _field_name: &CachedString,
        _new_value: &Value,
    ) -> boa_engine::JsResult<()> {
        Err(error_immutable_context())
    }

    fn classes(&self) -> &TsClasses {
        unsafe { self.classes.as_ref().unwrap() }
    }
    fn cache(&self) -> &Cache {
        self.cache.as_ref()
    }
    fn graphs_map(&self) -> &GraphsMap {
        &self.context().graphs_map
    }
    fn mut_graphs_map(&self) -> boa_engine::JsResult<&mut GraphsMap> {
        Err(error_immutable_context())
    }
    fn get_graph_id_for_new_object(&self) -> boa_engine::JsResult<GraphIndex> {
        Err(error_immutable_context())
    }
    fn id_gen(&self) -> &GraphIdGenerator {
        &self.context().graph_id_gen
    }

    fn dirty(&self) -> bool {
        false
    }
    fn set_dirty(&mut self) {}
}

enum RuseJsGlobalObject {
    None,
    Graph(GraphGlobalObjectInner),
    Contxt(ContextGlobalObjectInner),
    MutContxt(MutContextGlobalObjectInner),
}

impl RuseJsGlobalObject {
    fn inner(&self) -> JsResult<&dyn GlobalObjectInner> {
        match self {
            RuseJsGlobalObject::None => Err(error_not_initialized()),
            RuseJsGlobalObject::Graph(graph_inner) => Ok(graph_inner),
            RuseJsGlobalObject::Contxt(context_inner) => Ok(context_inner),
            RuseJsGlobalObject::MutContxt(context_inner) => Ok(context_inner),
        }
    }

    fn inner_mut(&mut self) -> JsResult<&mut dyn GlobalObjectInner> {
        match self {
            RuseJsGlobalObject::None => Err(error_not_initialized()),
            RuseJsGlobalObject::Graph(graph_inner) => Ok(graph_inner),
            RuseJsGlobalObject::Contxt(context_inner) => Ok(context_inner),
            RuseJsGlobalObject::MutContxt(context_inner) => Ok(context_inner),
        }
    }

    fn get_field(
        wrapped_obj: &boa_engine::JsValue,
        field_name: &CachedString,
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsResult<boa_engine::JsValue> {
        let obj_val = JsWrapped::<ObjectValue>::get_from_js_val(wrapped_obj)?;
        let global_obj = engine_ctx.global_object();
        let wrapped_inner: boa_engine::gc::GcRef<'_, JsWrapped<RuseJsGlobalObject>> =
            JsWrapped::<Self>::get_from_js_obj(&global_obj)?;
        let inner = wrapped_inner.inner()?;

        let field = obj_val
            .get_field_value(field_name, inner.graphs_map())
            .unwrap_or(vnull!());
        Ok(inner.value_to_js_value(&field, engine_ctx))
    }

    fn set_field(
        wrapped_obj: &boa_engine::JsValue,
        field_name: &CachedString,
        new_js_value: &boa_engine::JsValue,
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsResult<()> {
        let obj_val = JsWrapped::<ObjectValue>::get_from_js_val(wrapped_obj)?;
        let global_obj = engine_ctx.global_object();
        let mut wrapped_inner = JsWrapped::<Self>::mut_from_js_obj(&global_obj)?;
        let inner = wrapped_inner.inner_mut()?;

        let new_value = inner.js_value_to_value(&new_js_value, engine_ctx);

        inner.set_field(&obj_val, field_name, &new_value)?;
        inner.set_dirty();

        Ok(())
    }

    fn get_static_field(
        class_name: &CachedString,
        field_name: &CachedString,
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsResult<boa_engine::JsValue> {
        let global_obj = engine_ctx.global_object();
        let wrapped_inner = JsWrapped::<Self>::get_from_js_obj(&global_obj)?;
        let inner = wrapped_inner.inner()?;
        let classes = inner.classes();

        let class = classes.get_class(class_name).unwrap();

        let field = &class.static_fields[field_name];

        Ok(inner.value_to_js_value(&field.val, engine_ctx))
    }

    fn set_static_field(
        class_name: &CachedString,
        field_name: &CachedString,
        new_js_value: &boa_engine::JsValue,
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsResult<()> {
        let global_obj = engine_ctx.global_object();
        let mut wrapped_inner = JsWrapped::<Self>::mut_from_js_obj(&global_obj)?;
        let inner = wrapped_inner.inner_mut()?;
        let classes = inner.classes();

        let class = classes.get_class(class_name).unwrap();
        let new_value = inner.js_value_to_value(&new_js_value, engine_ctx);

        let obj_val = class
            .static_object_value()
            .ok_or(error_no_static_graph(class_name))?;

        inner.set_field(&obj_val, field_name, &new_value)
    }
}

impl Default for RuseJsGlobalObject {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug)]
pub struct EngineContext<'a>(&'a mut boa_engine::Context);

impl<'a> EngineContext<'a> {
    pub fn new_boa_ctx() -> Box<boa_engine::Context> {
        Box::new(
            boa_engine::context::ContextBuilder::default()
                .host_hooks(EngineContextHooks.into())
                .build()
                .expect("Failed to build context"),
        )
    }

    pub fn create_engine_ctx(boa_ctx: &'a mut boa_engine::Context, classes: &TsClasses) -> Self {
        let mut engine_ctx = Self(boa_ctx);
        classes.init_engine_ctx(&mut engine_ctx);

        engine_ctx
    }

    pub fn reset_with_context(&self, post_ctx: &Context, classes: &TsClasses, cache: &Arc<Cache>) {
        let global_obj = self.global_object();
        let mut wrapped_inner =
            JsWrapped::<RuseJsGlobalObject>::mut_from_js_obj(&global_obj).unwrap();
        wrapped_inner.0 = RuseJsGlobalObject::Contxt(ContextGlobalObjectInner {
            context: std::ptr::from_ref(post_ctx),
            classes: std::ptr::from_ref(classes),
            cache: cache.clone(),
        });
    }

    pub fn reset_with_mut_context(
        &self,
        post_ctx: &mut Context,
        classes: &TsClasses,
        cache: &Arc<Cache>,
    ) {
        let global_obj = self.global_object();
        let mut wrapped_inner =
            JsWrapped::<RuseJsGlobalObject>::mut_from_js_obj(&global_obj).unwrap();
        wrapped_inner.0 = RuseJsGlobalObject::MutContxt(MutContextGlobalObjectInner {
            context: std::ptr::from_mut(post_ctx),
            classes: std::ptr::from_ref(classes),
            cache: cache.clone(),
            dirty: false,
        });
    }

    pub fn reset_with_graph(
        &self,
        graph_id: GraphIndex,
        graphs_map: &mut GraphsMap,
        classes: &TsClasses,
        id_gen: &Arc<GraphIdGenerator>,
        cache: &Arc<Cache>,
    ) {
        let global_obj = self.global_object();
        let mut wrapped_inner =
            JsWrapped::<RuseJsGlobalObject>::mut_from_js_obj(&global_obj).unwrap();
        wrapped_inner.0 = RuseJsGlobalObject::Graph(GraphGlobalObjectInner {
            graph_id: graph_id,
            graphs_map: std::ptr::from_mut(graphs_map),
            classes: std::ptr::from_ref(classes),
            id_gen: id_gen.clone(),
            cache: cache.clone(),
            dirty: false,
        });
    }
}

impl<'a> Deref for EngineContext<'a> {
    type Target = boa_engine::Context;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<'a> DerefMut for EngineContext<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}

impl<'a> EngineContext<'a> {
    pub fn is_dirty(&self) -> bool {
        let global_obj = self.global_object();
        let wrapped_inner = JsWrapped::<RuseJsGlobalObject>::get_from_js_obj(&global_obj).unwrap();
        if let Ok(inner) = wrapped_inner.inner() {
            inner.dirty()
        } else {
            false
        }
    }
}

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
                RuseJsGlobalObject::get_field(this, &field_name, &mut EngineContext(boa_ctx))
            })
        })
    }

    fn setter_for_field(field: &JsFieldDescription) -> Option<boa_engine::NativeFunction> {
        debug_assert!(!field.is_static);

        if field.is_readonly {
            return None;
        }

        let field_name = field.name.clone();
        Some(unsafe {
            boa_engine::native_function::NativeFunction::from_closure(move |this, args, boa_ctx| {
                RuseJsGlobalObject::set_field(
                    this,
                    &field_name,
                    &args[0],
                    &mut EngineContext(boa_ctx),
                )?;
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
                RuseJsGlobalObject::get_static_field(
                    &class_name,
                    &field_name,
                    &mut EngineContext(boa_ctx),
                )
            })
        })
    }

    fn setter_for_static_field(
        class_name: &CachedString,
        field: &JsFieldDescription,
    ) -> Option<boa_engine::NativeFunction> {
        debug_assert!(field.is_static);

        if field.is_readonly {
            return None;
        }

        let class_name = class_name.clone();
        let field_name = field.name.clone();
        Some(unsafe {
            boa_engine::native_function::NativeFunction::from_closure(move |_, args, boa_ctx| {
                RuseJsGlobalObject::set_static_field(
                    &class_name,
                    &field_name,
                    &args[0],
                    &mut EngineContext(boa_ctx),
                )?;
                Ok(boa_engine::JsValue::undefined())
            })
        })
    }

    fn method_js_function(
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
        let inner = global_ctx.inner()?;

        let cache = inner.cache();
        let id_gen = inner.id_gen();

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

        let graph_id = inner.get_graph_id_for_new_object()?;
        let node_id = id_gen.get_id_for_node();

        let graphs_map = inner.mut_graphs_map()?;

        graphs_map.construct_node(
            graph_id,
            node_id,
            self.desc.class_name.clone(),
            FieldsMap::from_iter(primitive_fields),
        );

        for (param, arg) in self.desc.constructor.params.iter().zip_eq(args) {
            if !param.is_prop || param.value_type.is_primitive() {
                continue;
            }

            if !arg.is_null_or_undefined() {
                let obj_val = JsWrapped::<ObjectValue>::get_from_js_val(arg)?;
                graphs_map.set_field(
                    param.name.clone(),
                    graph_id,
                    node_id,
                    &Value::Object(obj_val.0.clone()),
                );
            }
        }

        Ok(JsWrapped(ObjectValue {
            obj_type: self.desc.class_name.clone(),
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
                let attribute = match field.is_readonly {
                    true => boa_engine::property::Attribute::READONLY,
                    false => boa_engine::property::Attribute::WRITABLE,
                };
                builder.accessor(key, getter, setter, attribute);
            } else {
                let getter = Self::getter_for_static_field(&self.desc.class_name, field)
                    .map(|x| x.to_js_function(builder.context().realm()));
                let setter = Self::setter_for_static_field(&self.desc.class_name, field)
                    .map(|x| x.to_js_function(builder.context().realm()));
                let attribute = match field.is_readonly {
                    true => boa_engine::property::Attribute::READONLY,
                    false => boa_engine::property::Attribute::WRITABLE,
                };
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
                    MethodKind::Method => unreachable!(),
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
        self.create_new_object(new_target, args, &mut EngineContext(boa_ctx))
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
        self.call_constructor_body(instance, args, &mut EngineContext(boa_ctx))
    }
}
