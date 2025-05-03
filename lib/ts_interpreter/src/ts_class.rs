use std::{collections::HashMap, fmt::Debug, io::Read, path::Path, sync::Arc};

use boa_engine::{class::DynamicClassBuilder, object::builtins::JsTypedArray, JsValue};
use itertools::Itertools;
use ruse_object_graph::{
    fields, scached, str_cached, vbool, vnull, vnum, vstring, Attributes, Cache, CachedString,
    FieldName, GraphIndex, GraphsMap, NodeIndex, ObjectGraph, ObjectType, PrimitiveValue,
    ValueType,
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
use swc_ecma_ast::{self as ast, BlockStmt, ClassDecl, FnDecl, VarDecl};

use anyhow::Error;
use ruse_object_graph::value::*;
use swc_ecma_parser::{Syntax, TsSyntax};

use crate::{
    engine_context::EngineContext,
    js_errors::{js_error_not_builtin_array, js_error_null_this},
    js_object_wrapper::{JsArrayWrapper, JsObjectWrapper},
    js_value::{js_value_to_value, value_to_js_value},
    js_wrapped::JsWrapped,
    opcode::{ClassConstructorOp, ClassMethodOp, MemberOp, StaticMemberOp},
};

#[derive(Debug)]
pub struct TsClasses {
    pub static_classes_gen_id: Arc<GraphIdGenerator>,
    builtin_classes: HashMap<String, Box<dyn TsBuiltinClass>>,
    user_classes: HashMap<String, TsUserClass>,
}

impl SynthesizerContextData for TsClasses {}

impl TsClasses {
    const GLOBAL_CLASS_NAME: &'static str = "__GLOBAL_CLASS__";

    pub fn get_class(&self, obj_type: &ObjectType) -> Option<&dyn TsClass> {
        match obj_type {
            ObjectType::Class(class_name) => self
                .user_classes
                .get(class_name.as_str())
                .map(|user_class| user_class as &dyn TsClass),
            _ => self
                .get_builtin_class(obj_type)
                .map(|class| class as &dyn TsClass),
        }
    }

    pub fn get_builtin_class(&self, obj_type: &ObjectType) -> Option<&dyn TsBuiltinClass> {
        let base_class_name = obj_type.obj_type_base_name();
        self.builtin_classes
            .get(base_class_name)
            .map(|class| class.as_ref() as &dyn TsBuiltinClass)
    }

    pub fn get_user_class(&self, class: &str) -> Option<&TsUserClass> {
        let base_class_name = class.split("<").next().unwrap();
        self.user_classes.get(base_class_name)
    }

    pub fn get_user_class_mut(&mut self, class: &str) -> Option<&mut TsUserClass> {
        let base_class_name = class.split("<").next().unwrap();
        self.user_classes.get_mut(base_class_name)
    }

    pub(crate) fn init_engine_ctx(&self, engine_ctx: &mut EngineContext<'_>) {
        for (name, class) in &self.user_classes {
            if name == &Self::GLOBAL_CLASS_NAME {
                engine_ctx
                    .register_global_class(class)
                    .expect("Failed to register global class");
                continue;
            }
            engine_ctx
                .register_global_dynamic_class(&class.wrapper)
                .expect("Failed to register dynamic class");
        }
    }

    pub(crate) fn builtin_classes(&self) -> impl Iterator<Item = &dyn TsBuiltinClass> {
        self.builtin_classes.values().map(|class| class.as_ref())
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
impl FromWithCache<&ast::VarDeclarator> for JsFieldDescription {
    fn from_with_cache(value: &ast::VarDeclarator, cache: &Cache) -> Self {
        let ident = value.name.as_ident().unwrap();
        let field_name = ident.sym.as_str();
        let access = ast::Accessibility::Public;
        let value_type = if let Some(type_ann) = &ident.type_ann {
            get_value_type_from_ts_type(&type_ann.type_ann, cache)
        } else {
            get_value_type_from_expr(&value.init.as_ref().unwrap(), cache)
        };

        JsFieldDescription {
            name: str_cached!(cache; field_name),
            value_type,
            is_private: access != ast::Accessibility::Public,
            is_readonly: false,
            is_static: true,
            is_constructor_prop: false,
            init_expr: value.init.clone(),
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
    GlobalFunction,
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

impl FromWithCache<&ast::FnDecl> for MethodDescription {
    fn from_with_cache(value: &ast::FnDecl, cache: &Cache) -> Self {
        let name = str_cached!(cache; value.ident.sym.as_str());
        let mut params = vec![];
        let access = ast::Accessibility::Public;
        let body = value.function.body.as_ref().map(|body| get_code(body));

        for param in &value.function.params {
            params.push(ParamDescription::from_with_cache(param, cache))
        }

        MethodDescription {
            name,
            params,
            is_static: true,
            is_private: access != ast::Accessibility::Public,
            kind: MethodKind::GlobalFunction,
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
            ValueType::class_value_type(scached!(cache; id))
        }
        ast::TsType::TsTypeQuery(_) => todo!(),
        ast::TsType::TsTypeLit(_) => todo!(),
        ast::TsType::TsArrayType(t) => {
            let elem_type = get_value_type_from_ts_type(t.elem_type.as_ref(), cache);
            ValueType::array_value_type(&elem_type)
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
            ValueType::class_value_type(name)
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

            let class = classes.get_user_class(&str_cached!(cache; name)).unwrap();
            Value::Object(
                class
                    .call_constructor(&params, classes, &mut engine_ctx)
                    .unwrap(),
            )
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
    pub extends: Option<CachedString>,
    pub is_abstract: bool,
}

fn static_graph_obj_type(class_name: &str, cache: &Cache) -> ObjectType {
    ObjectType::class_obj_type(&format!("{}_{}", class_name, &"Static"), cache)
}

fn static_graph_root_name(class_name: &str, cache: &Cache) -> CachedString {
    scached!(cache; format!("{}_{}", class_name, &"Static"))
}

pub trait TsClass: Send + Sync + Debug {
    fn obj_type(&self, template_types: Option<Vec<ValueType>>) -> ValueType;
    fn is_parametrized(&self) -> bool;
    fn get_class_name(&self) -> &CachedString;
    fn get_class_id(&self) -> u64;
    fn wrap_as_js_object(
        &self,
        obj: ObjectValue,
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsObject;
}

pub trait TsBuiltinClass: TsClass {
    fn is_builtin_object(&self, value: &boa_engine::JsObject) -> bool;

    fn get_from_js_obj(
        &self,
        classes: &TsClasses,
        value: &boa_engine::JsObject,
        engine_ctx: &mut EngineContext<'_>,
        cache: &Cache,
    ) -> Result<ObjectValue, boa_engine::JsError>;
}

#[derive(Debug)]
pub struct BuiltinArrayClass {
    class_name: CachedString,
    id: u64,
}

impl BuiltinArrayClass {
    fn new(id: u64, cache: &Cache) -> Self {
        Self {
            class_name: str_cached!(cache; "Array"),
            id,
        }
    }

    fn get_from_js_array(
        &self,
        classes: &TsClasses,
        js_array: &boa_engine::object::builtins::JsArray,
        engine_ctx: &mut EngineContext<'_>,
        cache: &Cache,
    ) -> Result<ObjectValue, boa_engine::JsError> {
        let arr_len = js_array.length(engine_ctx).unwrap() as i64;
        if arr_len == 0 {
            return engine_ctx.create_array_object(vec![], &ValueType::Null);
        }

        let elements: Vec<Value> = (0..arr_len)
            .map(|i| {
                let js_elem = js_array.at(i, engine_ctx).unwrap();
                let elem = js_value_to_value(classes, &js_elem, engine_ctx, cache);
                elem
            })
            .collect();

        let elem_type = elements[0].val_type();
        engine_ctx.create_array_object(elements, &elem_type)
    }

    fn get_from_js_typed_array(
        &self,
        classes: &TsClasses,
        js_typed_array: &JsTypedArray,
        engine_ctx: &mut EngineContext<'_>,
        cache: &Cache,
    ) -> Result<ObjectValue, boa_engine::JsError> {
        let arr_len = js_typed_array.length(engine_ctx).unwrap() as i64;
        if arr_len == 0 {
            return engine_ctx.create_array_object(vec![], &ValueType::Number);
        }

        let elements: Vec<Value> = (0..arr_len)
            .map(|i| {
                let js_elem = js_typed_array.at(i, engine_ctx).unwrap();
                let elem = js_value_to_value(classes, &js_elem, engine_ctx, cache);
                assert!(elem.number_value().is_some());
                elem
            })
            .collect();

        engine_ctx.create_array_object(elements, &ValueType::Number)
    }
}

impl TsClass for BuiltinArrayClass {
    fn obj_type(&self, template_types: Option<Vec<ValueType>>) -> ValueType {
        let template_types = template_types.unwrap();
        assert!(template_types.len() == 1);
        ValueType::array_value_type(&template_types[0])
    }

    fn wrap_as_js_object(
        &self,
        obj: ObjectValue,
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsObject {
        JsArrayWrapper::wrap_object(&obj, engine_ctx).unwrap()
    }

    fn is_parametrized(&self) -> bool {
        false
    }

    fn get_class_name(&self) -> &CachedString {
        &self.class_name
    }

    fn get_class_id(&self) -> u64 {
        self.id
    }
}

impl TsBuiltinClass for BuiltinArrayClass {
    fn is_builtin_object(&self, value: &boa_engine::JsObject) -> bool {
        value.is_array() || value.is::<boa_engine::builtins::typed_array::TypedArray>()
    }

    fn get_from_js_obj(
        &self,
        classes: &TsClasses,
        value: &boa_engine::JsObject,
        engine_ctx: &mut EngineContext<'_>,
        cache: &Cache,
    ) -> Result<ObjectValue, boa_engine::JsError> {
        if let Ok(js_array) = boa_engine::object::builtins::JsArray::from_object(value.clone()) {
            self.get_from_js_array(classes, &js_array, engine_ctx, cache)
        } else if let Ok(typed_array) =
            boa_engine::object::builtins::JsTypedArray::from_object(value.clone())
        {
            self.get_from_js_typed_array(classes, &typed_array, engine_ctx, cache)
        } else {
            Err(js_error_not_builtin_array())
        }
    }
}

#[derive(Debug)]
pub struct TsUserClass {
    pub description: Arc<TsClassDescription>,
    pub member_opcodes: OpcodesList,
    pub method_opcodes: OpcodesList,
    pub constructor_opcodes: OpcodesList,
    pub static_graph: Option<Arc<ObjectGraph>>,
    pub root_node: NodeIndex,
    pub static_fields: HashMap<CachedString, LocValue>,
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

    fn get_class_name(&self) -> &CachedString {
        &self.description.class_name
    }

    fn get_class_id(&self) -> u64 {
        self.description.id
    }
}

unsafe impl Send for TsUserClass {}
unsafe impl Sync for TsUserClass {}

pub struct TsClassesBuilder {
    classes: Vec<(Vec<String>, ClassDecl)>,
    functions: Vec<(Vec<String>, FnDecl)>,
    variables: Vec<(Vec<String>, Box<VarDecl>)>,
}

impl TsClassesBuilder {
    pub fn new() -> Self {
        Self {
            classes: Default::default(),
            functions: Default::default(),
            variables: Default::default(),
        }
    }

    pub fn add_classes(&mut self, code: &str, cache: &Cache) -> Result<Vec<CachedString>, Error> {
        let script = TsUserClass::get_ast(code)?.script().unwrap();
        self.parse_body(&vec![], &script.body, cache)
    }

    pub fn add_ts_files<P: AsRef<Path>>(
        &mut self,
        full_path: P,
        cache: &Cache,
    ) -> Result<Vec<CachedString>, Error> {
        if !full_path.as_ref().exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("{} doesn't exist", full_path.as_ref().display()),
            )
            .into());
        } else if full_path.as_ref().is_dir() {
            let mut classes_names = vec![];

            for entry in walkdir::WalkDir::new(full_path)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|e| {
                    e.file_type().is_file()
                        && e.path().extension().map(|s| s == "ts").unwrap_or(false)
                })
            {
                classes_names.extend(self.add_ts_file(entry.path(), cache)?);
            }

            Ok(classes_names)
        } else {
            self.add_ts_file(full_path, cache)
        }
    }

    fn add_ts_file<P: AsRef<Path>>(
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

        self.parse_body(&vec![], &script.body, cache)
    }

    pub fn parse_body(
        &mut self,
        namespace: &Vec<String>,
        body: &Vec<ast::Stmt>,
        cache: &Cache,
    ) -> Result<Vec<CachedString>, Error> {
        let mut class_names = vec![];

        for stmt in body {
            match stmt {
                ast::Stmt::Decl(ast::Decl::Class(class_decl)) => {
                    let class_name =
                        scached!(cache; TsClassBuilder::get_class_name(namespace, class_decl));
                    self.classes.push((namespace.clone(), class_decl.clone()));
                    class_names.push(class_name.clone());
                }
                ast::Stmt::Decl(ast::Decl::Fn(fn_decl)) => {
                    self.functions.push((namespace.clone(), fn_decl.clone()));
                }
                ast::Stmt::Decl(ast::Decl::Var(var_decl)) => {
                    self.variables.push((namespace.clone(), var_decl.clone()));
                }
                ast::Stmt::Decl(ast::Decl::TsModule(ts_module)) => {
                    if let Some(ast::TsNamespaceBody::TsModuleBlock(ns_body)) =
                        ts_module.body.clone()
                    {
                        let ns_name = ts_module.id.as_ident().unwrap().sym.as_str().to_string();
                        let mut new_ns = namespace.clone();
                        new_ns.insert(0, ns_name);
                        let stmts = ns_body
                            .body
                            .into_iter()
                            .filter_map(|x| x.stmt())
                            .collect_vec();
                        let ns_classes = self.parse_body(&new_ns, &stmts, cache)?;
                        class_names.extend(ns_classes);
                    }
                }
                _ => {
                    // Ignore other statements
                }
            }
        }

        Ok(class_names)
    }

    fn get_builtin_classes(&self, cache: &Cache) -> HashMap<String, Box<dyn TsBuiltinClass>> {
        let mut builtin_classes: HashMap<String, Box<dyn TsBuiltinClass>> = HashMap::new();
        builtin_classes.insert(
            "Array".to_string(),
            Box::new(BuiltinArrayClass::new(builtin_classes.len() as u64, cache)),
        );
        builtin_classes
    }

    pub fn finalize(self, cache: &Arc<Cache>) -> Box<TsClasses> {
        let builtin_classes = self.get_builtin_classes(cache);

        let gen_id = Arc::new(GraphIdGenerator::with_initial_values(
            NodeIndex(0),
            builtin_classes.len(),
        ));

        let mut classes = TsClasses {
            static_classes_gen_id: gen_id,
            user_classes: Default::default(),
            builtin_classes,
        };

        let mut graphs_map = GraphsMap::default();
        let global_class_builder = TsClassBuilder::global_class(
            self.functions,
            self.variables,
            classes.static_classes_gen_id.clone(),
            cache,
        );
        global_class_builder.finalize(&mut classes, &mut graphs_map, cache);

        for (namespace, class_decl) in &self.classes {
            let builder = TsClassBuilder::from_class_decl(
                namespace,
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
    super_class: Option<CachedString>,
    is_abstract: bool,
}

// Main functions
impl TsClassBuilder {
    fn get_name_with_namespace(namespace: &Vec<String>, name: &str) -> String {
        if namespace.is_empty() {
            name.to_string()
        } else {
            namespace.join(".") + "." + name
        }
    }

    fn get_class_name(namespace: &Vec<String>, decl: &ClassDecl) -> String {
        Self::get_name_with_namespace(namespace, decl.ident.sym.as_str())
    }

    fn from_class_decl(
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

    fn global_class(
        functions: Vec<(Vec<String>, FnDecl)>,
        variables: Vec<(Vec<String>, Box<VarDecl>)>,
        gen_id: Arc<GraphIdGenerator>,
        cache: &Cache,
    ) -> Self {
        let mut fields = HashMap::default();
        let mut methods = HashMap::default();

        for func in functions {
            let func_name = Self::get_name_with_namespace(&func.0, func.1.ident.sym.as_str());
            let desc = MethodDescription::from_with_cache(&func.1, cache);
            methods.insert(func_name, desc);
        }
        for var in variables {
            for decl in var.1.decls.iter() {
                let var_name = Self::get_name_with_namespace(
                    &var.0,
                    decl.name.as_ident().unwrap().sym.as_str(),
                );
                let desc = JsFieldDescription::from_with_cache(decl, cache);
                fields.insert(var_name, desc);
            }
        }

        let constructor = MethodDescription {
            name: str_cached!(cache; TsClasses::GLOBAL_CLASS_NAME),
            params: Default::default(),
            kind: MethodKind::Method,
            is_static: false,
            is_private: true,
            body: None,
        };

        let class = Self {
            id: gen_id.get_id_for_graph() as u64,
            class_name: str_cached!(cache; TsClasses::GLOBAL_CLASS_NAME),
            fields,
            methods,
            constructor,
            gen_id,
            super_class: None,
            is_abstract: true,
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
            extends: self.super_class.clone(),
            is_abstract: self.is_abstract,
        });

        let static_graph_obj_type = static_graph_obj_type(&description.class_name, cache);
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
        classes
            .user_classes
            .insert(class.description.class_name.to_string(), class);

        if self.fields.values().any(|field| field.is_static) {
            let graph = self.populate_static_graph(root_node, graphs_map, classes, cache);
            classes
                .get_user_class_mut(&self.class_name)
                .unwrap()
                .static_graph = Some(graph);
        }

        self.populate_opcodes(classes.get_user_class_mut(&self.class_name).unwrap(), cache);
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
            methods.insert(desc.name.to_string(), desc);
        }
    }
}

// Opcodes
impl TsClassBuilder {
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
        graphs_map.new_static_graph(graph_id);
        self.add_root_node(graphs_map, graph_id, root_node, cache);

        for field in self.fields.values().filter(|field| field.is_static) {
            self.add_static_field(field, graph_id, graphs_map, root_node, classes, cache);
        }

        graphs_map.get(&graph_id).unwrap().clone()
    }

    fn add_root_node(
        &self,
        graphs_map: &mut GraphsMap,
        graph_id: GraphIndex,
        root_node: NodeIndex,
        cache: &Cache,
    ) {
        graphs_map.construct_node(
            graph_id,
            root_node,
            static_graph_obj_type(&self.class_name, cache),
            fields!(),
        );
        graphs_map.set_as_root(
            static_graph_root_name(&self.class_name, cache),
            graph_id,
            root_node,
        );
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

        graphs_map.set_field(
            str_cached!(cache; &field.name),
            graph_id,
            root_node,
            &init_val,
        );

        let class = classes.get_user_class_mut(&self.class_name).unwrap();
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
