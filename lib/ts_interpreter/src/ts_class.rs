use std::{collections::HashMap, io::Read, path::Path, sync::Arc};

use boa_engine::{class::DynamicClassBuilder, JsValue};
use itertools::Itertools;
use ruse_object_graph::{
    fields, scached, str_cached, vbool, vnull, vnum, vstring, Attributes, Cache, CachedString,
    FieldName, GraphIndex, GraphsMap, NodeIndex, ObjectGraph, ObjectType, PrimitiveValue, RootName,
};
use ruse_synthesizer::{
    context::{GraphIdGenerator, SynthesizerContextData},
    location::{LocValue, Location, ObjectFieldLoc},
    opcode::{ExprOpcode, OpcodesList},
};
use swc_common::{
    errors::{ColorConfig, Handler},
    FileName, SourceMap,
};
use swc_ecma_ast::{self as ast, BlockStmt, ClassDecl};

use anyhow::Error;
use ruse_object_graph::value::*;
use swc_ecma_parser::{Syntax, TsSyntax};

use crate::{
    js_object_wrapper::{error_null_this, EngineContext, JsObjectWrapper, JsWrapped},
    js_value::{js_value_to_value, value_to_js_value},
    opcode::{ClassMethodOp, MemberOp, StaticMemberOp},
};

#[derive(Debug)]
pub struct TsClasses {
    pub static_classes_gen_id: Arc<GraphIdGenerator>,
    classes: HashMap<CachedString, TsClass>,
}

impl SynthesizerContextData for TsClasses {}

impl TsClasses {
    pub fn get_class(&self, class: &CachedString) -> Option<&TsClass> {
        self.classes.get(class)
    }

    pub fn get_class_mut(&mut self, class: &CachedString) -> Option<&mut TsClass> {
        self.classes.get_mut(class)
    }

    pub(crate) fn init_engine_ctx(&self, engine_ctx: &mut EngineContext<'_>) {
        for class in self.classes.values() {
            engine_ctx
                .register_global_dynamic_class(&class.wrapper)
                .expect("Failed to register dynamic class");
        }
    }
}

trait FromWithCache<T> {
    /// Converts to this type from the input type.
    #[must_use]
    fn from_with_cache(value: T, cache: &Cache) -> Self;
}

#[derive(Debug, Clone)]
pub struct JsFieldDescription {
    pub name: CachedString,
    pub value_type: ValueType,
    pub is_private: bool,
    pub is_static: bool,
    pub is_readonly: bool,
    pub is_constructor_prop: bool,
    init_expr: Option<Box<ast::Expr>>,
}

impl JsFieldDescription {
    pub fn get_primitive_value(&self, cache: &Cache) -> Option<PrimitiveValue> {
        match self.init_expr.as_ref()?.as_ref() {
            ast::Expr::Lit(lit) => match lit {
                ast::Lit::Str(s) => {
                    Some(PrimitiveValue::String(str_cached!(cache; s.value.as_str())))
                }
                ast::Lit::Bool(bool) => Some(PrimitiveValue::Bool(bool.value)),
                ast::Lit::Null(_) => None,
                ast::Lit::Num(number) => Some(PrimitiveValue::Number(number.value.into())),
                _ => todo!(),
            },
            _ => None,
        }
    }
}

impl FromWithCache<&ast::ClassProp> for JsFieldDescription {
    fn from_with_cache(value: &ast::ClassProp, cache: &Cache) -> Self {
        let ident = value.key.as_ident().unwrap();
        let field_name = ident.sym.as_str();
        let access = value.accessibility.unwrap_or(ast::Accessibility::Public);
        let value_type = if let Some(type_ann) = &value.type_ann {
            get_value_type_from_ts_type(&type_ann.type_ann, cache)
        } else {
            get_value_type_from_expr(&value.value.as_ref().unwrap(), cache)
        };

        JsFieldDescription {
            name: str_cached!(cache; field_name),
            value_type,
            is_private: access != ast::Accessibility::Public,
            is_readonly: value.readonly,
            is_static: value.is_static,
            is_constructor_prop: false,
            init_expr: value.value.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParamDescription {
    pub name: CachedString,
    pub value_type: ValueType,
    pub(crate) is_prop: bool,
}

impl FromWithCache<&ast::Param> for ParamDescription {
    fn from_with_cache(value: &ast::Param, cache: &Cache) -> Self {
        match &value.pat {
            swc_ecma_ast::Pat::Ident(binding_ident) => ParamDescription {
                name: str_cached!(cache; binding_ident.id.sym.as_str()),
                value_type: get_value_type_from_ts_type(
                    &binding_ident.type_ann.as_ref().unwrap().type_ann,
                    cache,
                ),
                is_prop: false,
            },
            swc_ecma_ast::Pat::Assign(assign_pat) => {
                let ident = assign_pat.left.as_ident().unwrap();
                let value_type = if let Some(type_ann) = &ident.type_ann {
                    get_value_type_from_ts_type(&type_ann.type_ann, cache)
                } else {
                    get_value_type_from_expr(&assign_pat.right, cache)
                };
                ParamDescription {
                    name: str_cached!(cache; ident.id.sym.as_str()),
                    value_type: value_type,
                    is_prop: false,
                }
            }
            _ => todo!(),
        }
    }
}

impl FromWithCache<&ast::TsParamProp> for ParamDescription {
    fn from_with_cache(value: &ast::TsParamProp, cache: &Cache) -> Self {
        match &value.param {
            swc_ecma_ast::TsParamPropParam::Ident(binding_ident) => ParamDescription {
                name: str_cached!(cache; binding_ident.id.sym.as_str()),
                value_type: get_value_type_from_ts_type(
                    &binding_ident.type_ann.as_ref().unwrap().type_ann,
                    cache,
                ),
                is_prop: true,
            },
            swc_ecma_ast::TsParamPropParam::Assign(assign_pat) => {
                let ident = assign_pat.left.as_ident().unwrap();
                let value_type = if let Some(type_ann) = &ident.type_ann {
                    get_value_type_from_ts_type(&type_ann.type_ann, cache)
                } else {
                    get_value_type_from_expr(&assign_pat.right, cache)
                };
                ParamDescription {
                    name: str_cached!(cache; ident.id.sym.as_str()),
                    value_type: value_type,
                    is_prop: true,
                }
            }
        }
    }
}

impl FromWithCache<&ast::ParamOrTsParamProp> for ParamDescription {
    fn from_with_cache(value: &ast::ParamOrTsParamProp, cache: &Cache) -> Self {
        match value {
            swc_ecma_ast::ParamOrTsParamProp::TsParamProp(ts_param_prop) => {
                Self::from_with_cache(ts_param_prop, cache)
            }
            swc_ecma_ast::ParamOrTsParamProp::Param(param) => Self::from_with_cache(param, cache),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodKind {
    Method,
    Getter,
    Setter,
}

impl From<ast::MethodKind> for MethodKind {
    fn from(value: ast::MethodKind) -> Self {
        match value {
            ast::MethodKind::Method => MethodKind::Method,
            ast::MethodKind::Getter => MethodKind::Getter,
            ast::MethodKind::Setter => MethodKind::Setter,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MethodDescription {
    pub name: CachedString,
    pub params: Vec<ParamDescription>,
    pub is_static: bool,
    pub is_private: bool,
    pub kind: MethodKind,
    pub(crate) body: Option<String>,
}

impl FromWithCache<&ast::ClassMethod> for MethodDescription {
    fn from_with_cache(value: &ast::ClassMethod, cache: &Cache) -> Self {
        let name = str_cached!(cache; value.key.as_ident().unwrap().sym.as_str());
        let mut params = vec![];
        let access = value.accessibility.unwrap_or(ast::Accessibility::Public);
        let body = value.function.body.as_ref().map(|body| get_code(body));

        for param in &value.function.params {
            params.push(ParamDescription::from_with_cache(param, cache))
        }

        MethodDescription {
            name,
            params,
            is_static: value.is_static,
            is_private: access != ast::Accessibility::Public,
            kind: value.kind.into(),
            body: body,
        }
    }
}

fn get_value_type_from_ts_type(type_ann: &ast::TsType, cache: &Cache) -> ValueType {
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
            swc_ecma_ast::TsKeywordTypeKind::TsNullKeyword => ValueType::Null,
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
            let elem_type = get_value_type_from_ts_type(t.elem_type.as_ref(), cache);
            ValueType::array_value_type(&elem_type, cache)
        }
        ast::TsType::TsTupleType(_) => todo!(),
        ast::TsType::TsOptionalType(_) => todo!(),
        ast::TsType::TsRestType(_) => todo!(),
        ast::TsType::TsUnionOrIntersectionType(t) => {
            if let Some(u) = t.as_ts_union_type() {
                if u.types.len() == 2 {
                    let left = get_value_type_from_ts_type(&u.types[0], cache);
                    let right = get_value_type_from_ts_type(&u.types[1], cache);
                    if left == ValueType::Null && !right.is_primitive() {
                        return right;
                    } else if right == ValueType::Null && !left.is_primitive() {
                        return left;
                    }
                }
            }

            todo!()
        }
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

fn get_code(body: &BlockStmt) -> String {
    let c = swc::Compiler::new(Arc::<SourceMap>::default());

    let codegen_config = swc_ecma_codegen::Config::default().with_target(ast::EsVersion::Es2022);

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
    c.print(body, print_args).expect("Failed to get code").code
}

fn get_value_type_from_expr(expr: &ast::Expr, cache: &Cache) -> ValueType {
    match expr {
        ast::Expr::Lit(lit) => match lit {
            ast::Lit::Str(_) => ValueType::String,
            ast::Lit::Bool(_) => ValueType::Bool,
            ast::Lit::Null(_) => todo!(),
            ast::Lit::Num(_) => ValueType::Number,
            _ => todo!(),
        },
        ast::Expr::New(new) => {
            let id = new.callee.as_ident().unwrap();
            let name = str_cached!(cache; id.sym.as_str());
            ValueType::Object(name)
        }
        _ => todo!("expr {:#?} value type is unknown", expr),
    }
}

pub fn get_value_from_expr(
    expr: &ast::Expr,
    graph_id: GraphIndex,
    graphs_map: &mut GraphsMap,
    classes: &TsClasses,
    id_gen: &Arc<GraphIdGenerator>,
    cache: &Arc<Cache>,
) -> Value {
    match expr {
        ast::Expr::Lit(lit) => match lit {
            ast::Lit::Str(s) => vstring!(cache; s.value.to_string()),
            ast::Lit::Bool(bool) => vbool!(bool.value),
            ast::Lit::Null(_) => vnull!(),
            ast::Lit::Num(number) => vnum!(number.value.into()),
            _ => todo!(),
        },
        ast::Expr::New(new) => {
            let id = new.callee.as_ident().unwrap();
            let name = id.sym.as_str();

            let mut params =
                Vec::with_capacity(new.args.as_ref().map(|args| args.len()).unwrap_or(0));
            if let Some(args) = &new.args {
                for arg in args {
                    params.push(get_value_from_expr(
                        &arg.expr, graph_id, graphs_map, classes, id_gen, cache,
                    ));
                }
            }

            let mut boa_ctx = EngineContext::new_boa_ctx();
            let mut engine_ctx = EngineContext::create_engine_ctx(&mut boa_ctx, classes);

            engine_ctx.reset_with_graph(graph_id, graphs_map, classes, id_gen, cache);

            let class = classes.get_class(&str_cached!(cache; name)).unwrap();
            Value::Object(class.call_constructor(&params, classes, &mut engine_ctx))
        }
        _ => todo!("Can't parse static prop value {:#?}", expr),
    }
}

#[derive(Debug, Clone)]
pub struct TsClassDescription {
    pub(crate) id: u64,
    pub class_name: CachedString,
    pub fields: HashMap<String, JsFieldDescription>,
    pub methods: HashMap<String, MethodDescription>,
    pub constructor: MethodDescription,
}

fn static_graph_obj_type(class_name: &str, cache: &Cache) -> ObjectType {
    scached!(cache; format!("{}_{}", class_name, &"Static"))
}

fn static_graph_root_name(class_name: &str, cache: &Cache) -> CachedString {
    scached!(cache; format!("{}_{}", class_name, &"Static"))
}

#[derive(Debug)]
pub struct TsClass {
    pub description: Arc<TsClassDescription>,
    pub member_opcodes: OpcodesList,
    pub method_opcodes: OpcodesList,
    pub static_graph: Option<Arc<ObjectGraph>>,
    pub root_node: NodeIndex,
    pub static_fields: HashMap<CachedString, LocValue>,
    static_graph_obj_type: ObjectType,
    wrapper: JsObjectWrapper,
}

impl TsClass {
    pub fn obj_type(&self) -> ValueType {
        ValueType::Object(self.description.class_name.clone())
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

        let obj_id = graph.add_object_from_map(
            graph_id_gen.get_id_for_node(),
            self.description.class_name.clone(),
            map,
        );

        ObjectValue {
            obj_type: self.description.class_name.clone(),
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

    pub fn wrap_as_js_object(
        &self,
        obj: ObjectValue,
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsObject {
        let wrapper = JsObjectWrapper::new(self.description.clone());
        wrapper.from_data(obj.into(), engine_ctx).unwrap()
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

    pub fn call_constructor<'a, I>(
        &self,
        params: I,
        classes: &TsClasses,
        engine_ctx: &mut EngineContext<'_>,
    ) -> ObjectValue
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
            .construct(&boa_engine::JsValue::new(target), &args, engine_ctx)
            .unwrap();
        let new_instance = JsWrapped::<ObjectValue>::get_from_js_obj(&obj).unwrap();

        new_instance.clone()
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
            .call_method_body(method_name, &this, &args, engine_ctx)?;

        Ok(js_value_to_value(classes, &result, engine_ctx, cache))
    }

    pub fn call_method<'a, I>(
        &self,
        method_name: &str,
        this: &Value,
        param_values: I,
        classes: &TsClasses,
        cache: &Arc<Cache>,
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsResult<Value>
    where
        I: IntoIterator<Item = &'a Value>,
    {
        debug_assert!(!self.description.methods[method_name].is_static);
        if this.is_null() {
            return Err(error_null_this());
        }

        let this = value_to_js_value(classes, &this, engine_ctx);
        let args = param_values
            .into_iter()
            .map(|x| value_to_js_value(classes, x, engine_ctx))
            .collect_vec();

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
}

unsafe impl Send for TsClass {}
unsafe impl Sync for TsClass {}

pub struct TsClassesBuilder {
    classes: Vec<ClassDecl>,
}

impl TsClassesBuilder {
    pub fn new() -> Self {
        Self {
            classes: Default::default(),
        }
    }

    pub fn add_class(&mut self, code: &str, cache: &Cache) -> Result<CachedString, Error> {
        let class_decl = TsClassBuilder::get_class_decl(code)?;
        let class_name = str_cached!(cache; TsClassBuilder::get_class_name(&class_decl));
        self.classes.push(class_decl);
        Ok(class_name)
    }

    pub fn add_ts_file<P: AsRef<Path>>(
        &mut self,
        full_path: P,
        cache: &Cache,
    ) -> Result<Vec<CachedString>, Error> {
        let cm = Arc::<SourceMap>::default();
        let handler = Handler::with_tty_emitter(ColorConfig::Auto, true, false, Some(cm.clone()));
        let c = swc::Compiler::new(cm.clone());

        let mut file = std::fs::File::open(full_path.as_ref())?;
        let mut src = String::new();
        file.read_to_string(&mut src)?;
        let fm = cm.new_source_file(FileName::Real(full_path.as_ref().into()).into(), src);

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
                let class_name = str_cached!(cache; TsClassBuilder::get_class_name(class_decl));
                self.classes.push(class_decl.clone());
                class_names.push(class_name.clone());
            }
        }

        Ok(class_names)
    }

    pub fn finalize(self, cache: &Arc<Cache>) -> Box<TsClasses> {
        let gen_id = Arc::new(GraphIdGenerator::default());
        let mut classes = TsClasses {
            static_classes_gen_id: gen_id,
            classes: Default::default(),
        };

        let mut graphs_map = GraphsMap::default();
        for class_decl in &self.classes {
            let builder = TsClassBuilder::from_class_decl(
                class_decl,
                classes.static_classes_gen_id.clone(),
                cache,
            );
            builder.finalize(&mut classes, &mut graphs_map, cache);
        }

        Box::new(classes)
    }
}

struct TsClassBuilder {
    id: u64,
    class_name: CachedString,
    fields: HashMap<String, JsFieldDescription>,
    methods: HashMap<String, MethodDescription>,
    constructor: MethodDescription,
    gen_id: Arc<GraphIdGenerator>,
}

// Main functions
impl TsClassBuilder {
    fn get_class_decl(code: &str) -> Result<ClassDecl, Error> {
        let script = TsClass::get_ast(code)?.script().unwrap();
        Ok(script.body[0]
            .as_decl()
            .unwrap()
            .as_class()
            .unwrap()
            .clone())
    }

    fn get_class_name(decl: &ClassDecl) -> &str {
        decl.ident.sym.as_str()
    }

    fn from_class_decl(decl: &ClassDecl, gen_id: Arc<GraphIdGenerator>, cache: &Cache) -> Self {
        let mut fields = Default::default();
        let mut methods = Default::default();
        let mut constructor = MethodDescription {
            name: str_cached!(cache; Self::get_class_name(decl)),
            params: Default::default(),
            kind: MethodKind::Method,
            is_static: false,
            is_private: false,
            body: None,
        };

        Self::add_fields(&decl.class, &mut fields, &mut constructor, cache);
        Self::add_methods(&decl.class, &mut methods, cache);

        let class = Self {
            id: gen_id.get_id_for_graph() as u64,
            class_name: str_cached!(cache; Self::get_class_name(decl)),
            fields,
            methods,
            constructor,
            gen_id,
        };

        class
    }

    fn finalize(self, classes: &mut TsClasses, graphs_map: &mut GraphsMap, cache: &Arc<Cache>) {
        let description = Arc::new(TsClassDescription {
            id: self.id,
            class_name: self.class_name.clone(),
            fields: self.fields.clone(),
            constructor: self.constructor.clone(),
            methods: self.methods.clone(),
        });

        let static_graph_obj_type = static_graph_obj_type(&description.class_name, cache);
        let wrapper = JsObjectWrapper::new(description.clone());

        let class = TsClass {
            description,
            member_opcodes: Default::default(),
            method_opcodes: Default::default(),
            static_graph: None,
            root_node: classes.static_classes_gen_id.get_id_for_node(),
            static_fields: Default::default(),
            static_graph_obj_type,
            wrapper,
        };
        let root_node = class.root_node;
        classes
            .classes
            .insert(class.description.class_name.clone(), class);

        if self.fields.values().any(|field| field.is_static) {
            let graph = self.populate_static_graph(root_node, graphs_map, classes, cache);
            classes
                .get_class_mut(&self.class_name)
                .unwrap()
                .static_graph = Some(graph);
        }

        self.populate_opcodes(classes.get_class_mut(&self.class_name).unwrap(), cache);
    }
}

// Field parsing
impl TsClassBuilder {
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
            methods.insert(desc.name.to_string(), desc);
        }
    }
}

// Opcodes
impl TsClassBuilder {
    fn populate_opcodes(&self, class: &mut TsClass, cache: &Cache) {
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
    }

    fn get_opcode_from_field(
        &self,
        class: &TsClass,
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

// static graph
impl TsClassBuilder {
    fn populate_static_graph(
        &self,
        root_node: NodeIndex,
        graphs_map: &mut GraphsMap,
        classes: &mut TsClasses,
        cache: &Arc<Cache>,
    ) -> Arc<ObjectGraph> {
        let graph_id = self.id as GraphIndex;
        let mut graph = ObjectGraph::new_static(graph_id);
        self.add_root_node(&mut graph, root_node, cache);
        graphs_map.insert_graph(graph.into());

        for field in self.fields.values().filter(|field| field.is_static) {
            self.add_static_field(field, graph_id, graphs_map, root_node, classes, cache);
        }

        graphs_map.get(&graph_id).unwrap().clone()
    }

    fn add_root_node(&self, graph: &mut ObjectGraph, root_node: NodeIndex, cache: &Cache) {
        graph.construct_node(
            root_node,
            static_graph_obj_type(&self.class_name, cache),
            fields!(),
        );
        graph.set_as_root(static_graph_root_name(&self.class_name, cache), root_node);
    }

    fn add_static_field(
        &self,
        field: &JsFieldDescription,
        graph_id: GraphIndex,
        graphs_map: &mut GraphsMap,
        root_node: NodeIndex,
        classes: &mut TsClasses,
        cache: &Arc<Cache>,
    ) {
        let init_expr = field.init_expr.as_ref().unwrap();
        let init_val = get_value_from_expr(
            init_expr,
            graph_id,
            graphs_map,
            classes,
            &self.gen_id,
            cache,
        );

        let graph = Arc::make_mut(graphs_map.get_mut(&graph_id).unwrap());
        graph.set_field(&root_node, str_cached!(cache; &field.name), &init_val);

        let class = classes.get_class_mut(&self.class_name).unwrap();
        class.static_fields.insert(
            field.name.clone(),
            LocValue {
                loc: Location::ObjectField(ObjectFieldLoc {
                    graph: graph_id,
                    node: root_node,
                    field: str_cached!(cache; &field.name),
                    attrs: Attributes::default(),
                }),
                val: init_val,
            },
        );
    }
}
