use std::{
    cell::RefCell,
    collections::HashMap,
    ops::{Deref, DerefMut},
    sync::Arc,
};

use boa_engine::{
    context::intrinsics::StandardConstructor, js_string, property::PropertyDescriptor, JsArgs,
    JsObject, JsResult, JsValue,
};
use ruse_object_graph::{
    class_name,
    value::{ObjectValue, Value},
    vnull, ClassName, FieldName, GraphIndex, GraphsMap, ObjectType, PrimitiveValue, ValueType,
};
use ruse_synthesizer::{
    context::{Context, GraphIdGenerator},
    temp_value,
};

use crate::{
    js_console_logger::RuseJsConsoleLogger,
    js_errors::*,
    js_object_wrapper::JsUserClassWrapper,
    js_value::{js_value_to_value, value_to_js_value},
    js_wrapped::JsWrapped,
    jsbuiltins::{jsarray::JsArrayWrapper, jsiterator::JsObjectIterator, jsmap::JsMapWrapper},
    ts_class::{BuiltinClassWrapper, JsFieldDescription},
    ts_classes::TsClasses,
    ts_global_class::TsGlobalClass,
};

#[derive(Debug, Clone, Copy)]
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
        field_name: &FieldName,
        new_value: &Value,
    ) -> boa_engine::JsResult<()>;

    fn classes(&self) -> &TsClasses;
    fn graphs_map(&self) -> &GraphsMap;
    fn mut_graphs_map(&self) -> boa_engine::JsResult<&mut GraphsMap>;
    fn get_graph_id_for_new_object(&self) -> boa_engine::JsResult<GraphIndex>;
    fn id_gen(&self) -> &GraphIdGenerator;
    fn dirty(&self) -> bool;
    fn set_dirty(&mut self);
}

struct GraphGlobalObjectInner {
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
        field_name: &FieldName,
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
        field_name: &FieldName,
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
        _field_name: &FieldName,
        _new_value: &Value,
    ) -> boa_engine::JsResult<()> {
        Err(js_error_immutable_context())
    }

    fn classes(&self) -> &TsClasses {
        unsafe { self.classes.as_ref().unwrap() }
    }
    fn graphs_map(&self) -> &GraphsMap {
        &self.context().graphs_map
    }
    fn mut_graphs_map(&self) -> boa_engine::JsResult<&mut GraphsMap> {
        Err(js_error_immutable_context())
    }
    fn get_graph_id_for_new_object(&self) -> boa_engine::JsResult<GraphIndex> {
        Err(js_error_immutable_context())
    }
    fn id_gen(&self) -> &GraphIdGenerator {
        &self.context().graph_id_gen
    }

    fn dirty(&self) -> bool {
        false
    }
    fn set_dirty(&mut self) {}
}

enum RuseJsGlobalObjectInner {
    None,
    Graph(GraphGlobalObjectInner),
    Contxt(ContextGlobalObjectInner),
    MutContxt(MutContextGlobalObjectInner),
}

#[derive(Debug, Default)]
pub struct RuseJsConstructors {
    array_constructor: RefCell<Option<StandardConstructor>>,
    map_constructor: RefCell<Option<StandardConstructor>>,
    iter_constructor: RefCell<Option<StandardConstructor>>,
}

impl RuseJsConstructors {
    pub fn array_prototype(&self, engine_ctx: &mut EngineContext<'_>) -> JsResult<JsObject> {
        if let Some(constructor) = self.array_constructor.borrow().as_ref() {
            return Ok(constructor.prototype());
        }
        let constructor = JsArrayWrapper::build_standard_constructor(engine_ctx)?;
        Ok(self
            .array_constructor
            .borrow_mut()
            .insert(constructor)
            .prototype())
    }

    pub fn map_prototype(&self, engine_ctx: &mut EngineContext<'_>) -> JsResult<JsObject> {
        if let Some(constructor) = self.map_constructor.borrow().as_ref() {
            return Ok(constructor.prototype());
        }
        let constructor = JsMapWrapper::build_standard_constructor(engine_ctx)?;
        Ok(self
            .map_constructor
            .borrow_mut()
            .insert(constructor)
            .prototype())
    }

    pub fn iter_prototype(&self, engine_ctx: &mut EngineContext<'_>) -> JsResult<JsObject> {
        if let Some(constructor) = self.iter_constructor.borrow().as_ref() {
            return Ok(constructor.prototype());
        }
        let constructor = JsObjectIterator::build_standard_constructor(engine_ctx)?;
        Ok(self
            .iter_constructor
            .borrow_mut()
            .insert(constructor)
            .prototype())
    }
}

pub(crate) struct RuseJsGlobalObject {
    inner: RuseJsGlobalObjectInner,
    constructors: RuseJsConstructors,
    user_classes: HashMap<ClassName, JsUserClassWrapper>,
}

impl RuseJsGlobalObject {
    fn inner(&self) -> JsResult<&dyn GlobalObjectInner> {
        match &self.inner {
            RuseJsGlobalObjectInner::None => Err(js_error_not_initialized()),
            RuseJsGlobalObjectInner::Graph(graph_inner) => Ok(graph_inner),
            RuseJsGlobalObjectInner::Contxt(context_inner) => Ok(context_inner),
            RuseJsGlobalObjectInner::MutContxt(context_inner) => Ok(context_inner),
        }
    }

    fn inner_mut(&mut self) -> JsResult<&mut dyn GlobalObjectInner> {
        match &mut self.inner {
            RuseJsGlobalObjectInner::None => Err(js_error_not_initialized()),
            RuseJsGlobalObjectInner::Graph(graph_inner) => Ok(graph_inner),
            RuseJsGlobalObjectInner::Contxt(context_inner) => Ok(context_inner),
            RuseJsGlobalObjectInner::MutContxt(context_inner) => Ok(context_inner),
        }
    }

    fn value_to_js_value(
        &self,
        value: &Value,
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsResult<boa_engine::JsValue> {
        value_to_js_value(self.classes()?, value, engine_ctx)
    }

    fn js_value_to_value(
        &self,
        js_value: &boa_engine::JsValue,
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsResult<Value> {
        js_value_to_value(self.classes()?, js_value, engine_ctx)
    }

    pub fn get_field(
        wrapped_obj: &boa_engine::JsValue,
        field_name: &FieldName,
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsResult<boa_engine::JsValue> {
        let obj_val = JsWrapped::<ObjectValue>::get_from_js_val(wrapped_obj)?;
        let global_obj = engine_ctx.global_object();
        let inner: boa_engine::gc::GcRef<'_, JsWrapped<RuseJsGlobalObject>> =
            JsWrapped::<Self>::get_from_js_obj(&global_obj)?;

        let field = obj_val
            .get_field_value(field_name, inner.graphs_map()?)
            .unwrap_or(vnull!());

        inner.value_to_js_value(&field, engine_ctx)
    }

    pub fn set_field(
        wrapped_obj: &boa_engine::JsValue,
        field_name: &FieldName,
        new_js_value: &boa_engine::JsValue,
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsResult<()> {
        let obj_val = JsWrapped::<ObjectValue>::get_from_js_val(wrapped_obj)?;
        let global_obj = engine_ctx.global_object();

        let new_value = {
            let inner = JsWrapped::<Self>::get_from_js_obj(&global_obj)?;
            inner.js_value_to_value(&new_js_value, engine_ctx)
        }?;

        {
            let mut wrapped_inner = JsWrapped::<Self>::mut_from_js_obj(&global_obj)?;
            let inner = wrapped_inner.inner_mut()?;

            inner.set_field(&obj_val, field_name, &new_value)?;
            inner.set_dirty();
        }
        Ok(())
    }

    pub fn get_static_field(
        class_name: &ClassName,
        field_name: &FieldName,
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsResult<boa_engine::JsValue> {
        let global_obj = engine_ctx.global_object();
        let inner = JsWrapped::<Self>::get_from_js_obj(&global_obj)?;
        let classes = inner.classes()?;

        let class = classes.get_user_class(class_name).unwrap();

        let field = class
            .static_fields
            .get(field_name)
            .unwrap_or(&temp_value!(vnull!()));

        inner.value_to_js_value(&field.val, engine_ctx)
    }

    pub fn set_static_field(
        class_name: &ClassName,
        field_name: &FieldName,
        new_js_value: &boa_engine::JsValue,
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsResult<()> {
        let global_obj = engine_ctx.global_object();
        let (new_value, obj_val) = {
            let inner = JsWrapped::<Self>::get_from_js_obj(&global_obj)?;
            let classes = inner.classes()?;

            let class = classes.get_user_class(class_name).unwrap();
            let obj_val = class
                .static_object_value()
                .ok_or_else(|| js_error_no_static_graph(class_name))?;

            (inner.js_value_to_value(&new_js_value, engine_ctx)?, obj_val)
        };

        {
            let mut wrapped_inner = JsWrapped::<Self>::mut_from_js_obj(&global_obj)?;
            let inner = wrapped_inner.inner_mut()?;
            inner.set_field(&obj_val, field_name, &new_value)
        }
    }

    pub fn get_global_field(
        field_name: &FieldName,
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsResult<boa_engine::JsValue> {
        let global_obj = engine_ctx.global_object();
        let inner = JsWrapped::<Self>::get_from_js_obj(&global_obj)?;
        let classes = inner.classes()?;

        let class = classes
            .get_global_class()
            .ok_or_else(|| js_error_global_class_not_found())?;

        let field = class
            .static_fields
            .get(field_name)
            .ok_or_else(|| js_error_global_field_not_found(field_name))?;

        inner.value_to_js_value(&field.val, engine_ctx)
    }

    pub fn set_global_field(
        field_name: &FieldName,
        new_js_value: &boa_engine::JsValue,
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsResult<()> {
        let global_obj = engine_ctx.global_object();
        let (new_value, obj_val) = {
            let inner = JsWrapped::<Self>::get_from_js_obj(&global_obj)?;
            let classes = inner.classes()?;

            let class = classes
                .get_global_class()
                .ok_or_else(|| js_error_global_class_not_found())?;
            let obj_val = class
                .static_object_value()
                .ok_or_else(|| js_error_no_global_static_graph())?;

            (inner.js_value_to_value(&new_js_value, engine_ctx)?, obj_val)
        };

        {
            let mut wrapped_inner = JsWrapped::<Self>::mut_from_js_obj(&global_obj)?;
            let inner = wrapped_inner.inner_mut()?;
            inner.set_field(&obj_val, field_name, &new_value)
        }
    }

    pub fn classes(&self) -> boa_engine::JsResult<&TsClasses> {
        Ok(self.inner()?.classes())
    }
    pub fn graphs_map(&self) -> boa_engine::JsResult<&GraphsMap> {
        Ok(self.inner()?.graphs_map())
    }
    pub fn mut_graphs_map(&self) -> boa_engine::JsResult<&mut GraphsMap> {
        self.inner()?.mut_graphs_map()
    }
    pub fn get_graph_id_for_new_object(&self) -> boa_engine::JsResult<GraphIndex> {
        self.inner()?.get_graph_id_for_new_object()
    }
    pub fn id_gen(&self) -> boa_engine::JsResult<&GraphIdGenerator> {
        Ok(self.inner()?.id_gen())
    }

    pub fn constructors(&self) -> &RuseJsConstructors {
        &self.constructors
    }

    pub fn get_user_class(&self, class_name: &ClassName) -> JsResult<&JsUserClassWrapper> {
        self.user_classes
            .get(class_name)
            .ok_or_else(|| js_error_user_class_not_found(class_name))
    }
}

impl Default for RuseJsGlobalObject {
    fn default() -> Self {
        Self {
            inner: RuseJsGlobalObjectInner::None,
            constructors: Default::default(),
            user_classes: Default::default(),
        }
    }
}

#[derive(Debug)]
pub struct EngineContext<'a>(&'a mut boa_engine::Context);

impl<'a> EngineContext<'a> {
    pub fn new_boa_ctx() -> Box<boa_engine::Context> {
        Box::new(
            boa_engine::context::ContextBuilder::default()
                .host_hooks(&EngineContextHooks)
                .build()
                .expect("Failed to build context"),
        )
    }

    pub fn create_engine_ctx(boa_ctx: &'a mut boa_engine::Context, classes: &TsClasses) -> Self {
        let mut engine_ctx = Self(boa_ctx);
        classes.init_engine_ctx(&mut engine_ctx);

        let console = boa_runtime::Console::init_with_logger(&mut engine_ctx, RuseJsConsoleLogger);
        // Register the console as a global property to the context.
        engine_ctx
            .register_global_property(
                js_string!(boa_runtime::Console::NAME),
                console,
                boa_engine::property::Attribute::all(),
            )
            .expect("the console object shouldn't exist yet");

        engine_ctx
    }

    pub fn reset_with_context(&self, post_ctx: &Context, classes: &TsClasses) {
        let global_obj = self.global_object();
        let mut wrapped_inner =
            JsWrapped::<RuseJsGlobalObject>::mut_from_js_obj(&global_obj).unwrap();
        wrapped_inner.0.inner = RuseJsGlobalObjectInner::Contxt(ContextGlobalObjectInner {
            context: std::ptr::from_ref(post_ctx),
            classes: std::ptr::from_ref(classes),
        });
    }

    pub fn reset_with_mut_context(&self, post_ctx: &mut Context, classes: &TsClasses) {
        let global_obj = self.global_object();
        let mut wrapped_inner =
            JsWrapped::<RuseJsGlobalObject>::mut_from_js_obj(&global_obj).unwrap();
        wrapped_inner.0.inner = RuseJsGlobalObjectInner::MutContxt(MutContextGlobalObjectInner {
            context: std::ptr::from_mut(post_ctx),
            classes: std::ptr::from_ref(classes),
            dirty: false,
        });
    }

    pub fn reset_with_graph(
        &self,
        graph_id: GraphIndex,
        graphs_map: &mut GraphsMap,
        classes: &TsClasses,
        id_gen: &Arc<GraphIdGenerator>,
    ) {
        let global_obj = self.global_object();
        let mut wrapped_inner =
            JsWrapped::<RuseJsGlobalObject>::mut_from_js_obj(&global_obj).unwrap();
        wrapped_inner.0.inner = RuseJsGlobalObjectInner::Graph(GraphGlobalObjectInner {
            graph_id: graph_id,
            graphs_map: std::ptr::from_mut(graphs_map),
            classes: std::ptr::from_ref(classes),
            id_gen: id_gen.clone(),
            dirty: false,
        });
    }

    pub fn create_array_object<I>(
        &self,
        elements: I,
        elem_type: &ValueType,
    ) -> JsResult<ObjectValue>
    where
        I: IntoIterator<Item = Value>,
    {
        let global_obj = self.global_object();
        let global_ctx = JsWrapped::<RuseJsGlobalObject>::get_from_js_obj(&global_obj)?;
        let inner = global_ctx.inner()?;

        let graph_id = inner.get_graph_id_for_new_object()?;
        let node_id = inner.id_gen().get_id_for_node();

        let graphs_map = inner.mut_graphs_map()?;

        if elem_type.is_primitive() {
            graphs_map.add_primitive_array_object(
                graph_id,
                node_id,
                elem_type,
                elements.into_iter().map(|x| x.into_primitive().unwrap()),
            );
        } else {
            graphs_map.add_array_object(graph_id, node_id, elem_type, elements);
        }

        Ok(ObjectValue {
            obj_type: ObjectType::array_obj_type(elem_type),
            graph_id,
            node: node_id,
        })
    }

    pub fn create_map_object<I>(
        &self,
        elements: I,
        key_type: &ValueType,
        value_type: &ValueType,
    ) -> JsResult<ObjectValue>
    where
        I: IntoIterator<Item = (PrimitiveValue, Value)>,
    {
        let global_obj = self.global_object();
        let global_ctx = JsWrapped::<RuseJsGlobalObject>::get_from_js_obj(&global_obj)?;
        let inner = global_ctx.inner()?;

        let graph_id = inner.get_graph_id_for_new_object()?;
        let node_id = inner.id_gen().get_id_for_node();

        let graphs_map = inner.mut_graphs_map()?;

        if value_type.is_primitive() {
            graphs_map.add_primitive_map_object(
                graph_id,
                node_id,
                key_type,
                value_type,
                elements
                    .into_iter()
                    .map(|(k, v)| (k, v.into_primitive().unwrap())),
            );
        } else {
            graphs_map.add_map_object(graph_id, node_id, key_type, value_type, elements);
        }

        Ok(ObjectValue {
            obj_type: ObjectType::map_obj_type(key_type, value_type),
            graph_id,
            node: node_id,
        })
    }

    pub fn register_global_class(&mut self, global_class: &TsGlobalClass) -> JsResult<()> {
        for (name, method) in &global_class.methods {
            let js_func = JsUserClassWrapper::method_js_function(method, self);
            self.register_global_builtin_callable(
                js_string!(name.as_str()),
                method.params.len(),
                js_func,
            )?;
        }

        for (name, field) in &global_class.fields {
            let getter = Self::getter_for_global_field(field).to_js_function(self.realm());
            let setter = Self::setter_for_global_field(field).to_js_function(self.realm());

            let property = PropertyDescriptor::builder().get(getter).set(setter);
            self.global_object().define_property_or_throw(
                js_string!(name.as_str()),
                property,
                self,
            )?;
        }

        Ok(())
    }

    fn getter_for_global_field(field: &JsFieldDescription) -> boa_engine::NativeFunction {
        let field_name = field.name.clone();
        unsafe {
            boa_engine::native_function::NativeFunction::from_closure(move |_, _, boa_ctx| {
                RuseJsGlobalObject::get_global_field(&field_name, &mut boa_ctx.into())
            })
        }
    }

    fn setter_for_global_field(field: &JsFieldDescription) -> boa_engine::NativeFunction {
        let field_name = field.name.clone();
        unsafe {
            boa_engine::native_function::NativeFunction::from_closure(move |_, args, boa_ctx| {
                RuseJsGlobalObject::set_global_field(
                    &field_name,
                    args.get_or_undefined(0),
                    &mut boa_ctx.into(),
                )?;
                Ok(boa_engine::JsValue::undefined())
            })
        }
    }

    pub(crate) fn call_global_function(
        &mut self,
        method_name: &str,
        args: &[JsValue],
    ) -> JsResult<JsValue> {
        let func = self.global_object().get(js_string!(method_name), self)?;
        func.as_callable()
            .unwrap()
            .call(&JsValue::undefined(), args, self)
    }

    pub fn register_user_class(&mut self, class: JsUserClassWrapper) -> JsResult<()> {
        let global_obj = self.global_object();

        let name = class_name!(class.name().as_str());
        let property = PropertyDescriptor::builder().value(class.constructor());

        {
            let mut global_ctx = JsWrapped::<RuseJsGlobalObject>::mut_from_js_obj(&global_obj)?;
            global_ctx.user_classes.insert(name.clone(), class);
        }

        global_obj.define_property_or_throw(js_string!(name.as_str()), property, self)?;
        Ok(())
    }
}

impl<'a> From<&'a mut boa_engine::Context> for EngineContext<'a> {
    fn from(value: &'a mut boa_engine::Context) -> Self {
        Self(value)
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
