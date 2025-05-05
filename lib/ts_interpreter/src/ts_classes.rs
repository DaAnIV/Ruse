use std::{
    collections::{HashMap, HashSet},
    io::Read,
    path::Path,
    sync::Arc,
};

use anyhow::Error;
use itertools::Itertools;
use ruse_object_graph::{scached, Cache, ClassName, GraphsMap, NodeIndex, ObjectType};
use ruse_synthesizer::context::{GraphIdGenerator, SynthesizerContextData};
use swc_common::{
    errors::{ColorConfig, Handler},
    FileName, SourceMap,
};
use swc_ecma_ast::{self as ast, ClassDecl, FnDecl, VarDecl};
use swc_ecma_parser::{Syntax, TsSyntax};

use crate::{
    engine_context::EngineContext,
    jsbuiltins::jsarray::BuiltinArrayClass,
    ts_class::{TsBuiltinClass, TsClass},
    ts_global_class::{TsGlobalClass, TsGlobalClassBuilder},
    ts_user_class::{TsUserClass, TsUserClassBuilder},
};

#[derive(Debug)]
pub struct TsClasses {
    pub static_classes_gen_id: Arc<GraphIdGenerator>,
    builtin_classes: HashMap<String, Box<dyn TsBuiltinClass>>,
    user_classes: HashMap<String, TsUserClass>,
    global_class: Option<TsGlobalClass>,
}

impl SynthesizerContextData for TsClasses {}

impl TsClasses {
    pub(crate) fn add_user_class(&mut self, name: String, class: TsUserClass) {
        self.user_classes.insert(name, class);
    }

    pub(crate) fn set_global_class(&mut self, class: TsGlobalClass) {
        self.global_class = Some(class);
    }

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

    pub fn get_global_class(&self) -> Option<&TsGlobalClass> {
        self.global_class.as_ref()
    }

    pub(crate) fn init_engine_ctx(&self, engine_ctx: &mut EngineContext<'_>) {
        for (_, class) in &self.user_classes {
            class
                .register_class(engine_ctx)
                .expect("Failed to register class");
        }
        if let Some(global_class) = &self.global_class {
            global_class
                .register_class(engine_ctx)
                .expect("Failed to register global class");
        }
    }

    pub(crate) fn builtin_classes(&self) -> impl Iterator<Item = &dyn TsBuiltinClass> {
        self.builtin_classes.values().map(|class| class.as_ref())
    }
}

pub struct TsClassesBuilder {
    classes: Vec<(Vec<String>, ClassDecl)>,
    functions: Vec<(Vec<String>, FnDecl)>,
    variables: Vec<(Vec<String>, Box<VarDecl>)>,
    exports: HashSet<String>,
}

impl TsClassesBuilder {
    pub fn new() -> Self {
        Self {
            classes: Default::default(),
            functions: Default::default(),
            variables: Default::default(),
            exports: Default::default(),
        }
    }

    pub fn add_classes(&mut self, code: &str, cache: &Cache) -> Result<Vec<ClassName>, Error> {
        let script = TsUserClass::get_ast(code)?.script().unwrap();
        self.parse_body(&vec![], &script.body, cache)
    }

    pub fn add_ts_files<P: AsRef<Path>>(
        &mut self,
        full_path: P,
        cache: &Cache,
    ) -> Result<Vec<ClassName>, Error> {
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
    ) -> Result<Vec<ClassName>, Error> {
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
    ) -> Result<Vec<ClassName>, Error> {
        let mut class_names = vec![];

        for stmt in body {
            match stmt {
                ast::Stmt::Decl(ast::Decl::Class(class_decl)) => {
                    let class_name =
                        scached!(cache; TsUserClassBuilder::get_class_name(namespace, class_decl));
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
                ast::Stmt::Expr(expr_stmt) => match expr_stmt.expr.as_ref() {
                    ast::Expr::Assign(assign_expr) => {
                        if let ast::AssignTarget::Simple(ast::SimpleAssignTarget::Member(
                            member_expr,
                        )) = &assign_expr.left
                        {
                            let name = member_expr.obj.as_ident().unwrap().sym.as_str();
                            let prop = member_expr.prop.as_ident().unwrap().sym.as_str();
                            if name == "module" && prop == "exports" {
                                if let ast::Expr::Lit(lit) = assign_expr.right.as_ref() {
                                    if let ast::Lit::Str(s) = lit {
                                        self.exports.insert(s.value.to_string());
                                    }
                                }
                                if let ast::Expr::Object(lit) = assign_expr.right.as_ref() {
                                    for prop in &lit.props {
                                        let val = prop.as_prop().unwrap();
                                        let ident = val.as_shorthand().unwrap();
                                        self.exports.insert(ident.sym.to_string());
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                },
                _ => {
                    println!("Unknown statement {:#?}", stmt);
                }
            }
        }

        Ok(class_names)
    }

    fn get_builtin_classes(&self, cache: &Cache) -> HashMap<String, Box<dyn TsBuiltinClass>> {
        let mut builtin_classes: HashMap<String, Box<dyn TsBuiltinClass>> = HashMap::new();
        builtin_classes.insert(
            BuiltinArrayClass::CLASS_NAME.to_string(),
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
            global_class: None,
        };

        let mut graphs_map = GraphsMap::default();
        let global_class_builder = TsGlobalClassBuilder::global_class(
            self.functions,
            self.variables,
            self.exports,
            classes.static_classes_gen_id.clone(),
            cache,
        );
        global_class_builder.finalize(&mut classes, &mut graphs_map, cache);

        for (namespace, class_decl) in &self.classes {
            let builder = TsUserClassBuilder::from_class_decl(
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
