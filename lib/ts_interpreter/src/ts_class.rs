use std::{collections::HashMap, sync::Arc};

use dashmap::DashMap;
use ruse_object_graph::{str_cached, Cache, CachedString};
use ruse_synthesizer::{
    synthesizer::OpcodesList,
    value::Value,
};
use swc_common::{
    errors::{ColorConfig, Handler},
    FileName, SourceMap,
};
use swc_ecma_ast as ast;

use anyhow::Error;
use swc_ecma_parser::{Syntax, TsConfig};

impl TsClasses {
    pub fn new() -> Self {
        Self {
            classes: Default::default(),
        }
    }

    pub fn add_class(&self, code: String, cache: &Cache) -> Result<CachedString, Error> {
        let class = TsClass::from_code(code, cache)?;
        let class_name = class.class_name.clone();
        self.classes.insert(class_name.clone(), class);
        Ok(class_name.clone())
    }

    fn get_class(&self, class: &CachedString) -> dashmap::mapref::one::Ref<Arc<String>, TsClass> {
        self.classes.get(class).unwrap()
    }

    pub fn generate_object(
        &self,
        class: &CachedString,
        root_name: CachedString,
        map: HashMap<CachedString, Value>,
    ) -> Value {
        self.get_class(class).generate_object(root_name, map)
    }
use crate::opcode::{MemberOp, TsExprAst};

pub struct TsClass {

    pub fn class_members_opcodes(&self, class: &CachedString) -> OpcodesList {
        self.get_class(class).member_opcodes().clone()
    }
}

struct TsClass {
    class: Box<ast::Class>,
    class_name: CachedString,
    member_opcodes: OpcodesList<TsExprAst>,
}

impl TsClass {
    pub fn from_code(code: String, cache: &Cache) -> Result<Self, Error> {
        let script = match TsClass::get_ast(code) {
            Ok(ast) => ast.script().unwrap(),
            Err(e) => return Err(e),
        };

        let class_decl = script.body[0].as_decl().unwrap().as_class().unwrap();

        let mut class = Self {
            class: class_decl.class.clone(),
            class_name: str_cached!(cache; class_decl.ident.sym.as_str()),
            member_opcodes: Default::default(),
        };

        class.populate_opcodes(cache);

        Ok(class)
    }

    fn member_opcodes(&self) -> &OpcodesList {
        &self.member_opcodes
    }

    fn generate_object(&self, root_name: CachedString, map: HashMap<CachedString, Value>) -> Value {
        Value::generate_object_from_map(root_name, self.class_name.clone(), map)
    }

        &self,
        name: CachedString,
        map: HashMap<CachedString, Value>,
    ) -> Value {
        Value::generate_object_from_map(name, self.class_name.clone(), map)
    }

    fn get_ast(code: String) -> Result<ast::Program, Error> {
        let cm = Arc::<SourceMap>::default();
        let handler = Handler::with_tty_emitter(ColorConfig::Auto, true, false, Some(cm.clone()));
        let c = swc::Compiler::new(cm.clone());

        let fm = cm.new_source_file(FileName::Anon, code);

        match c.parse_js(
            fm,
            &handler,
            ast::EsVersion::Es2022,
            Syntax::Typescript(TsConfig::default()),
            swc::config::IsModule::Bool(false),
            None,
        ) {
            Ok(v) => Ok(v),
            Err(e) => Err(e),
        }
    }

    fn populate_opcodes(&mut self, cache: &Cache) {
        for member in self.class.body.clone().iter() {
            match member {
                ast::ClassMember::Constructor(constructor) => {
                    self.add_opcodes_from_constructor(constructor, cache);
                }
                ast::ClassMember::Method(_) => todo!(),
                ast::ClassMember::ClassProp(_) => todo!(),
                ast::ClassMember::TsIndexSignature(_) => todo!(),
                ast::ClassMember::StaticBlock(_) => todo!(),
                ast::ClassMember::AutoAccessor(_) => todo!(),
                _ => continue,
            };
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

            let member = str_cached!(cache; ts_param.param.as_ident().unwrap().sym.as_str());
            let accessor = Arc::new(MemberOp::new(self.class_name.clone(), member));
            self.member_opcodes.push(accessor);
        }
    }
}
