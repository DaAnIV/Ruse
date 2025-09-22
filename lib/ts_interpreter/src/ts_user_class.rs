use std::{collections::HashMap, sync::Arc};

use boa_engine::JsResult;
use ruse_object_graph::{
    class_name,
    value::{ObjectValue, Value},
    ClassName, FieldName, GraphIndex, GraphsMap, NodeIndex, ObjectGraph, ObjectType, ValueType,
};
use ruse_synthesizer::{
    context::GraphIdGenerator,
    location::LocValue,
    opcode::{ExprOpcode, OpcodesList},
};
use swc_ecma_visit::{Visit, VisitWith};
use tracing::error;

use crate::{
    dts_visitor::DtsClassDecl,
    engine_context::{EngineContext, RuseJsGlobalObject},
    js_errors::*,
    js_object_wrapper::JsUserClassWrapper,
    js_value::{args_to_js_args, TryFromJs, TryIntoJs},
    opcode::{ClassConstructorOp, ClassMethodOp, MemberOp, StaticMemberOp},
    ts_class::*,
    ts_classes::TsClasses,
};

use swc_ecma_ast::ClassDecl;

#[derive(Debug)]
pub struct TsUserClass {
    pub description: Arc<TsClassDescription>,
    pub member_opcodes: OpcodesList,
    pub method_opcodes: OpcodesList,
    pub constructor_opcodes: OpcodesList,
    pub static_graph: Option<Arc<ObjectGraph>>,
    pub root_node: NodeIndex,
    pub static_fields: HashMap<FieldName, LocValue>,
    static_graph_obj_type: ObjectType,
}

impl TsUserClass {
    pub fn generate_object<I>(
        &self,
        map: I,
        graphs_map: &mut GraphsMap,
        graph_id: GraphIndex,
        graph_id_gen: &GraphIdGenerator,
    ) -> ObjectValue
    where
        I: IntoIterator<Item = (FieldName, Value)>,
    {
        // TODO: Check map set attributes etc...

        let obj_id = graphs_map.add_object_from_map(
            graph_id,
            graph_id_gen.get_id_for_node(),
            ObjectType::Class(self.description.class_name.clone()),
            map,
        );

        ObjectValue {
            obj_type: ObjectType::Class(self.description.class_name.clone()),
            graph_id: graph_id,
            node: obj_id,
        }
    }

    pub fn call_constructor<'a, I>(
        &self,
        params: I,
        engine_ctx: &mut EngineContext,
    ) -> boa_engine::JsResult<ObjectValue>
    where
        I: IntoIterator<Item = &'a Value>,
    {
        let js_args = args_to_js_args(params, engine_ctx)?;

        let js_obj = JsUserClassWrapper::call_constructor(
            &self.description.class_name,
            &js_args,
            engine_ctx,
        )?;

        ObjectValue::try_from_js(&js_obj, engine_ctx)
    }

    pub fn call_static_method<'a, I>(
        &self,
        method_desc: &MethodDescription,
        params: I,
        engine_ctx: &mut EngineContext,
    ) -> boa_engine::JsResult<Value>
    where
        I: IntoIterator<Item = &'a Value>,
    {
        debug_assert!(method_desc.is_static);

        let js_args = args_to_js_args(params, engine_ctx)?;

        let method_name = match method_desc.kind {
            MethodKind::Method => method_desc.name.as_str(),
            MethodKind::GlobalFunction => method_desc.name.as_str(),
            MethodKind::Getter => &JsUserClassWrapper::getter_name(&method_desc.name),
            MethodKind::Setter => &JsUserClassWrapper::setter_name(&method_desc.name),
        };

        let result = JsUserClassWrapper::call_static_method_body(
            &self.description.class_name,
            method_name,
            &js_args,
            engine_ctx,
        )?;

        Value::try_from_js(&result, engine_ctx)
    }

    pub fn call_method<'a, I>(
        &self,
        method_desc: &MethodDescription,
        this: &Value,
        params: I,
        engine_ctx: &mut EngineContext,
    ) -> boa_engine::JsResult<Value>
    where
        I: IntoIterator<Item = &'a Value>,
    {
        debug_assert!(!method_desc.is_static);
        if this.is_null() {
            return Err(js_error_null_this());
        }

        let this = this.try_into_js(engine_ctx)?;
        let js_args = args_to_js_args(params, engine_ctx)?;

        let method_name = match method_desc.kind {
            MethodKind::Method => method_desc.name.as_str(),
            MethodKind::GlobalFunction => method_desc.name.as_str(),
            MethodKind::Getter => &JsUserClassWrapper::getter_name(&method_desc.name),
            MethodKind::Setter => &JsUserClassWrapper::setter_name(&method_desc.name),
        };

        let result = JsUserClassWrapper::call_method_body(
            &self.description.class_name,
            method_name,
            &this,
            &js_args,
            engine_ctx,
        )?;

        Value::try_from_js(&result, engine_ctx)
    }

    pub fn static_object_value(&self) -> Option<ObjectValue> {
        let static_graph = self.static_graph.as_ref()?;
        Some(ObjectValue {
            obj_type: self.static_graph_obj_type.clone(),
            graph_id: static_graph.id,
            node: self.root_node,
        })
    }

    pub(crate) fn register_class(&self, engine_ctx: &mut EngineContext) -> JsResult<()> {
        let wrapper = JsUserClassWrapper::new(self.description.clone(), engine_ctx)?;
        engine_ctx.register_user_class(wrapper)
    }
}

impl TsClass for TsUserClass {
    fn obj_type(&self, template_types: Option<Vec<ValueType>>) -> ValueType {
        assert!(template_types.is_none());

        ValueType::class_value_type(self.get_class_name().clone())
    }

    fn wrap_as_js_object(
        &self,
        obj: ObjectValue,
        engine_ctx: &mut EngineContext,
    ) -> JsResult<boa_engine::JsObject> {
        let global_obj = engine_ctx.global_object();
        let global_ctx = RuseJsGlobalObject::from_object(&global_obj)?;

        let wrapper = global_ctx.get_user_class(&self.description.class_name)?;
        wrapper.wrap_object(obj, engine_ctx)
    }

    fn is_parametrized(&self) -> bool {
        false
    }

    fn get_class_name(&self) -> &ClassName {
        &self.description.class_name
    }

    fn get_class_id(&self) -> u64 {
        self.description.id
    }
}

unsafe impl Send for TsUserClass {}
unsafe impl Sync for TsUserClass {}

pub(crate) struct TsUserClassBuilder<'a> {
    id: u64,
    class_name: ClassName,
    dts: &'a DtsClassDecl,
    fields: HashMap<String, JsFieldDescription>,
    methods: HashMap<(String, MethodKind), MethodDescription>,
    constructor: Option<MethodDescription>,
    gen_id: Arc<GraphIdGenerator>,
    super_class: Option<ClassName>,
    is_abstract: bool,

    visitor_failed: bool,
}

impl<'a> Visit for TsUserClassBuilder<'a> {
    fn visit_class_prop(&mut self, node: &swc_ecma_ast::ClassProp) {
        let id = node.key.as_ident().unwrap().sym.clone();
        let field_desc = if let Some(prop_dts) = self.dts.props.get(&id) {
            JsFieldDescription::from((node, prop_dts))
        } else {
            JsFieldDescription::from(node)
        };

        if !field_desc.is_private && field_desc.value_type.is_none() {
            error!(
                "Field {} of class {} is public but has no type",
                field_desc.name, self.class_name
            );
            self.visitor_failed = true;
        }

        self.fields.insert(field_desc.name.to_string(), field_desc);
    }

    fn visit_class_method(&mut self, node: &swc_ecma_ast::ClassMethod) {
        let id = node.key.as_ident().unwrap().sym.clone();
        let method_desc = if let Some(method_dts) = self.dts.methods.get(&(id, node.kind)) {
            MethodDescription::from((node, method_dts))
        } else {
            MethodDescription::from(node)
        };

        self.methods.insert(
            (method_desc.name.to_string(), method_desc.kind.clone()),
            method_desc,
        );
    }
    fn visit_constructor(&mut self, node: &swc_ecma_ast::Constructor) {
        let method_desc = if let Some(method_dts) = self.dts.constructor.as_ref() {
            MethodDescription::from((node, method_dts))
        } else {
            MethodDescription::from(node)
        };

        assert!(self.constructor.replace(method_desc).is_none());
    }
}

// Main functions
impl<'a> TsUserClassBuilder<'a> {
    pub fn from_class_decl(
        decl: &ClassDecl,
        dts: &'a DtsClassDecl,
        gen_id: Arc<GraphIdGenerator>,
    ) -> Result<Self, ()> {
        let super_class = decl.class.super_class.as_deref().map(|super_class| {
            let ident = super_class.as_ident().unwrap();
            class_name!(ident.sym.as_str())
        });

        let mut class = Self {
            id: gen_id.get_id_for_graph().0 as u64,
            class_name: class_name!(decl.ident.sym.as_str()),
            dts,
            fields: Default::default(),
            methods: Default::default(),
            constructor: Default::default(),
            gen_id,
            super_class,
            is_abstract: decl.class.is_abstract,
            visitor_failed: false,
        };

        decl.visit_children_with(&mut class);

        if class.visitor_failed {
            return Err(());
        }

        Ok(class)
    }

    pub fn finalize(self, classes: &mut TsClasses, graphs_map: &mut GraphsMap) {
        let description = Arc::new(TsClassDescription {
            id: self.id,
            class_name: self.class_name.clone(),
            fields: self.fields.clone(),
            constructor: self.constructor.as_ref().unwrap().clone(),
            methods: self.methods.clone(),
            extends: self.super_class.clone(),
            is_abstract: self.is_abstract,
        });

        let static_graph_obj_type =
            StaticGraphBuilder::static_graph_obj_type(&description.class_name);

        let class = TsUserClass {
            description,
            member_opcodes: Default::default(),
            method_opcodes: Default::default(),
            constructor_opcodes: Default::default(),
            static_graph: None,
            root_node: classes.static_classes_gen_id.get_id_for_node(),
            static_fields: Default::default(),
            static_graph_obj_type,
        };
        let root_node = class.root_node;
        classes.add_user_class(class.description.class_name.to_string(), class);

        if self.fields.values().any(|field| field.is_static) {
            let static_graph_builder = StaticGraphBuilder {
                id: GraphIndex(self.id as usize),
                class_name: &self.class_name,
                gen_id: &self.gen_id,
            };
            let graph = static_graph_builder.populate_static_graph(
                self.fields.values(),
                root_node,
                graphs_map,
                classes,
            );
            classes
                .get_user_class_mut(&self.class_name)
                .unwrap()
                .static_graph = Some(graph);
        }

        self.populate_opcodes(classes.get_user_class_mut(&self.class_name).unwrap());
    }
}

// Opcodes
impl<'a> TsUserClassBuilder<'a> {
    fn populate_opcodes(&self, class: &mut TsUserClass) {
        for field in self.fields.values() {
            if let Some(op) = self.get_opcode_from_field(class, field) {
                class.member_opcodes.push(op);
            }
        }

        class.method_opcodes.extend(
            self.methods
                .values()
                .flat_map(|m| self.get_method_opcodes(m)),
        );

        let constructor = self.constructor.as_ref().unwrap();

        if !self.is_abstract && !constructor.is_private {
            for arg_types in constructor.param_types.iter() {
                class
                    .constructor_opcodes
                    .push(Arc::new(ClassConstructorOp::new(
                        self.class_name.clone(),
                        constructor.clone(),
                        arg_types.clone(),
                    )));
            }
        }
    }

    fn get_opcode_from_field(
        &self,
        class: &TsUserClass,
        field: &JsFieldDescription,
    ) -> Option<Arc<dyn ExprOpcode>> {
        if field.is_private {
            return None;
        }

        let op: Arc<dyn ExprOpcode> = if field.is_static {
            let loc_val = class.static_fields[&field.name].clone();
            Arc::new(StaticMemberOp::new(
                self.class_name.clone(),
                field.name.clone(),
                class.static_graph.as_ref().unwrap().clone(),
                loc_val,
            ))
        } else {
            Arc::new(MemberOp::new(self.class_name.clone(), field.name.clone()))
        };

        return Some(op);
    }

    fn get_method_opcodes(&self, method: &MethodDescription) -> Vec<Arc<dyn ExprOpcode>> {
        let params = if method.is_private {
            &vec![]
        } else {
            &method.param_types
        };

        params
            .iter()
            .map(|x| {
                Arc::new(ClassMethodOp::new(
                    self.class_name.clone(),
                    method,
                    x.clone(),
                )) as Arc<dyn ExprOpcode>
            })
            .collect()
    }
}
