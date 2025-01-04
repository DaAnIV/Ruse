use std::{
    collections::HashMap,
    io::Read,
    ops::{Deref, DerefMut},
    sync::Arc,
};

use dashmap::DashMap;
use ruse_object_graph::{
    scached, str_cached, Cache, CachedString, FieldName, ObjectGraph, RootName,
};
use ruse_synthesizer::{
    context::{Context, GraphIdGenerator},
    opcode::OpcodesList,
};
use swc_common::{
    errors::{ColorConfig, Handler},
    FileName, SourceMap,
};
use swc_ecma_ast::{self as ast, ClassDecl};

use anyhow::Error;
use ruse_object_graph::value::*;
use swc_ecma_parser::{Syntax, TsSyntax};

use crate::{
    js_value::{js_value_to_value, value_to_js_value},
    opcode::{ClassMethodOp, MemberOp},
};

#[derive(Clone, Debug)]
pub struct TsClasses {
    classes: Arc<DashMap<CachedString, Arc<TsClass>>>,
}

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
    pub fn new() -> Self {
        Self {
            classes: Default::default(),
        }
    }

    pub fn add_class(&self, code: &str, cache: &Cache) -> Result<CachedString, Error> {
        let class = TsClass::from_code(self, code, cache)?;
        let class_name = class.class_name.clone();
        self.classes.insert(class_name.clone(), class.into());
        Ok(class_name.clone())
    }

    pub fn get_class(&self, class: &CachedString) -> Option<Arc<TsClass>> {
        Option::map(self.classes.get(class), |x| x.clone())
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

    pub fn add_ts_file(
        &self,
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
                let class = TsClass::from_class_decl(self, class_decl, cache)?;
                let class_name = class.class_name.clone();
                self.classes.insert(class_name.clone(), class.into());
                class_names.push(class_name.clone());
            }
        }

        Ok(class_names)
    }
}

impl Default for TsClasses {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct TsClass {
    class: Box<ast::Class>,
    pub class_name: CachedString,
    pub fields: HashMap<CachedString, ValueType>,
    pub member_opcodes: OpcodesList,
    pub method_opcodes: OpcodesList,
}

impl TsClass {
    fn from_code(classes: &TsClasses, code: &str, cache: &Cache) -> Result<Self, Error> {
        let script = TsClass::get_ast(code)?.script().unwrap();

        let class_decl = script.body[0].as_decl().unwrap().as_class().unwrap();

        Self::from_class_decl(classes, class_decl, cache)
    }

    fn from_class_decl(
        classes: &TsClasses,
        decl: &ClassDecl,
        cache: &Cache,
    ) -> Result<Self, Error> {
        let mut class = Self {
            class: decl.class.clone(),
            class_name: str_cached!(cache; decl.ident.sym.as_str()),
            fields: Default::default(),
            member_opcodes: Default::default(),
            method_opcodes: Default::default(),
        };

        class.populate_opcodes(classes, cache);

        Ok(class)
    }

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
        for member in self.class.body.clone().iter() {
            if let ast::ClassMember::Constructor(constructor) = member {
                self.add_accessors_from_constructor(&mut builder, classes, constructor, cache);
            };
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
            let field_name = ts_param.param.as_ident().unwrap().sym.as_str();
            if ts_param.accessibility.is_none() {
                continue;
            }
            let attibute = match ts_param.readonly {
                true => boa_engine::property::Attribute::READONLY,
                false => boa_engine::property::Attribute::WRITABLE,
            };

            let getter_field_name = str_cached!(cache; field_name);
            let getter_classes = classes.clone();
            let getter = unsafe {
                boa_engine::native_function::NativeFunction::from_closure(
                    move |this, _, boa_ctx| {
                        getter_classes.object_getter(this, &getter_field_name, boa_ctx)
                    },
                )
                .to_js_function(obj_initializer.context().realm())
            };

            if ts_param.readonly {
                obj_initializer.accessor(
                    boa_engine::js_string!(field_name),
                    Some(getter),
                    None,
                    boa_engine::property::Attribute::READONLY,
                );
            } else {
                let setter_field_name = str_cached!(cache; field_name);
                let setter_classes = classes.clone();
                let setter = unsafe {
                    boa_engine::native_function::NativeFunction::from_closure(
                        move |this, args, boa_ctx| {
                            setter_classes.object_setter(
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
                    attibute,
                );
            }
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

    fn populate_opcodes(&mut self, classes: &TsClasses, cache: &Cache) {
        for member in self.class.body.clone().iter() {
            match member {
                ast::ClassMember::Constructor(constructor) => {
                    self.add_opcodes_from_constructor(constructor, cache);
                }
                ast::ClassMember::Method(m) => self.add_method_opcode(classes, m, cache),
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
        let member_type = Self::get_value_type(&prop.type_ann.as_ref().unwrap().type_ann, cache);
        if let Some(_field_type) = self.fields.get(&member) {
            // TODO: check field type matches
            return;
        }
        self.fields.insert(member.clone(), member_type);

        let accessor = Arc::new(MemberOp::new(self.class_name.clone(), member));
        self.member_opcodes.push(accessor);
    }

    fn add_method_opcode(&mut self, classes: &TsClasses, method: &ast::ClassMethod, cache: &Cache) {
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
            classes.clone(),
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
