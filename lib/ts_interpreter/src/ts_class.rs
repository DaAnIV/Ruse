use std::{collections::HashMap, fmt::Debug, sync::Arc};

use boa_engine::{context::intrinsics::StandardConstructor, JsResult};
use ruse_object_graph::{
    class_name, field_name, fields,
    location::{LocValue, Location, ObjectFieldLoc},
    root_name, str_cached, vbool, vnull, vnum, vstr, Attributes, ClassName, FieldName, GraphIndex,
    GraphsMap, NodeIndex, ObjectGraph, ObjectType, PrimitiveValue, RootName, ValueType,
};
use ruse_synthesizer::context::GraphIdGenerator;
use swc_common::SourceMap;
use swc_compiler_base::IdentCollector;
use swc_ecma_ast::{self as ast, Ident};

use ruse_object_graph::value::*;
use swc_ecma_codegen::Node;
use swc_ecma_utils::find_pat_ids;
use swc_ecma_visit::VisitWith;

use crate::{
    dts_visitor::{
        get_value_type_from_ts_type, DtsFieldDecl, DtsFnDecl, DtsMethodDecl, DtsVarDecl,
    },
    engine_context::EngineContext,
    ts_classes::TsClasses,
};

#[derive(Debug, Clone)]
pub struct JsFieldDescription {
    pub name: FieldName,
    pub value_type: Option<ValueType>,
    pub is_private: bool,
    pub is_static: bool,
    pub is_readonly: bool,
    pub init_expr: Option<Box<ast::Expr>>,
}

impl JsFieldDescription {
    pub fn get_primitive_value(&self) -> Option<PrimitiveValue> {
        match self.init_expr.as_ref()?.as_ref() {
            ast::Expr::Lit(lit) => match lit {
                ast::Lit::Str(s) => Some(PrimitiveValue::String(str_cached!(s.value.as_str()))),
                ast::Lit::Bool(bool) => Some(PrimitiveValue::Bool(bool.value)),
                ast::Lit::Null(_) => None,
                ast::Lit::Num(number) => Some(PrimitiveValue::Number(number.value.into())),
                _ => todo!(),
            },
            _ => None,
        }
    }
}

impl From<&ast::ClassProp> for JsFieldDescription {
    fn from(value: &ast::ClassProp) -> Self {
        let ident = value.key.as_ident().unwrap();
        let field_name = ident.sym.as_str();
        let access = value.accessibility.unwrap_or(ast::Accessibility::Public);
        let value_type = if let Some(type_ann) = &value.type_ann {
            Some(get_value_type_from_ts_type(&type_ann.type_ann))
        } else if let Some(init_expr) = &value.value {
            Some(get_value_type_from_expr(init_expr))
        } else {
            None
        };

        JsFieldDescription {
            name: field_name!(field_name),
            value_type,
            is_private: access != ast::Accessibility::Public,
            is_readonly: value.readonly,
            is_static: value.is_static,
            init_expr: value.value.clone(),
        }
    }
}

impl From<(&ast::ClassProp, &DtsFieldDecl)> for JsFieldDescription {
    fn from(value: (&ast::ClassProp, &DtsFieldDecl)) -> Self {
        let (decl, dts) = value;
        let ident = decl.key.as_ident().unwrap();
        let field_name = ident.sym.as_str();

        JsFieldDescription {
            name: field_name!(field_name),
            value_type: dts.var_type.clone(),
            is_private: !dts.is_public,
            is_readonly: dts.is_readonly,
            is_static: dts.is_static,
            init_expr: decl.value.clone(),
        }
    }
}

impl From<(&ast::VarDeclarator, &DtsVarDecl, bool)> for JsFieldDescription {
    fn from(value: (&ast::VarDeclarator, &DtsVarDecl, bool)) -> Self {
        let (decl, dts, exported) = value;
        let ident = decl.name.as_ident().unwrap();
        let field_name = ident.sym.as_str();
        let value_type = if exported { dts.var_type.clone() } else { None };

        JsFieldDescription {
            name: field_name!(field_name),
            value_type,
            is_private: !exported,
            is_readonly: false,
            is_static: true,
            init_expr: decl.init.clone(),
        }
    }
}

impl From<&ast::VarDeclarator> for JsFieldDescription {
    fn from(decl: &ast::VarDeclarator) -> Self {
        let ident = decl.name.as_ident().unwrap();
        let field_name = ident.sym.as_str();

        JsFieldDescription {
            name: field_name!(field_name),
            value_type: None,
            is_private: true,
            is_readonly: false,
            is_static: true,
            init_expr: decl.init.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    pub name: String,
    pub param_names: Vec<String>,
    pub param_types: Vec<Vec<ValueType>>,
    pub has_rest_param: bool,
    pub is_static: bool,
    pub is_private: bool,
    pub kind: MethodKind,
    pub(crate) body_code: String,
}

impl From<(&ast::ClassMethod, &DtsMethodDecl)> for MethodDescription {
    fn from(value: (&ast::ClassMethod, &DtsMethodDecl)) -> Self {
        let (decl, dts) = value;
        let name = dts.name.to_string();
        let body_code = get_code(decl.function.body.as_ref().unwrap());
        let param_names: Vec<String> = find_pat_ids::<_, Ident>(&decl.function.params)
            .into_iter()
            .map(|id| id.sym.to_string())
            .collect();

        MethodDescription {
            name,
            param_names,
            param_types: dts.params.clone(),
            has_rest_param: dts.has_rest_param,
            is_static: dts.is_static,
            is_private: !dts.is_public,
            kind: decl.kind.into(),
            body_code,
        }
    }
}

impl From<(&ast::Constructor, &DtsMethodDecl)> for MethodDescription {
    fn from(value: (&ast::Constructor, &DtsMethodDecl)) -> Self {
        let (decl, dts) = value;
        let name = format!("__{}", dts.name);
        let body_code = get_code(decl.body.as_ref().unwrap());
        let param_names: Vec<String> = find_pat_ids::<_, Ident>(&decl.params)
            .into_iter()
            .map(|id| id.sym.to_string())
            .collect();

        MethodDescription {
            name,
            param_names,
            param_types: dts.params.clone(),
            has_rest_param: dts.has_rest_param,
            is_static: true,
            is_private: !dts.is_public,
            kind: MethodKind::Method,
            body_code,
        }
    }
}

impl From<&ast::Constructor> for MethodDescription {
    fn from(decl: &ast::Constructor) -> Self {
        let name = format!("__{}", decl.key.as_ident().unwrap().sym.to_string());
        let body_code = get_code(decl.body.as_ref().unwrap());
        let param_names: Vec<String> = find_pat_ids::<_, Ident>(&decl.params)
            .into_iter()
            .map(|id| id.sym.to_string())
            .collect();
        let access = decl.accessibility.unwrap_or(ast::Accessibility::Public);
        let has_rest_param = decl.params.iter().any(|param| match param {
            swc_ecma_ast::ParamOrTsParamProp::Param(param) => param.pat.is_rest(),
            swc_ecma_ast::ParamOrTsParamProp::TsParamProp(param) => match &param.param {
                swc_ecma_ast::TsParamPropParam::Ident(ident) => ident
                    .type_ann
                    .as_ref()
                    .map_or(false, |x| x.type_ann.is_ts_rest_type()),
                swc_ecma_ast::TsParamPropParam::Assign(assign) => assign.left.is_rest(),
            },
        });

        MethodDescription {
            name,
            param_names,
            param_types: vec![],
            has_rest_param,
            is_static: true,
            is_private: access != ast::Accessibility::Public,
            kind: MethodKind::Method,
            body_code,
        }
    }
}

impl From<&ast::ClassMethod> for MethodDescription {
    fn from(value: &ast::ClassMethod) -> Self {
        let name = value.key.as_ident().unwrap().sym.to_string();
        let access = value.accessibility.unwrap_or(ast::Accessibility::Public);
        let body_code = get_code(value.function.body.as_ref().unwrap());
        let param_names: Vec<String> = find_pat_ids::<_, Ident>(&value.function.params)
            .into_iter()
            .map(|id| id.sym.to_string())
            .collect();

        let has_rest_param = value
            .function
            .params
            .iter()
            .any(|param| param.pat.is_rest());

        MethodDescription {
            name,
            param_names,
            param_types: vec![],
            has_rest_param,
            is_static: value.is_static,
            is_private: access != ast::Accessibility::Public,
            kind: value.kind.into(),
            body_code,
        }
    }
}

impl From<(&ast::FnDecl, &DtsFnDecl, bool)> for MethodDescription {
    fn from(value: (&ast::FnDecl, &DtsFnDecl, bool)) -> Self {
        let (decl, dts, exported) = value;
        let name = dts.name.0.to_string();
        let body_code = get_code(decl.function.body.as_ref().unwrap());
        let param_names: Vec<String> = find_pat_ids::<_, Ident>(&decl.function.params)
            .into_iter()
            .map(|id| id.sym.to_string())
            .collect();

        let is_private = !exported;

        MethodDescription {
            name,
            param_names,
            param_types: dts.params.clone(),
            has_rest_param: dts.has_rest_param,
            is_static: true,
            is_private,
            kind: MethodKind::GlobalFunction,
            body_code,
        }
    }
}

impl From<&ast::FnDecl> for MethodDescription {
    fn from(decl: &ast::FnDecl) -> Self {
        let name = decl.ident.sym.as_str().to_string();
        let body_code = get_code(decl.function.body.as_ref().unwrap());
        let param_names: Vec<String> = find_pat_ids::<_, Ident>(&decl.function.params)
            .into_iter()
            .map(|id| id.sym.to_string())
            .collect();
        let has_rest_param = decl.function.params.iter().any(|param| param.pat.is_rest());

        MethodDescription {
            name,
            param_names,
            param_types: vec![],
            has_rest_param,
            is_static: true,
            is_private: true,
            kind: MethodKind::GlobalFunction,
            body_code,
        }
    }
}

pub(crate) fn get_code<T>(body: &T) -> String
where
    T: Node + VisitWith<IdentCollector>,
{
    let c = swc::Compiler::new(Arc::<SourceMap>::default());

    let codegen_config = swc_ecma_codegen::Config::default().with_target(ast::EsVersion::Es2022);

    let mut print_args = swc::PrintArgs::default();
    print_args.source_map = swc::config::SourceMapsConfig::Bool(false);
    print_args.codegen_config = codegen_config;

    c.print(body, print_args).expect("Failed to get code").code
}

pub(crate) fn get_value_type_from_expr(expr: &ast::Expr) -> ValueType {
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
            let name = class_name!(id.sym.as_str());
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
) -> Value {
    match expr {
        ast::Expr::Lit(lit) => match lit {
            ast::Lit::Str(s) => vstr!(s.value.to_string()),
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
                        &arg.expr, graph_id, graphs_map, classes, id_gen,
                    ));
                }
            }

            let mut engine_ctx = EngineContext::create_engine_ctx(classes);

            engine_ctx.reset_with_graph(graph_id, graphs_map, classes, id_gen);

            let class = classes.get_user_class(&class_name!(name)).unwrap();
            Value::Object(class.call_constructor(&params, &mut engine_ctx).unwrap())
        }
        _ => todo!("Can't parse static prop value {:#?}", expr),
    }
}

#[derive(Debug, Clone)]
pub struct TsClassDescription {
    pub(crate) id: u64,
    pub class_name: ClassName,
    pub fields: HashMap<String, JsFieldDescription>,
    pub methods: HashMap<(String, MethodKind), MethodDescription>,
    pub constructor: MethodDescription,
    pub extends: Option<ClassName>,
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
        engine_ctx: &mut EngineContext,
    ) -> JsResult<boa_engine::JsObject>;
}

pub trait TsBuiltinClass: TsClass {
    fn is_builtin_object(&self, value: &boa_engine::JsObject) -> bool;

    fn get_from_js_obj(
        &self,
        value: &boa_engine::JsObject,
        engine_ctx: &mut EngineContext,
    ) -> Result<ObjectValue, boa_engine::JsError>;
}

pub trait BuiltinClassWrapper {
    fn build_standard_constructor(engine_ctx: &mut EngineContext) -> JsResult<StandardConstructor>;

    fn wrap_object(
        map_obj: &ObjectValue,
        engine_ctx: &mut EngineContext,
    ) -> boa_engine::JsResult<boa_engine::JsObject>;
}

pub(crate) struct StaticGraphBuilder<'a> {
    pub id: GraphIndex,
    pub class_name: &'a ClassName,
    pub gen_id: &'a Arc<GraphIdGenerator>,
}

impl<'a> StaticGraphBuilder<'a> {
    pub fn static_graph_obj_type(class_name: &str) -> ObjectType {
        ObjectType::class_obj_type(&format!("{}_{}", class_name, &"Static"))
    }

    pub fn static_graph_root_name(class_name: &str) -> RootName {
        root_name!(format!("{}_{}", class_name, &"Static"))
    }

    pub fn populate_static_graph<'b, I>(
        &self,
        fields: I,
        root_node: NodeIndex,
        graphs_map: &mut GraphsMap,
        classes: &mut TsClasses,
    ) -> Arc<ObjectGraph>
    where
        I: IntoIterator<Item = &'b JsFieldDescription>,
    {
        let graph_id = self.id;
        graphs_map.new_static_graph(graph_id);
        self.add_root_node(graphs_map, graph_id, root_node);

        for field in fields.into_iter().filter(|field| field.is_static) {
            self.add_static_field(field, graph_id, graphs_map, root_node, classes);
        }

        graphs_map.get(&graph_id).unwrap().clone()
    }

    fn add_root_node(
        &self,
        graphs_map: &mut GraphsMap,
        graph_id: GraphIndex,
        root_node: NodeIndex,
    ) {
        graphs_map.construct_node(
            graph_id,
            root_node,
            Self::static_graph_obj_type(&self.class_name),
            fields!(),
        );
        graphs_map.set_as_root(
            Self::static_graph_root_name(&self.class_name),
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
    ) {
        let init_expr = field.init_expr.as_ref().unwrap();
        let init_val = get_value_from_expr(init_expr, graph_id, graphs_map, classes, self.gen_id);

        graphs_map.set_field(
            field_name!(field.name.as_str()),
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
                    field: field_name!(field.name.as_str()),
                    attrs: Attributes::default(),
                }),
                val: init_val,
            },
        );
    }
}
