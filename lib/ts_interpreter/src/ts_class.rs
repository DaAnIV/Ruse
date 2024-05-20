use std::{collections::HashMap, ops::Deref, sync::Arc};

use dashmap::DashMap;
use ruse_object_graph::{str_cached, Cache, CachedString};
use ruse_synthesizer::{
    context::Context,
    synthesizer::OpcodesList,
    value::{ObjectValue, Value},
};
use swc_common::{
    errors::{ColorConfig, Handler},
    FileName, SourceMap,
};
use swc_ecma_ast as ast;

use anyhow::Error;
use swc_ecma_parser::{Syntax, TsConfig};

use crate::{
    js_value::value_to_js_value,
    opcode::{ClassMethodOp, MemberOp},
};

#[derive(Clone)]
pub struct TsClasses {
    classes: Arc<DashMap<CachedString, TsClass>>,
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
        };
        boa_engine::JsObject::from_proto_and_data(
            intrinsics.constructors().object().prototype(),
            global_obj,
        )
    }
}

impl TsClasses {
    pub fn new() -> Self {
        Self {
            classes: Default::default(),
        }
    }

    pub fn add_class(&self, code: String, cache: &Cache) -> Result<CachedString, Error> {
        let class = TsClass::from_code(self, code, cache)?;
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

    pub fn generate_js_object(
        &self,
        class: &CachedString,
        obj: ObjectValue,
        boa_ctx: &mut boa_engine::Context,
        cache: &Arc<Cache>,
    ) -> boa_engine::JsObject {
        self.get_class(class)
            .generate_js_object(&self, obj, boa_ctx, cache)
    }

    pub fn create_boa_ctx(&self) -> boa_engine::Context {
        boa_engine::context::ContextBuilder::default()
            .host_hooks(&TsContextHooks)
            .build()
            .expect("Failed to build context")
    }

    pub fn class_members_opcodes(&self, class: &CachedString) -> OpcodesList {
        self.get_class(class).member_opcodes().clone()
    }

    pub fn class_method_opcodes(&self, class: &CachedString) -> OpcodesList {
        self.get_class(class).method_opcodes().clone()
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
        let cache = global_ctx.cache.as_ref().unwrap();

        let field = js_object_value.get_field_value(field_name).unwrap();

        Ok(value_to_js_value(self, &field, boa_ctx, cache))
    }

    fn object_setter(
        &self,
        _this: &boa_engine::JsValue,
        _field_name: &CachedString,
        _boa_ctx: &mut boa_engine::Context,
    ) -> boa_engine::JsResult<boa_engine::JsValue> {
        // Implementation of the object_getter function goes here
        unimplemented!()
    }
}

struct TsClass {
    class: Box<ast::Class>,
    class_name: CachedString,
    member_opcodes: OpcodesList,
    method_opcodes: OpcodesList,
}

impl TsClass {
    fn from_code(classes: &TsClasses, code: String, cache: &Cache) -> Result<Self, Error> {
        let script = match TsClass::get_ast(code) {
            Ok(ast) => ast.script().unwrap(),
            Err(e) => return Err(e),
        };

        let class_decl = script.body[0].as_decl().unwrap().as_class().unwrap();

        let mut class = Self {
            class: class_decl.class.clone(),
            class_name: str_cached!(cache; class_decl.ident.sym.as_str()),
            member_opcodes: Default::default(),
            method_opcodes: Default::default(),
        };

        class.populate_opcodes(classes, cache);

        Ok(class)
    }

    fn member_opcodes(&self) -> &OpcodesList {
        &self.member_opcodes
    }

    fn method_opcodes(&self) -> &OpcodesList {
        &self.method_opcodes
    }

    fn generate_object(&self, root_name: CachedString, map: HashMap<CachedString, Value>) -> Value {
        Value::generate_object_from_map(root_name, self.class_name.clone(), map)
    }

    fn generate_js_object(
        &self,
        classes: &TsClasses,
        obj: ObjectValue,
        boa_ctx: &mut boa_engine::Context,
        cache: &Arc<Cache>,
    ) -> boa_engine::JsObject {
        assert!(obj.obj_type() == self.class_name);

        let mut builder =
            boa_engine::object::ObjectInitializer::with_native_data(TsObjectValue(obj), boa_ctx);
        for member in self.class.body.clone().iter() {
            match member {
                ast::ClassMember::Constructor(constructor) => {
                    self.add_accessors_from_constructor(&mut builder, classes, &constructor, cache);
                }
                _ => (),
            };
        }
        builder.build()
    }

    fn add_accessors_from_constructor(
        &self,
        obj_initializer: &mut boa_engine::object::ObjectInitializer,
        classes: &TsClasses,
        constructor: &ast::Constructor,
        cache: &Arc<Cache>,
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
                        move |this, _, boa_ctx| {
                            setter_classes.object_setter(this, &setter_field_name, boa_ctx)
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

    fn populate_opcodes(&mut self, classes: &TsClasses, cache: &Cache) {
        for member in self.class.body.clone().iter() {
            match member {
                ast::ClassMember::Constructor(constructor) => {
                    self.add_opcodes_from_constructor(constructor, cache);
                }
                ast::ClassMember::Method(m) => self.add_method_opcode(classes, m, cache),
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

    fn add_method_opcode(&mut self, classes: &TsClasses, method: &ast::ClassMethod, cache: &Cache) {
        let method_name = method.key.as_ident().unwrap().sym.to_string();
        let mut args = Vec::with_capacity(method.function.params.len());

        let c = swc::Compiler::new(Arc::<SourceMap>::default());

        let mut print_args: swc::PrintArgs = Default::default();
        print_args.source_map = swc::config::SourceMapsConfig::Bool(false);
        print_args.codegen_config.target = ast::EsVersion::Es2022;
        let function_body = c
            .print(method.function.body.as_ref().unwrap(), print_args)
            .expect("Failed to get code")
            .code;
        match &method.function.type_params {
            Some(params) => {
                for param in &params.params {
                    println!("{:?}", param);
                }
            }
            None => (),
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
}

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

pub(crate) struct TsGlobalObject {
    pub(crate) cache: Option<Arc<Cache>>,
    pub(crate) context: Option<Context>,
}

impl<'a> boa_gc::Finalize for TsGlobalObject {}

unsafe impl<'a> boa_gc::Trace for TsGlobalObject {
    boa_gc::empty_trace!();
}

impl<'a> boa_engine::JsData for TsGlobalObject {}
