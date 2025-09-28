use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Error;
use ruse_object_graph::{ClassName, GraphIndex, GraphsMap, NodeIndex, ObjectType};
use ruse_synthesizer::context::GraphIdGenerator;
use ruse_synthesizer::synthesizer_context::SynthesizerContextData;
use swc_common::{
    errors::{ColorConfig, Handler},
    Mark, SourceFile, SourceMap,
};
use swc_ecma_ast::{self as ast, Pass};
use swc_ecma_parser::{Syntax, TsSyntax};
use swc_ecma_visit::{VisitMutWith, VisitWith};
use tracing::info_span;

use crate::{
    dts_visitor::DtsVisitor,
    engine_context::EngineContext,
    jsbuiltins::{jsarray::BuiltinArrayClass, jsmap::BuiltinMapClass},
    program_visitor::ProgramVisitor,
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

    pub fn get_user_class(&self, class: &ClassName) -> Option<&TsUserClass> {
        let base_class_name = class.split("<").next().unwrap();
        self.user_classes.get(base_class_name)
    }

    pub fn get_user_class_mut(&mut self, class: &ClassName) -> Option<&mut TsUserClass> {
        let base_class_name = class.split("<").next().unwrap();
        self.user_classes.get_mut(base_class_name)
    }

    pub fn get_global_class(&self) -> Option<&TsGlobalClass> {
        self.global_class.as_ref()
    }

    pub(crate) fn init_engine_ctx(&self, engine_ctx: &mut EngineContext) {
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

    pub fn user_classes(&self) -> impl Iterator<Item = &TsUserClass> {
        self.user_classes.values()
    }

    pub(crate) fn builtin_classes(&self) -> impl Iterator<Item = &dyn TsBuiltinClass> {
        self.builtin_classes.values().map(|class| class.as_ref())
    }

    pub fn classes_names(&self) -> impl Iterator<Item = &ClassName> {
        self.user_classes
            .values()
            .map(|class| class.get_class_name())
            .chain(
                self.builtin_classes
                    .values()
                    .map(|class| class.get_class_name()),
            )
    }
}

#[derive(Default)]
pub struct TsClassesBuilderOptions {
    pub print_code: bool,
}

pub struct TsClassesBuilder {
    compiler: swc::Compiler,
    cm: Arc<SourceMap>,

    dts_visitor: DtsVisitor,
    program_visitor: ProgramVisitor,

    options: TsClassesBuilderOptions,
}

enum FileType {
    Ts,
    Js,
    Dts,
}

impl TryFrom<&Path> for FileType {
    type Error = anyhow::Error;

    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        match path.extension().and_then(|x| x.to_str()) {
            Some("js") => Ok(FileType::Js),
            Some("ts") => Ok(FileType::Ts),
            Some("d.ts") => Ok(FileType::Dts),
            _ => Err(anyhow::anyhow!(
                "Unknown file extension: {}",
                path.display()
            )),
        }
    }
}

impl TsClassesBuilder {
    pub fn new() -> Self {
        Self::new_with_options(TsClassesBuilderOptions::default())
    }

    pub fn new_with_options(options: TsClassesBuilderOptions) -> Self {
        let cm = Arc::<SourceMap>::default();
        Self {
            compiler: swc::Compiler::new(cm.clone()),
            cm,
            dts_visitor: DtsVisitor::default(),
            program_visitor: ProgramVisitor::default(),
            options: options,
        }
    }

    pub fn add_classes(&mut self, code: &str) -> Result<(), Error> {
        let file_name = PathBuf::from("temp.ts");
        let src = self
            .cm
            .new_source_file(Arc::new(file_name.into()), code.to_string());
        self.add_file(src, FileType::Ts)
    }

    pub fn add_files<P: AsRef<Path>>(&mut self, full_path: P) -> Result<(), Error> {
        if !full_path.as_ref().exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("{} doesn't exist", full_path.as_ref().display()),
            )
            .into());
        } else if full_path.as_ref().is_dir() {
            for entry in walkdir::WalkDir::new(full_path)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|e| {
                    e.file_type().is_file()
                        && e.path()
                            .extension()
                            .map(|s| s == "ts" || s == "js" || s == "d.ts")
                            .unwrap_or(false)
                })
            {
                let file_type = FileType::try_from(entry.path())?;
                let source_file = self.cm.load_file(entry.path())?;
                self.add_file(source_file, file_type)?;
            }

            Ok(())
        } else {
            let file_type = FileType::try_from(full_path.as_ref())?;
            let source_file = self.cm.load_file(full_path.as_ref())?;
            self.add_file(source_file, file_type)
        }
    }

    fn print_code(
        &self,
        source_file: &str,
        program: Option<&ast::Program>,
        dts: Option<&ast::Program>,
    ) {
        if !self.options.print_code {
            return;
        }

        if let Some(program) = program {
            let program_code = self
                .compiler
                .print(program, swc::PrintArgs::default())
                .unwrap();
            println!("{} program code:", source_file);
            println!("{}", program_code.code);
            println!("--------------------------------");
        }

        if let Some(dts) = dts {
            let dts_code = self.compiler.print(dts, swc::PrintArgs::default()).unwrap();
            println!("{} DTS code:", source_file);
            println!("{}", dts_code.code);
            println!("--------------------------------");
        }
    }

    fn add_file(&mut self, source_file: Arc<SourceFile>, extension: FileType) -> Result<(), Error> {
        let file_name = source_file.name.to_string();
        let _span =
            info_span!(target: "ruse:TsClassesBuilder", "Parsing file", file_name = &file_name)
                .entered();

        match extension {
            FileType::Ts => {
                let (program, dts) = self.parse_ts_file(source_file)?;
                self.print_code(&file_name, Some(&program), Some(&dts));

                self.add_declarations(dts)?;
                self.parse_program(program)
            }
            FileType::Js => {
                let program = self.parse_js_file(source_file)?;
                self.print_code(&file_name, Some(&program), None);

                self.parse_program(program)
            }
            FileType::Dts => {
                let dts = self.parse_dts_file(source_file)?;
                self.print_code(&file_name, None, Some(&dts));

                self.add_declarations(dts)?;
                Ok(())
            }
        }
    }

    fn parse_ts_file(
        &mut self,
        source_file: Arc<SourceFile>,
    ) -> Result<(ast::Program, ast::Program), Error> {
        let handler =
            Handler::with_tty_emitter(ColorConfig::Auto, true, false, Some(self.cm.clone()));

        swc_common::GLOBALS.set(&Default::default(), || {
            let mut program = self.compiler.parse_js(
                source_file.clone(),
                &handler,
                ast::EsVersion::Es2022,
                Syntax::Typescript(TsSyntax {
                    tsx: false,
                    decorators: false,
                    dts: false,
                    no_early_errors: false,
                    disallow_ambiguous_jsx_like: false,
                }),
                swc::config::IsModule::Bool(true),
                None,
            )?;

            let unresolved_mark = Mark::new();
            let top_level_mark = Mark::new();

            let mut optimized = self.compiler.run_transform(&handler, false, || {
                program.mutate(&mut swc_ecma_transforms::resolver(
                    unresolved_mark,
                    top_level_mark,
                    true,
                ));

                program.visit_mut_with(&mut swc_ecma_transforms::hygiene::hygiene());
                program
            });

            let mut dts_prog = optimized.clone();
            let mut fast_dts = swc_typescript::fast_dts::FastDts::new(
                source_file.name.clone(),
                unresolved_mark,
                swc_typescript::fast_dts::FastDtsOptions {
                    internal_annotations: None,
                    add_types_to_private_properties: true,
                },
            );
            let issues = fast_dts.transform(&mut dts_prog);

            for issue in issues {
                handler
                    .struct_span_err(issue.range.span, &issue.message)
                    .emit();
            }

            let mut stripper =
                swc_ecma_transforms::typescript::strip(unresolved_mark, top_level_mark);
            stripper.process(&mut optimized);

            let mut simplifier =
                swc_ecma_transforms::optimization::simplifier(unresolved_mark, Default::default());
            simplifier.process(&mut optimized);

            Ok((optimized, dts_prog))
        })
    }

    fn parse_dts_file(&mut self, source_file: Arc<SourceFile>) -> Result<ast::Program, Error> {
        let handler =
            Handler::with_tty_emitter(ColorConfig::Auto, true, false, Some(self.cm.clone()));

        swc_common::GLOBALS.set(&Default::default(), || {
            self.compiler.parse_js(
                source_file,
                &handler,
                ast::EsVersion::Es2022,
                Syntax::Typescript(TsSyntax {
                    tsx: false,
                    decorators: false,
                    dts: true,
                    no_early_errors: false,
                    disallow_ambiguous_jsx_like: false,
                }),
                swc::config::IsModule::Bool(true),
                None,
            )
        })
    }

    fn parse_js_file(&mut self, source_file: Arc<SourceFile>) -> Result<ast::Program, Error> {
        let handler =
            Handler::with_tty_emitter(ColorConfig::Auto, true, false, Some(self.cm.clone()));

        swc_common::GLOBALS.set(&Default::default(), || {
            let mut program = self.compiler.parse_js(
                source_file,
                &handler,
                ast::EsVersion::Es2022,
                Syntax::Es(Default::default()),
                swc::config::IsModule::Bool(true),
                None,
            )?;

            let unresolved_mark = Mark::new();
            let top_level_mark = Mark::new();

            let mut optimized = self.compiler.run_transform(&handler, false, || {
                program.mutate(&mut swc_ecma_transforms::resolver(
                    unresolved_mark,
                    top_level_mark,
                    false,
                ));

                program.visit_mut_with(&mut swc_ecma_transforms::hygiene::hygiene());
                program
            });

            let mut simplifier =
                swc_ecma_transforms::optimization::simplifier(unresolved_mark, Default::default());
            simplifier.process(&mut optimized);

            Ok(optimized)
        })
    }

    fn add_declarations(&mut self, dts: ast::Program) -> Result<(), Error> {
        dts.visit_with(&mut self.dts_visitor);
        Ok(())
    }

    fn parse_program(&mut self, program: ast::Program) -> Result<(), Error> {
        program.visit_with(&mut self.program_visitor);
        Ok(())
    }

    fn get_builtin_classes(&self) -> HashMap<String, Box<dyn TsBuiltinClass>> {
        let mut builtin_classes: HashMap<String, Box<dyn TsBuiltinClass>> = HashMap::new();
        builtin_classes.insert(
            BuiltinArrayClass::CLASS_NAME.to_string(),
            Box::new(BuiltinArrayClass::new(builtin_classes.len() as u64)),
        );
        builtin_classes.insert(
            BuiltinMapClass::CLASS_NAME.to_string(),
            Box::new(BuiltinMapClass::new(builtin_classes.len() as u64)),
        );
        builtin_classes
    }

    pub fn finalize(self) -> Result<Box<TsClasses>, Error> {
        let builtin_classes = self.get_builtin_classes();

        let gen_id = Arc::new(GraphIdGenerator::with_initial_values(
            NodeIndex(0),
            GraphIndex(builtin_classes.len()),
        ));

        let mut classes = TsClasses {
            static_classes_gen_id: gen_id,
            user_classes: Default::default(),
            builtin_classes,
            global_class: None,
        };

        let mut graphs_map = GraphsMap::default();
        let global_class_builder = TsGlobalClassBuilder::global_class(
            &self.program_visitor.functions,
            &self.program_visitor.globals,
            &self.dts_visitor.functions,
            &self.dts_visitor.globals,
            &self.program_visitor.export,
            classes.static_classes_gen_id.clone(),
        );
        global_class_builder.finalize(&mut classes, &mut graphs_map);

        for (id, class_decl) in &self.program_visitor.classes {
            let dts = self.dts_visitor.classes.get(id).unwrap();
            let builder = TsUserClassBuilder::from_class_decl(
                class_decl,
                dts,
                classes.static_classes_gen_id.clone(),
            )
            .map_err(|_| anyhow::anyhow!("Failed to build user class {}", id.0))?;
            builder.finalize(&mut classes, &mut graphs_map);
        }

        Ok(Box::new(classes))
    }
}
