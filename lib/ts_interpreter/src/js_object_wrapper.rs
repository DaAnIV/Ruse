use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    sync::Arc,
};

use boa_engine::{js_str, js_string, JsError, JsResult, JsValue, NativeFunction};
use itertools::Itertools;
use ruse_object_graph::{
    value::ObjectValue, value::Value, Attributes, Cache, CachedString, FieldName, FieldsMap,
    GraphIndex, GraphsMap, ObjectGraph, PrimitiveField,
};
use ruse_synthesizer::context::{Context, GraphIdGenerator};

use crate::{
    js_value::{js_value_to_primitive_value, js_value_to_value, value_to_js_value},
    ts_class::{JsFieldDescription, MethodDescription, TsClassDescription, TsClasses},
};

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
    fn get_graph(&self) -> &mut ObjectGraph;
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

impl GraphGlobalObjectInner {
    fn graph(&self) -> &mut ObjectGraph {
        Arc::make_mut(self.graphs_map_mut().get_mut(&self.graph_id).unwrap())
    }
    fn graphs_map_mut(&self) -> &mut GraphsMap {
        unsafe { self.graphs_map.as_mut().unwrap() }
    }
}

impl GlobalObjectInner for GraphGlobalObjectInner {
    fn set_field(
        &mut self,
        obj_val: &ObjectValue,
        field_name: &CachedString,
        new_value: &Value,
    ) -> boa_engine::JsResult<()> {
        let graph = Arc::make_mut(self.graphs_map_mut().get_mut(&obj_val.graph_id).unwrap());
        graph.set_field(&obj_val.node, field_name.clone(), new_value);

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
    fn get_graph(&self) -> &mut ObjectGraph {
        self.graph()
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

struct ContextGlobalObjectInner {
    cache: Arc<Cache>,
    context: *mut Context,
    classes: *const TsClasses,
    dirty: bool,
}

impl ContextGlobalObjectInner {
    fn context(&self) -> &mut Context {
        unsafe { self.context.as_mut().unwrap() }
    }
}

impl GlobalObjectInner for ContextGlobalObjectInner {
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
    fn get_graph(&self) -> &mut ObjectGraph {
        self.context().create_graph_in_map()
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

enum RuseJsGlobalObject {
    None,
    Graph(GraphGlobalObjectInner),
    Contxt(ContextGlobalObjectInner),
}

impl RuseJsGlobalObject {
    fn not_initialized_err() -> JsError {
        boa_engine::JsNativeError::typ()
            .with_message("Global object is not initialized")
            .into()
    }

    fn field_not_found_err(field_name: &str) -> JsError {
        boa_engine::JsNativeError::typ()
            .with_message(format!("Field {} not found", field_name))
            .into()
    }

    fn no_static_graph_err(class_name: &str) -> JsError {
        boa_engine::JsNativeError::typ()
            .with_message(format!("Class {} has not static graph", class_name))
            .into()
    }

    fn inner(&self) -> JsResult<&dyn GlobalObjectInner> {
        match self {
            RuseJsGlobalObject::None => Err(Self::not_initialized_err()),
            RuseJsGlobalObject::Graph(graph_inner) => Ok(graph_inner),
            RuseJsGlobalObject::Contxt(context_inner) => Ok(context_inner),
        }
    }

    fn inner_mut(&mut self) -> JsResult<&mut dyn GlobalObjectInner> {
        match self {
            RuseJsGlobalObject::None => Err(Self::not_initialized_err()),
            RuseJsGlobalObject::Graph(graph_inner) => Ok(graph_inner),
            RuseJsGlobalObject::Contxt(context_inner) => Ok(context_inner),
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
            .ok_or(Self::field_not_found_err(field_name))?;
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
            .ok_or(Self::no_static_graph_err(class_name))?;

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

    pub fn reset_with_context(
        &self,
        post_ctx: &mut Context,
        classes: &TsClasses,
        cache: &Arc<Cache>,
    ) {
        let global_obj = self.global_object();
        let mut wrapped_inner =
            JsWrapped::<RuseJsGlobalObject>::mut_from_js_obj(&global_obj).unwrap();
        wrapped_inner.0 = RuseJsGlobalObject::Contxt(ContextGlobalObjectInner {
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
    methods_funcs: HashMap<String, boa_engine::NativeFunction>,
    constructor_func: boa_engine::NativeFunction,
}

impl JsObjectWrapper {
    pub fn new(desc: Arc<TsClassDescription>) -> Self {
        JsObjectWrapper {
            constructor_func: Self::method_js_function(&desc.constructor),
            methods_funcs: Self::method_js_functions(&desc.methods),
            desc,
        }
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

    fn method_js_function(method: &MethodDescription) -> boa_engine::NativeFunction {
        let arg_names = method
            .params
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let caller_args = (0..(method.params.len()))
            .map(|i| format!("arg{}", i))
            .collect::<Vec<_>>()
            .join(", ");
        let mut code = format!(
            "function func({}) {{{}}}\n",
            arg_names,
            method.body.as_ref().unwrap()
        );

        code += &format!("func.call(arg_this, {});", caller_args);
        unsafe {
            boa_engine::native_function::NativeFunction::from_closure(move |this, args, boa_ctx| {
                boa_ctx.register_global_property(
                    js_str!("arg_this"),
                    this.clone(),
                    boa_engine::property::Attribute::all(),
                )?;
                for (i, arg) in args.iter().enumerate() {
                    boa_ctx.register_global_property(
                        js_string!(format!("arg{}", i)),
                        arg.clone(),
                        boa_engine::property::Attribute::all(),
                    )?;
                }
                let js_souce = boa_engine::Source::from_bytes(code.as_str());
                boa_ctx.eval(js_souce)
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

        let fields =
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

        let graph = inner.get_graph();

        let node_id = id_gen.get_id_for_node();
        let graph_id = graph.id;

        graph.construct_node(
            node_id,
            self.desc.class_name.clone(),
            FieldsMap::from_iter(fields),
        );

        Ok(JsWrapped(ObjectValue {
            obj_type: self.desc.class_name.clone(),
            graph_id: graph_id,
            node: node_id,
        }))
    }

    fn call_constructor_body(
        &self,
        instance: &boa_engine::JsObject,
        args: &[boa_engine::JsValue],
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsResult<()> {
        self.constructor_func.call(
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
        self.methods_funcs[method_name].call(this, args, engine_ctx)
    }

    fn method_js_functions(
        methods: &HashMap<String, MethodDescription>,
    ) -> HashMap<String, NativeFunction> {
        let mut functions = HashMap::default();
        for (key, method) in methods {
            functions.insert(key.clone(), Self::method_js_function(method));
        }

        functions
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

        for method in self.desc.methods.values() {
            let func_name = js_string!(method.name.as_str());
            let func = self.methods_funcs[method.name.as_str()].clone();
            if method.is_static {
                builder.static_method(func_name, method.params.len(), func);
            } else {
                builder.method(func_name, method.params.len(), func);
            }
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
