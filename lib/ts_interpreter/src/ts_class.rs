use std::{
    collections::HashMap,
    io::Read,
    ops::{Deref, DerefMut},
    sync::Arc,
};

use ruse_object_graph::{
    fields, scached, str_cached, vbool, vnull, vnum, vobj, vstring, Attributes, Cache,
    CachedString, FieldName, NodeIndex, ObjectGraph, PrimitiveValue, RootName,
};
use ruse_synthesizer::{
    context::{Context, GraphIdGenerator, SynthesizerContextData},
    location::{LocValue, Location, ObjectFieldLoc},
    opcode::{ExprOpcode, OpcodesList},
};
use swc_common::{
    errors::{ColorConfig, Handler},
    FileName, SourceMap, DUMMY_SP,
};
use swc_ecma_ast::{self as ast, ClassDecl, ClassProp};

use anyhow::Error;
use ruse_object_graph::value::*;
use swc_ecma_parser::{Syntax, TsSyntax};

use crate::{
    js_value::{js_value_to_value, value_to_js_value},
    opcode::{ClassMethodOp, MemberOp, StaticMemberOp},
};

#[derive(Debug)]
pub struct TsClasses {
    pub static_classes_gen_id: Arc<GraphIdGenerator>,
    classes: HashMap<CachedString, TsClass>,
}

impl SynthesizerContextData for TsClasses {}

struct TsContextHooks;

impl boa_engine::context::HostHooks for TsContextHooks {
    fn create_global_object(
        &self,
        intrinsics: &boa_engine::context::intrinsics::Intrinsics,
    ) -> boa_engine::prelude::JsObject {
        let global_obj = TsGlobalObject {
            cache: None,
            context: None,
            dirty: false,
        };
        boa_engine::JsObject::from_proto_and_data(
            intrinsics.constructors().object().prototype(),
            global_obj,
        )
    }
}

pub struct EngineContext(boa_engine::Context);

impl EngineContext {
    fn set_context(&mut self, ctx: &mut Context, cache: &Arc<Cache>) {
        let global_obj = self.0.global_object();
        let mut a = global_obj.downcast_mut::<TsGlobalObject>().unwrap();
        let b = a.deref_mut();
        b.context = Some(std::ptr::from_mut(ctx));
        b.cache = Some(cache.clone());
        b.dirty = false
    }

    pub fn is_dirty(&self) -> bool {
        let global_obj = self.0.global_object();
        let a = global_obj.downcast_ref::<TsGlobalObject>().unwrap();
        a.dirty
    }
}

impl Deref for EngineContext {
    type Target = boa_engine::Context;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for EngineContext {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Default for EngineContext {
    fn default() -> Self {
        Self(
            boa_engine::context::ContextBuilder::default()
                .host_hooks(&TsContextHooks)
                .build()
                .expect("Failed to build context"),
        )
    }
}

impl TsClasses {
    pub fn get_class(&self, class: &CachedString) -> Option<&TsClass> {
        self.classes.get(class)
    }

    pub fn get_engine_ctx(&self, post_ctx: &mut Context, cache: &Arc<Cache>) -> EngineContext {
        let mut boa_ctx = EngineContext::default();
        boa_ctx.set_context(post_ctx, cache);
        boa_ctx
    }

    fn object_getter(
        &self,
        this: &boa_engine::JsValue,
        field_name: &CachedString,
        boa_ctx: &mut boa_engine::Context,
    ) -> boa_engine::JsResult<boa_engine::JsValue> {
        let js_object_value = this
            .as_object()
            .unwrap()
            .downcast_ref::<TsObjectValue>()
            .unwrap();
        let global_obj = boa_ctx.global_object();
        let global_ctx = global_obj.downcast_ref::<TsGlobalObject>().unwrap();
        let cache = global_ctx.cache();
        let context = global_ctx.context();

        let field = js_object_value
            .get_field_value(field_name, &context.graphs_map)
            .unwrap();

        Ok(value_to_js_value(self, &field, boa_ctx, context, cache))
    }

    fn object_setter(
        &self,
        this: &boa_engine::JsValue,
        field_name: &CachedString,
        new_js_value: &boa_engine::JsValue,
        boa_ctx: &mut boa_engine::Context,
    ) {
        let js_object_value = this
            .as_object()
            .unwrap()
            .downcast_ref::<TsObjectValue>()
            .unwrap();
        let global_obj = boa_ctx.global_object();
        let mut global_ctx = global_obj.downcast_mut::<TsGlobalObject>().unwrap();
        global_ctx.set_dirty();
        let cache = global_ctx.cache();
        let context = global_ctx.context();

        let new_value = js_value_to_value(self, new_js_value, boa_ctx, cache);

        context.set_field(
            js_object_value.graph_id,
            js_object_value.node,
            field_name.clone(),
            &new_value,
        );
    }
}

#[derive(Debug)]
pub struct TsClass {
    class: Box<ast::Class>,
    pub class_name: CachedString,
    pub fields: HashMap<CachedString, ValueType>,
    pub member_opcodes: OpcodesList,
    pub method_opcodes: OpcodesList,
    pub static_graph: Arc<ObjectGraph>,
    pub static_fields: HashMap<CachedString, (ValueType, LocValue)>,
}

impl TsClass {
    pub fn obj_type(&self) -> ValueType {
        ValueType::Object(self.class_name.clone())
    }

    pub fn generate_object<I>(
        &self,
        map: I,
        graph: &mut ObjectGraph,
        graph_id_gen: &GraphIdGenerator,
    ) -> ObjectValue
    where
        I: IntoIterator<Item = (FieldName, Value)>,
    {
        // TODO: Check map set attributes etc...

        let obj_id =
            graph.add_object_from_map(graph_id_gen.get_id_for_node(), self.class_name.clone(), map);

        ObjectValue {
            graph_id: graph.id,
            node: obj_id,
        }
    }

    pub fn generate_rooted_object<I>(
        &self,
        root_name: RootName,
        map: I,
        graph: &mut ObjectGraph,
        graph_id_gen: &GraphIdGenerator,
    ) -> ObjectValue
    where
        I: IntoIterator<Item = (CachedString, Value)>,
    {
        let val = self.generate_object(map, graph, graph_id_gen);
        graph.set_as_root(root_name, val.node);

        val
    }

    pub fn generate_js_object(
        &self,
        classes: &TsClasses,
        obj: ObjectValue,
        boa_ctx: &mut boa_engine::Context,
        cache: &Cache,
    ) -> boa_engine::JsObject {
        let mut builder =
            boa_engine::object::ObjectInitializer::with_native_data(TsObjectValue(obj), boa_ctx);
        for member in self.class.body.iter() {
            match member {
                swc_ecma_ast::ClassMember::Constructor(constructor) => {
                    self.add_accessors_from_constructor(&mut builder, classes, constructor, cache)
                }
                swc_ecma_ast::ClassMember::ClassProp(class_prop) => {
                    self.add_accessors_from_prop(&mut builder, classes, class_prop, cache)
                }
                _ => (),
            }
        }
        builder.build()
    }

    fn add_accessors_from_constructor(
        &self,
        obj_initializer: &mut boa_engine::object::ObjectInitializer,
        classes: &TsClasses,
        constructor: &ast::Constructor,
        cache: &Cache,
    ) {
        for param in &constructor.params {
            let ts_param = param.as_ts_param_prop().unwrap();
            let ident = ts_param.param.as_ident().unwrap();
            let prop = ast::ClassProp {
                span: DUMMY_SP,
                key: ast::PropName::Ident(ident.sym.clone().into()),
                value: None,
                type_ann: ident.type_ann.clone(),
                is_static: false,
                decorators: ts_param.decorators.clone(),
                accessibility: ts_param.accessibility,
                is_abstract: false,
                is_optional: false,
                is_override: false,
                readonly: ts_param.readonly,
                declare: false,
                definite: false,
            };
            self.add_accessors_from_prop(obj_initializer, classes, &prop, cache);
        }
    }

    fn add_accessors_from_prop(
        &self,
        obj_initializer: &mut boa_engine::object::ObjectInitializer<'_>,
        classes: &TsClasses,
        prop: &ast::ClassProp,
        cache: &Cache,
    ) {
        let ident = prop.key.as_ident().unwrap();
        let field_name = ident.sym.as_str();

        if prop.is_static {
            return;
        }
        if prop.accessibility.unwrap_or(ast::Accessibility::Public) != ast::Accessibility::Public {
            return;
        }
        let attribute = match prop.readonly {
            true => boa_engine::property::Attribute::READONLY,
            false => boa_engine::property::Attribute::WRITABLE,
        };

        let getter_field_name = str_cached!(cache; field_name);
        let getter_classes = std::ptr::from_ref(classes);
        let getter = unsafe {
            boa_engine::native_function::NativeFunction::from_closure(move |this, _, boa_ctx| {
                getter_classes
                    .as_ref()
                    .unwrap()
                    .object_getter(this, &getter_field_name, boa_ctx)
            })
            .to_js_function(obj_initializer.context().realm())
        };

        if prop.readonly {
            obj_initializer.accessor(
                boa_engine::js_string!(field_name),
                Some(getter),
                None,
                boa_engine::property::Attribute::READONLY,
            );
        } else {
            let setter_field_name = str_cached!(cache; field_name);
            let setter_classes = std::ptr::from_ref(classes);
            let setter = unsafe {
                boa_engine::native_function::NativeFunction::from_closure(
                    move |this, args, boa_ctx| {
                        setter_classes.as_ref().unwrap().object_setter(
                            this,
                            &setter_field_name,
                            &args[0],
                            boa_ctx,
                        );
                        Ok(boa_engine::JsValue::Undefined)
                    },
                )
                .to_js_function(obj_initializer.context().realm())
            };

            obj_initializer.accessor(
                boa_engine::js_string!(field_name),
                Some(getter),
                Some(setter),
                attribute,
            );
        }
    }

    fn get_ast(code: &str) -> Result<ast::Program, Error> {
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
}

pub struct TsClassesBuilder {
    classes: HashMap<CachedString, ClassDecl>,
}

impl TsClassesBuilder {
    pub fn new() -> Self {
        Self {
            classes: Default::default(),
        }
    }

    pub fn add_class(&mut self, code: &str, cache: &Cache) -> Result<CachedString, Error> {
        let class_decl = TsClassBuilder::get_class_decl(code)?;
        let class_name = TsClassBuilder::get_class_name(&class_decl, cache);
        self.classes.insert(class_name.clone(), class_decl);
        Ok(class_name.clone())
    }

    pub fn add_ts_file(
        &mut self,
        full_path: &std::path::PathBuf,
        cache: &Cache,
    ) -> Result<Vec<CachedString>, Error> {
        let cm = Arc::<SourceMap>::default();
        let handler = Handler::with_tty_emitter(ColorConfig::Auto, true, false, Some(cm.clone()));
        let c = swc::Compiler::new(cm.clone());

        let mut file = std::fs::File::open(full_path)?;
        let mut src = String::new();
        file.read_to_string(&mut src)?;
        let fm = cm.new_source_file(FileName::Real(full_path.clone()).into(), src);

        let script = c
            .parse_js(
                fm,
                &handler,
                ast::EsVersion::Es2022,
                Syntax::Typescript(TsSyntax::default()),
                swc::config::IsModule::Bool(false),
                None,
            )?
            .script()
            .unwrap();

        let mut class_names = vec![];

        for stmt in &script.body {
            if let ast::Stmt::Decl(ast::Decl::Class(class_decl)) = stmt {
                let class_name = TsClassBuilder::get_class_name(class_decl, cache);
                self.classes.insert(class_name.clone(), class_decl.clone());
                class_names.push(class_name.clone());
            }
        }

        Ok(class_names)
    }

    pub fn finalize(self, cache: &Cache) -> Box<TsClasses> {
        let mut classes = HashMap::default();
        let gen_id = Arc::new(GraphIdGenerator::default());
        for class_decl in self.classes.values() {
            let class = TsClassBuilder::from_class_decl(class_decl, &gen_id, cache)
                .finalize(&self.classes, cache);
            classes.insert(class.class_name.clone(), class);
        }

        Box::new(TsClasses {
            static_classes_gen_id: gen_id,
            classes,
        })
    }
}

struct TsClassBuilder<'a> {
    class: Box<ast::Class>,
    member_opcodes: OpcodesList,
    method_opcodes: OpcodesList,
    class_name: CachedString,
    fields: HashMap<CachedString, ValueType>,
    gen_id: &'a GraphIdGenerator,
    static_graph: Arc<ObjectGraph>,
    root_node: NodeIndex,
    static_fields: HashMap<CachedString, (ValueType, LocValue)>,
}

impl<'a> TsClassBuilder<'a> {
    fn get_class_decl(code: &str) -> Result<ClassDecl, Error> {
        let script = TsClass::get_ast(code)?.script().unwrap();
        Ok(script.body[0]
            .as_decl()
            .unwrap()
            .as_class()
            .unwrap()
            .clone())
    }

    fn get_class_name(decl: &ClassDecl, cache: &Cache) -> CachedString {
        str_cached!(cache; decl.ident.sym.as_str())
    }

    fn from_class_decl(decl: &ClassDecl, gen_id: &'a GraphIdGenerator, cache: &Cache) -> Self {
        let class = Self {
            class: decl.class.clone(),
            class_name: Self::get_class_name(decl, cache),
            fields: Default::default(),
            member_opcodes: Default::default(),
            method_opcodes: Default::default(),
            static_graph: ObjectGraph::new_static(gen_id.get_id_for_graph()).into(),
            root_node: gen_id.get_id_for_node(),
            static_fields: Default::default(),
            gen_id,
        };

        class
    }

    fn finalize(mut self, classes: &HashMap<CachedString, ClassDecl>, cache: &Cache) -> TsClass {
        self.generate_static_graph(classes, cache);
        self.populate_opcodes(cache);

        TsClass {
            class: self.class,
            class_name: self.class_name,
            fields: self.fields,
            member_opcodes: self.member_opcodes,
            method_opcodes: self.method_opcodes,
            static_graph: self.static_graph,
            static_fields: self.static_fields,
        }
    }

    fn populate_opcodes(&mut self, cache: &Cache) {
        for member in self.class.body.clone().iter() {
            match member {
                ast::ClassMember::Constructor(constructor) => {
                    self.add_opcodes_from_constructor(constructor, cache);
                }
                ast::ClassMember::Method(m) => self.add_method_opcode(m, cache),
                ast::ClassMember::ClassProp(prop) => self.add_opcodes_from_prop(prop, cache),
                ast::ClassMember::TsIndexSignature(_) => todo!(),
                ast::ClassMember::StaticBlock(_) => todo!(),
                ast::ClassMember::AutoAccessor(_) => todo!(),
                _ => continue,
            };
        }
    }

    fn get_value_type(type_ann: &ast::TsType, cache: &Cache) -> ValueType {
        match type_ann {
            ast::TsType::TsKeywordType(t) => match t.kind {
                swc_ecma_ast::TsKeywordTypeKind::TsAnyKeyword => todo!(),
                swc_ecma_ast::TsKeywordTypeKind::TsUnknownKeyword => todo!(),
                swc_ecma_ast::TsKeywordTypeKind::TsNumberKeyword => ValueType::Number,
                swc_ecma_ast::TsKeywordTypeKind::TsObjectKeyword => todo!(),
                swc_ecma_ast::TsKeywordTypeKind::TsBooleanKeyword => ValueType::Bool,
                swc_ecma_ast::TsKeywordTypeKind::TsBigIntKeyword => todo!(),
                swc_ecma_ast::TsKeywordTypeKind::TsStringKeyword => ValueType::String,
                swc_ecma_ast::TsKeywordTypeKind::TsSymbolKeyword => todo!(),
                swc_ecma_ast::TsKeywordTypeKind::TsVoidKeyword => todo!(),
                swc_ecma_ast::TsKeywordTypeKind::TsUndefinedKeyword => todo!(),
                swc_ecma_ast::TsKeywordTypeKind::TsNullKeyword => todo!(),
                swc_ecma_ast::TsKeywordTypeKind::TsNeverKeyword => todo!(),
                swc_ecma_ast::TsKeywordTypeKind::TsIntrinsicKeyword => todo!(),
            },
            ast::TsType::TsThisType(_) => todo!(),
            ast::TsType::TsFnOrConstructorType(_) => todo!(),
            ast::TsType::TsTypeRef(t) => {
                let id = t.type_name.as_ident().unwrap().sym.to_string();
                ValueType::Object(scached!(cache; id))
            }
            ast::TsType::TsTypeQuery(_) => todo!(),
            ast::TsType::TsTypeLit(_) => todo!(),
            ast::TsType::TsArrayType(t) => {
                let elem_type = Self::get_value_type(t.elem_type.as_ref(), cache);
                ValueType::array_value_type(&elem_type, cache)
            }
            ast::TsType::TsTupleType(_) => todo!(),
            ast::TsType::TsOptionalType(_) => todo!(),
            ast::TsType::TsRestType(_) => todo!(),
            ast::TsType::TsUnionOrIntersectionType(_) => todo!(),
            ast::TsType::TsConditionalType(_) => todo!(),
            ast::TsType::TsInferType(_) => todo!(),
            ast::TsType::TsParenthesizedType(_) => todo!(),
            ast::TsType::TsTypeOperator(_) => todo!(),
            ast::TsType::TsIndexedAccessType(_) => todo!(),
            ast::TsType::TsMappedType(_) => todo!(),
            ast::TsType::TsLitType(_) => todo!(),
            ast::TsType::TsTypePredicate(_) => todo!(),
            ast::TsType::TsImportType(_) => todo!(),
        }
    }

    fn add_opcodes_from_constructor(&mut self, constructor: &ast::Constructor, cache: &Cache) {
        for param in &constructor.params {
            let ts_param = param.as_ts_param_prop().unwrap();
            if ts_param
                .accessibility
                .unwrap_or(ast::Accessibility::Private)
                != ast::Accessibility::Public
            {
                continue;
            }

            let ident = ts_param.param.as_ident().unwrap();

            let member = str_cached!(cache; ident.sym.as_str());
            let member_type =
                Self::get_value_type(&ident.type_ann.as_ref().unwrap().type_ann, cache);
            if let Some(_field_type) = self.fields.get(&member) {
                // TODO: check field type matches
                continue;
            }
            self.fields.insert(member.clone(), member_type);

            let accessor = Arc::new(MemberOp::new(self.class_name.clone(), member));
            self.member_opcodes.push(accessor);
        }
    }

    fn add_opcodes_from_prop(&mut self, prop: &ast::ClassProp, cache: &Cache) {
        if prop.accessibility.unwrap_or(ast::Accessibility::Public) != ast::Accessibility::Public {
            return;
        }
        let ident = prop.key.as_ident().unwrap();
        let member = str_cached!(cache; ident.sym.as_str());

        let accessor: Arc<dyn ExprOpcode> = if prop.is_static {
            let loc_val = self.static_fields[&member].1.clone();
            Arc::new(StaticMemberOp::new(
                self.class_name.clone(),
                member,
                self.static_graph.clone().into(),
                loc_val,
            ))
        } else {
            let member_type =
                Self::get_value_type(&prop.type_ann.as_ref().unwrap().type_ann, cache);
            if let Some(_field_type) = self.fields.get(&member) {
                // TODO: check field type matches
                return;
            }
            self.fields.insert(member.clone(), member_type);
            Arc::new(MemberOp::new(self.class_name.clone(), member))
        };
        self.member_opcodes.push(accessor);
    }

    fn add_method_opcode(&mut self, method: &ast::ClassMethod, cache: &Cache) {
        let method_name = method.key.as_ident().unwrap().sym.to_string();
        let mut args = Vec::with_capacity(method.function.params.len());

        let c = swc::Compiler::new(Arc::<SourceMap>::default());

        let codegen_config =
            swc_ecma_codegen::Config::default().with_target(ast::EsVersion::Es2022);

        let print_args = swc::PrintArgs {
            source_root: None,
            source_file_name: None,
            output_path: None,
            inline_sources_content: false,
            source_map: swc::config::SourceMapsConfig::Bool(false),
            source_map_names: &Default::default(),
            orig: None,
            comments: None,
            emit_source_map_columns: false,
            preamble: "",
            codegen_config,
            output: None,
        };
        let function_body = c
            .print(method.function.body.as_ref().unwrap(), print_args)
            .expect("Failed to get code")
            .code;
        for param in &method.function.params {
            args.push(Self::pat_to_name_type(&param.pat, cache));
        }
        let method_op = Arc::new(ClassMethodOp::new(
            self.class_name.clone(),
            method_name,
            &args,
            function_body.as_str(),
            method.is_static,
        ));
        self.method_opcodes.push(method_op);
    }

    fn pat_to_name_type(pat: &ast::Pat, cache: &Cache) -> (String, ValueType) {
        match pat {
            swc_ecma_ast::Pat::Ident(binding_ident) => (
                binding_ident.id.sym.to_string(),
                Self::get_value_type(&binding_ident.type_ann.as_ref().unwrap().type_ann, cache),
            ),
            swc_ecma_ast::Pat::Assign(assign_pat) => {
                Self::pat_to_name_type(&assign_pat.left, cache)
            }
            _ => todo!(),
        }
    }

    fn generate_object_from_constructor<I>(
        decl: &ClassDecl,
        class_name: CachedString,
        graph: &mut ObjectGraph,
        node: NodeIndex,
        params: I,
        classes: &HashMap<CachedString, ClassDecl>,
        gen_id: &GraphIdGenerator,
        cache: &Cache,
    ) -> ObjectValue
    where
        I: IntoIterator<Item = Value> + Clone,
    {
        graph.construct_node(node, class_name, fields!());

        for member in decl.class.body.iter() {
            match &member {
                swc_ecma_ast::ClassMember::Constructor(constructor) => {
                    Self::add_fields_from_constructor(
                        graph,
                        node,
                        constructor,
                        params.clone(),
                        cache,
                    )
                }
                swc_ecma_ast::ClassMember::ClassProp(class_prop) => {
                    if class_prop.is_static {
                        continue;
                    }
                    let _ =
                        Self::add_field_from_prop(graph, node, class_prop, classes, gen_id, cache);
                }
                _ => (),
            }
        }

        ObjectValue {
            graph_id: graph.id,
            node: node,
        }
    }

    fn add_fields_from_constructor<I>(
        graph: &mut ObjectGraph,
        node: NodeIndex,
        constructor: &ast::Constructor,
        params: I,
        cache: &Cache,
    ) where
        I: IntoIterator<Item = Value>,
    {
        for (param, param_value) in constructor.params.iter().zip(params) {
            let ts_param = param.as_ts_param_prop().unwrap();
            let ident = ts_param.param.as_ident().unwrap();
            let field = str_cached!(cache; ident.sym.as_str());
            match param_value {
                Value::Primitive(primitive_value) => {
                    graph.set_field(&node, field.clone(), primitive_value)
                }
                Value::Object(object_value) => {
                    graph.set_edge(&node, object_value.node, field.clone());
                }
            };
        }
    }

    fn generate_static_graph(&mut self, classes: &HashMap<CachedString, ClassDecl>, cache: &Cache) {
        self.add_root_node(cache);

        for member in self.class.body.clone().iter() {
            match member {
                ast::ClassMember::ClassProp(prop) => {
                    if prop.is_static {
                        self.add_static_field_from_prop(prop, classes, cache);
                    }
                }
                ast::ClassMember::StaticBlock(_) => todo!(),
                _ => continue,
            };
        }
    }

    fn add_static_field_from_prop(
        &mut self,
        prop: &ClassProp,
        classes: &HashMap<CachedString, ClassDecl>,
        cache: &Cache,
    ) {
        debug_assert!(prop.is_static);

        let graph = Arc::get_mut(&mut self.static_graph).unwrap();
        if let Ok((val, value_type)) =
            Self::add_field_from_prop(graph, self.root_node, prop, classes, &self.gen_id, cache)
        {
            let ident = prop.key.as_ident().unwrap();
            let member = str_cached!(cache; ident.sym.as_str());

            let attrs = Attributes {
                readonly: prop.readonly,
            };
            let loc = Location::ObjectField(ObjectFieldLoc {
                graph: self.static_graph.id,
                node: self.root_node,
                field: member.clone(),
                attrs: attrs.clone(),
            });

            self.static_fields
                .insert(member, (value_type, LocValue { loc, val }));
        }
    }

    fn add_root_node(&mut self, cache: &Cache) {
        let graph = Arc::get_mut(&mut self.static_graph).unwrap();
        graph.construct_node(
            self.root_node,
            scached!(cache; self.class_name.to_string() + "Static"),
            fields!(),
        );
        graph.set_as_root(
            scached!(cache; self.class_name.to_string() + "Static"),
            self.root_node,
        );
    }

    fn add_field_from_prop(
        graph: &mut ObjectGraph,
        node: NodeIndex,
        prop: &ast::ClassProp,
        classes: &HashMap<CachedString, ClassDecl>,
        gen_id: &GraphIdGenerator,
        cache: &Cache,
    ) -> Result<(Value, ValueType), ()> {
        if prop.accessibility.unwrap_or(ast::Accessibility::Public) != ast::Accessibility::Public {
            return Err(());
        }
        let ident = prop.key.as_ident().unwrap();
        let member = str_cached!(cache; ident.sym.as_str());

        let attrs = Attributes {
            readonly: prop.readonly,
        };

        let (val, value_type) = Self::get_value_for_prop(graph, prop, classes, gen_id, cache);
        match &val {
            Value::Primitive(primitive_value) => graph.set_field_with_attributes(
                &node,
                member.clone(),
                primitive_value.clone(),
                attrs,
            ),
            Value::Object(object_value) => graph.set_edge(&node, object_value.node, member.clone()),
        }

        Ok((val, value_type))
    }

    fn get_value_for_prop(
        graph: &mut ObjectGraph,
        prop: &ast::ClassProp,
        classes: &HashMap<CachedString, ClassDecl>,
        gen_id: &GraphIdGenerator,
        cache: &Cache,
    ) -> (Value, ValueType) {
        assert!(prop.value.is_some() || prop.type_ann.is_some());

        let member_type = prop
            .type_ann
            .as_ref()
            .map(|type_ann| Self::get_value_type(&type_ann.type_ann, cache));

        if let Some(value) = &prop.value {
            Self::get_value_from_expr(graph, value.as_ref(), classes, gen_id, cache)
        } else {
            let value = match unsafe { member_type.as_ref().unwrap_unchecked() } {
                ValueType::Number => PrimitiveValue::Number(Default::default()),
                ValueType::Bool => PrimitiveValue::Bool(Default::default()),
                ValueType::String => PrimitiveValue::String(Default::default()),
                ValueType::Object(_) => {
                    unreachable!("Don't support static properties of type object without a value")
                }
            };
            (Value::Primitive(value), unsafe {
                member_type.unwrap_unchecked()
            })
        }
    }

    fn get_value_from_expr(
        graph: &mut ObjectGraph,
        expr: &ast::Expr,
        classes: &HashMap<CachedString, ClassDecl>,
        gen_id: &GraphIdGenerator,
        cache: &Cache,
    ) -> (Value, ValueType) {
        match expr {
            ast::Expr::Lit(lit) => match lit {
                ast::Lit::Str(s) => (vstring!(cache; s.value.to_string()), ValueType::String),
                ast::Lit::Bool(bool) => (vbool!(bool.value), ValueType::Number),
                ast::Lit::Null(_) => (vnull!(), ValueType::Number),
                ast::Lit::Num(number) => (vnum!(number.value.into()), ValueType::Number),
                _ => todo!(),
            },
            ast::Expr::New(new) => {
                let id = new.callee.as_ident().unwrap();
                let name = str_cached!(cache; id.sym.as_str());
                let node = gen_id.get_id_for_node();

                let mut params = vec![];
                if let Some(args) = &new.args {
                    for arg in args {
                        params.push(
                            Self::get_value_from_expr(graph, &arg.expr, classes, gen_id, cache).0,
                        );
                    }
                }

                let class_decl = classes.get(&name).unwrap();

                Self::generate_object_from_constructor(
                    class_decl,
                    name.clone(),
                    graph,
                    node,
                    params,
                    classes,
                    gen_id,
                    cache,
                );

                (vobj!(graph.id, node), ValueType::Object(name))
            }
            _ => todo!("Can't parse static prop value {:#?}", expr),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TsObjectValue(ObjectValue);

impl Deref for TsObjectValue {
    type Target = ObjectValue;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl boa_gc::Finalize for TsObjectValue {}

unsafe impl boa_gc::Trace for TsObjectValue {
    boa_gc::empty_trace!();
}

impl boa_engine::JsData for TsObjectValue {}

struct TsGlobalObject {
    cache: Option<Arc<Cache>>,
    context: Option<*mut Context>,
    dirty: bool,
}

impl boa_gc::Finalize for TsGlobalObject {}

unsafe impl boa_gc::Trace for TsGlobalObject {
    boa_gc::empty_trace!();
}

impl boa_engine::JsData for TsGlobalObject {}

impl TsGlobalObject {
    pub fn context(&self) -> &mut Context {
        unsafe { self.context.unwrap().as_mut().unwrap() }
    }
    pub fn cache(&self) -> &Cache {
        self.cache.as_ref().unwrap()
    }
    pub fn set_dirty(&mut self) {
        self.dirty = true;
    }
}
