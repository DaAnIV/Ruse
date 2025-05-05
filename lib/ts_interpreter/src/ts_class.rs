use std::{collections::HashMap, fmt::Debug, sync::Arc};

use boa_engine::{context::intrinsics::StandardConstructor, JsResult, JsString};
use ruse_object_graph::{
    fields, scached, str_cached, vbool, vnull, vnum, vstring, Attributes, Cache, CachedString,
    ClassName, GraphIndex, GraphsMap, NodeIndex, ObjectGraph, ObjectType, PrimitiveValue, RootName,
    ValueType,
};
use ruse_synthesizer::{
    context::GraphIdGenerator,
    location::{LocValue, Location, ObjectFieldLoc},
};
use swc_common::SourceMap;
use swc_ecma_ast::{self as ast, BlockStmt};

use ruse_object_graph::value::*;

use crate::{engine_context::EngineContext, ts_classes::TsClasses};

pub(crate) fn get_name_with_namespace(namespace: &Vec<String>, name: &str) -> String {
    if namespace.is_empty() {
        name.to_string()
    } else {
        namespace.join(".") + "." + name
    }
}

pub trait FromWithCache<T> {
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
    pub init_expr: Option<Box<ast::Expr>>,
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
        let value_type = if let Some(type_ann) = &ident.type_ann {
            get_value_type_from_ts_type(&type_ann.type_ann, cache)
        } else {
            get_value_type_from_expr(&value.init.as_ref().unwrap(), cache)
        };

        JsFieldDescription {
            name: str_cached!(cache; field_name),
            value_type,
            is_private: true,
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
            swc_ecma_ast::Pat::Ident(binding_ident) => {
                let value_type = if let Some(type_ann) = &binding_ident.type_ann.as_ref() {
                    get_value_type_from_ts_type(&type_ann.type_ann, cache)
                } else {
                    ValueType::Null
                };
                ParamDescription {
                    name: str_cached!(cache; binding_ident.id.sym.as_str()),
                    value_type,
                    is_prop: false,
                }
            }
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
        let body = value.function.body.as_ref().map(|body| get_code(body));

        for param in &value.function.params {
            params.push(ParamDescription::from_with_cache(param, cache))
        }

        MethodDescription {
            name,
            params,
            is_static: true,
            is_private: true,
            kind: MethodKind::GlobalFunction,
            body: body,
        }
    }
}

pub(crate) fn get_value_type_from_ts_type(type_ann: &ast::TsType, cache: &Cache) -> ValueType {
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
            match id.as_str() {
                "Array" => {
                    let type_params = t.type_params.as_ref().unwrap();
                    assert!(type_params.params.len() == 1);
                    let elem_type = get_value_type_from_ts_type(&type_params.params[0], cache);
                    return ValueType::array_value_type(&elem_type);
                }
                "Set" => {
                    let type_params = t.type_params.as_ref().unwrap();
                    assert!(type_params.params.len() == 1);
                    let elem_type = get_value_type_from_ts_type(&type_params.params[0], cache);
                    return ValueType::set_value_type(&elem_type);
                }
                "Map" => {
                    let type_params = t.type_params.as_ref().unwrap();
                    assert!(type_params.params.len() == 2);
                    let key_type = get_value_type_from_ts_type(&type_params.params[0], cache);
                    let value_type = get_value_type_from_ts_type(&type_params.params[1], cache);
                    return ValueType::map_value_type(&key_type, &value_type);
                }
                _ => {}
            }
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

pub(crate) fn get_code(body: &BlockStmt) -> String {
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
        source_map_url: None,
    };
    c.print(body, print_args).expect("Failed to get code").code
}

pub(crate) fn get_value_type_from_expr(expr: &ast::Expr, cache: &Cache) -> ValueType {
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

pub trait TsClass: Send + Sync + Debug {
    fn obj_type(&self, template_types: Option<Vec<ValueType>>) -> ValueType;
    fn is_parametrized(&self) -> bool;
    fn get_class_name(&self) -> &ClassName;
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

pub trait BuiltinClassWrapper {
    const NAME: JsString;

    fn build_standard_constructor(
        engine_ctx: &mut EngineContext<'_>,
    ) -> JsResult<StandardConstructor>;

    fn wrap_object(
        map_obj: &ObjectValue,
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsResult<boa_engine::JsObject>;
}

pub(crate) struct StaticGraphBuilder<'a> {
    pub id: u64,
    pub class_name: &'a str,
    pub gen_id: &'a Arc<GraphIdGenerator>,
}

impl<'a> StaticGraphBuilder<'a> {
    pub fn static_graph_obj_type(class_name: &str, cache: &Cache) -> ObjectType {
        ObjectType::class_obj_type(&format!("{}_{}", class_name, &"Static"), cache)
    }

    pub fn static_graph_root_name(class_name: &str, cache: &Cache) -> RootName {
        scached!(cache; format!("{}_{}", class_name, &"Static"))
    }

    pub fn populate_static_graph<'b, I>(
        &self,
        fields: I,
        root_node: NodeIndex,
        graphs_map: &mut GraphsMap,
        classes: &mut TsClasses,
        cache: &Arc<Cache>,
    ) -> Arc<ObjectGraph>
    where
        I: IntoIterator<Item = &'b JsFieldDescription>,
    {
        let graph_id = self.id as GraphIndex;
        graphs_map.new_static_graph(graph_id);
        self.add_root_node(graphs_map, graph_id, root_node, cache);

        for field in fields.into_iter().filter(|field| field.is_static) {
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
            Self::static_graph_obj_type(&self.class_name, cache),
            fields!(),
        );
        graphs_map.set_as_root(
            Self::static_graph_root_name(&self.class_name, cache),
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
        let init_val =
            get_value_from_expr(init_expr, graph_id, graphs_map, classes, self.gen_id, cache);

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
