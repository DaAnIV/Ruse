use std::{collections::HashMap, sync::Arc};

use anyhow::Error;
use boa_engine::{class::DynamicClassBuilder, JsResult, JsValue};
use itertools::Itertools;
use ruse_object_graph::{
    scached, str_cached,
    value::{ObjectValue, Value},
    Cache, ClassName, FieldName, GraphIndex, GraphsMap, NodeIndex, ObjectGraph, ObjectType,
    ValueType,
};
use ruse_synthesizer::{
    context::GraphIdGenerator,
    location::LocValue,
    opcode::{ExprOpcode, OpcodesList},
};
use swc_common::{
    errors::{ColorConfig, Handler},
    FileName, SourceMap,
};
use swc_ecma_parser::{Syntax, TsSyntax};

use crate::{
    engine_context::EngineContext,
    js_errors::*,
    js_object_wrapper::JsObjectWrapper,
    js_value::{js_value_to_value, value_to_js_value},
    js_wrapped::JsWrapped,
    opcode::{ClassConstructorOp, ClassMethodOp, MemberOp, StaticMemberOp},
    ts_class::*,
    ts_classes::TsClasses,
};

use swc_ecma_ast::{self as ast, ClassDecl};

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
    wrapper: JsObjectWrapper,
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

    pub(crate) fn get_ast(code: &str) -> Result<ast::Program, Error> {
        let cm = Arc::<SourceMap>::default();
        let handler = Handler::with_tty_emitter(ColorConfig::Auto, true, false, Some(cm.clone()));
        let c = swc::Compiler::new(cm.clone());

        let fm = cm.new_source_file(FileName::Anon.into(), code.to_owned());

        match c.parse_js(
            fm,
            &handler,
            ast::EsVersion::Es2022,
            Syntax::Typescript(TsSyntax::default()),
            swc::config::IsModule::Bool(false),
            None,
        ) {
            Ok(v) => Ok(v),
            Err(e) => Err(e),
        }
    }

    pub fn call_constructor<'a, I>(
        &self,
        params: I,
        classes: &TsClasses,
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsResult<ObjectValue>
    where
        I: IntoIterator<Item = &'a Value>,
    {
        let constructor = engine_ctx.get_global_dynamic_class(&self.wrapper).unwrap();

        let target = constructor.constructor();
        let args = params
            .into_iter()
            .map(|x| value_to_js_value(classes, x, engine_ctx))
            .collect_vec();
        let obj = self
            .wrapper
            .construct(&boa_engine::JsValue::new(target), &args, engine_ctx)?;
        let new_instance = JsWrapped::<ObjectValue>::get_from_js_obj(&obj)?;

        Ok(new_instance.clone())
    }

    pub fn call_static_method<'a, I>(
        &self,
        method_name: &str,
        param_values: I,
        classes: &TsClasses,
        cache: &Arc<Cache>,
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsResult<Value>
    where
        I: IntoIterator<Item = &'a Value>,
    {
        debug_assert!(self.description.methods[method_name].is_static);

        let constructor = engine_ctx.get_global_dynamic_class(&self.wrapper).unwrap();

        let this = JsValue::new(constructor.prototype());
        let args = param_values
            .into_iter()
            .map(|x| value_to_js_value(classes, x, engine_ctx))
            .collect_vec();

        let result = self
            .wrapper
            .call_static_method_body(method_name, &this, &args, engine_ctx)?;

        Ok(js_value_to_value(classes, &result, engine_ctx, cache))
    }

    pub fn call_method<'a, I>(
        &self,
        method_desc: &MethodDescription,
        this: &Value,
        param_values: I,
        classes: &TsClasses,
        cache: &Arc<Cache>,
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsResult<Value>
    where
        I: IntoIterator<Item = &'a Value>,
    {
        debug_assert!(!method_desc.is_static);
        if this.is_null() {
            return Err(js_error_null_this());
        }

        let this = value_to_js_value(classes, &this, engine_ctx);
        let args = param_values
            .into_iter()
            .map(|x| value_to_js_value(classes, x, engine_ctx))
            .collect_vec();

        let method_name = match method_desc.kind {
            MethodKind::Method => method_desc.name.as_str(),
            MethodKind::GlobalFunction => method_desc.name.as_str(),
            MethodKind::Getter => &JsObjectWrapper::getter_name(&method_desc.name),
            MethodKind::Setter => &JsObjectWrapper::setter_name(&method_desc.name),
        };

        let result = self
            .wrapper
            .call_method_body(method_name, &this, &args, engine_ctx)?;

        Ok(js_value_to_value(classes, &result, engine_ctx, cache))
    }

    pub fn static_object_value(&self) -> Option<ObjectValue> {
        let static_graph = self.static_graph.as_ref()?;
        Some(ObjectValue {
            obj_type: self.static_graph_obj_type.clone(),
            graph_id: static_graph.id,
            node: self.root_node,
        })
    }

    pub(crate) fn register_class(&self, engine_ctx: &mut EngineContext<'_>) -> JsResult<()> {
        engine_ctx.register_global_dynamic_class(&self.wrapper)
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
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsObject {
        let wrapper = JsObjectWrapper::new(self.description.clone());
        wrapper.from_data(obj.into(), engine_ctx).unwrap()
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

pub(crate) struct TsUserClassBuilder {
    id: u64,
    class_name: ClassName,
    fields: HashMap<String, JsFieldDescription>,
    methods: HashMap<String, MethodDescription>,
    constructor: MethodDescription,
    gen_id: Arc<GraphIdGenerator>,
    super_class: Option<ClassName>,
    is_abstract: bool,
}

// Main functions
impl TsUserClassBuilder {
    pub fn get_class_name(namespace: &Vec<String>, decl: &ClassDecl) -> String {
        get_name_with_namespace(namespace, decl.ident.sym.as_str())
    }

    pub fn from_class_decl(
        namespace: &Vec<String>,
        decl: &ClassDecl,
        gen_id: Arc<GraphIdGenerator>,
        cache: &Cache,
    ) -> Self {
        let mut fields = Default::default();
        let mut methods = Default::default();
        let mut constructor = MethodDescription {
            name: scached!(cache; Self::get_class_name(namespace, decl)),
            params: Default::default(),
            kind: MethodKind::Method,
            is_static: false,
            is_private: false,
            body: None,
        };

        Self::add_fields(&decl.class, &mut fields, &mut constructor, cache);
        Self::add_methods(&decl.class, &mut methods, cache);

        let super_class = decl.class.super_class.as_deref().map(|super_class| {
            let ident = super_class.as_ident().unwrap();
            str_cached!(cache; ident.sym.as_str())
        });

        let class = Self {
            id: gen_id.get_id_for_graph() as u64,
            class_name: scached!(cache; Self::get_class_name(namespace, decl)),
            fields,
            methods,
            constructor,
            gen_id,
            super_class,
            is_abstract: decl.class.is_abstract,
        };

        class
    }

    pub fn finalize(self, classes: &mut TsClasses, graphs_map: &mut GraphsMap, cache: &Arc<Cache>) {
        let description = Arc::new(TsClassDescription {
            id: self.id,
            class_name: self.class_name.clone(),
            fields: self.fields.clone(),
            constructor: self.constructor.clone(),
            methods: self.methods.clone(),
            extends: self.super_class.clone(),
            is_abstract: self.is_abstract,
        });

        let static_graph_obj_type =
            StaticGraphBuilder::static_graph_obj_type(&description.class_name, cache);
        let wrapper = JsObjectWrapper::new(description.clone());

        let class = TsUserClass {
            description,
            member_opcodes: Default::default(),
            method_opcodes: Default::default(),
            constructor_opcodes: Default::default(),
            static_graph: None,
            root_node: classes.static_classes_gen_id.get_id_for_node(),
            static_fields: Default::default(),
            static_graph_obj_type,
            wrapper,
        };
        let root_node = class.root_node;
        classes.add_user_class(class.description.class_name.to_string(), class);

        if self.fields.values().any(|field| field.is_static) {
            let static_graph_builder = StaticGraphBuilder {
                id: self.id,
                class_name: self.class_name.as_str(),
                gen_id: &self.gen_id,
            };
            let graph = static_graph_builder.populate_static_graph(
                self.fields.values(),
                root_node,
                graphs_map,
                classes,
                cache,
            );
            classes
                .get_user_class_mut(&self.class_name)
                .unwrap()
                .static_graph = Some(graph);
        }

        self.populate_opcodes(classes.get_user_class_mut(&self.class_name).unwrap(), cache);
    }
}

// Field parsing
impl TsUserClassBuilder {
    fn add_fields(
        class: &ast::Class,
        fields: &mut HashMap<String, JsFieldDescription>,
        constructor: &mut MethodDescription,
        cache: &Cache,
    ) {
        for class_prop in class.body.iter().filter_map(|x| x.as_class_prop()) {
            let desc = JsFieldDescription::from_with_cache(class_prop, cache);
            fields.insert(desc.name.to_string(), desc);
        }

        for constructor_ast in class.body.iter().filter_map(|x| x.as_constructor()) {
            Self::add_fields_from_constructor(fields, constructor, constructor_ast, cache);
        }
    }

    fn add_fields_from_constructor(
        fields: &mut HashMap<String, JsFieldDescription>,
        constructor: &mut MethodDescription,
        constructor_ast: &ast::Constructor,
        cache: &Cache,
    ) {
        constructor.is_private = constructor_ast
            .accessibility
            .unwrap_or(ast::Accessibility::Public)
            != ast::Accessibility::Public;
        for param in &constructor_ast.params {
            let param_description = ParamDescription::from_with_cache(param, cache);

            constructor.params.push(param_description.clone());

            if let Some(prop) = param.as_ts_param_prop() {
                let access = prop.accessibility.unwrap_or(ast::Accessibility::Public);
                fields.insert(
                    param_description.name.to_string(),
                    JsFieldDescription {
                        name: param_description.name.clone(),
                        value_type: param_description.value_type,
                        is_private: access != ast::Accessibility::Public,
                        is_readonly: prop.readonly,
                        is_static: false,
                        is_constructor_prop: true,
                        init_expr: None,
                    },
                );
            }
        }
        constructor.body = Some(get_code(constructor_ast.body.as_ref().unwrap()));
    }

    fn add_methods(
        class: &ast::Class,
        methods: &mut HashMap<String, MethodDescription>,
        cache: &Cache,
    ) {
        for method in class.body.iter().filter_map(|x| x.as_method()) {
            let desc = MethodDescription::from_with_cache(method, cache);
            if !desc.is_private {
                assert!(desc
                    .params
                    .iter()
                    .all(|p| !matches!(p.value_type, ValueType::Null)));
            }
            methods.insert(desc.name.to_string(), desc);
        }
    }
}

// Opcodes
impl TsUserClassBuilder {
    fn populate_opcodes(&self, class: &mut TsUserClass, cache: &Cache) {
        for field in self.fields.values() {
            if let Some(op) = self.get_opcode_from_field(class, field, cache) {
                class.member_opcodes.push(op);
            }
        }

        class.method_opcodes.extend(
            self.methods
                .values()
                .filter_map(|m| self.get_method_opcode(m)),
        );

        if !self.is_abstract && !self.constructor.is_private {
            class
                .constructor_opcodes
                .push(Arc::new(ClassConstructorOp::new(
                    self.class_name.clone(),
                    self.constructor.clone(),
                )));
        }
    }

    fn get_opcode_from_field(
        &self,
        class: &TsUserClass,
        field: &JsFieldDescription,
        cache: &Cache,
    ) -> Option<Arc<dyn ExprOpcode>> {
        if field.is_private {
            return None;
        }

        let op: Arc<dyn ExprOpcode> = if field.is_static {
            let loc_val = class.static_fields[&field.name].clone();
            Arc::new(StaticMemberOp::new(
                self.class_name.clone(),
                str_cached!(cache; &field.name),
                class.static_graph.as_ref().unwrap().clone(),
                loc_val,
            ))
        } else {
            Arc::new(MemberOp::new(
                self.class_name.clone(),
                str_cached!(cache; &field.name),
            ))
        };

        return Some(op);
    }

    fn get_method_opcode(&self, method: &MethodDescription) -> Option<Arc<dyn ExprOpcode>> {
        if method.is_private {
            return None;
        }

        Some(Arc::new(ClassMethodOp::new(
            self.class_name.clone(),
            method,
        )))
    }
}
