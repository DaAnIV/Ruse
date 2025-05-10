use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use boa_engine::JsResult;
use ruse_object_graph::{
    class_name,
    value::{ObjectValue, Value},
    FieldName, GraphsMap, NodeIndex, ObjectGraph, ObjectType, ValueType,
};
use ruse_synthesizer::{
    context::GraphIdGenerator,
    location::LocValue,
    opcode::{ExprOpcode, OpcodesList},
};

use crate::{
    engine_context::EngineContext,
    js_value::{args_to_js_args, js_value_to_value},
    opcode::GlobalFunctionOp,
    ts_class::*,
    ts_classes::TsClasses,
};

use swc_ecma_ast::{FnDecl, VarDecl};

#[derive(Debug)]
pub struct TsGlobalClass {
    pub fields: HashMap<String, JsFieldDescription>,
    pub methods: HashMap<String, MethodDescription>,
    pub variables_opcodes: OpcodesList,
    pub function_opcodes: OpcodesList,
    pub static_graph: Option<Arc<ObjectGraph>>,
    pub root_node: NodeIndex,
    pub static_fields: HashMap<FieldName, LocValue>,
    static_graph_obj_type: ObjectType,
}

impl TsGlobalClass {
    pub fn call_function<'a, I>(
        &self,
        method_name: &str,
        params: I,
        classes: &TsClasses,
        engine_ctx: &mut EngineContext<'_>,
    ) -> boa_engine::JsResult<Value>
    where
        I: IntoIterator<Item = &'a Value>,
    {
        let js_args = args_to_js_args(params, classes, engine_ctx)?;

        let result = engine_ctx.call_global_function(method_name, &js_args)?;

        js_value_to_value(classes, &result, engine_ctx)
    }

    pub fn static_object_value(&self) -> Option<ObjectValue> {
        let static_graph = self.static_graph.as_ref()?;
        Some(ObjectValue {
            obj_type: self.static_graph_obj_type.clone(),
            graph_id: static_graph.id,
            node: self.root_node,
        })
    }

    pub(crate) fn register_class(&self, engine_ctx: &mut EngineContext<'_>) -> JsResult<()> {
        engine_ctx.register_global_class(&self)
    }
}

unsafe impl Send for TsGlobalClass {}
unsafe impl Sync for TsGlobalClass {}

pub(crate) struct TsGlobalClassBuilder {
    id: u64,
    fields: HashMap<String, JsFieldDescription>,
    methods: HashMap<String, MethodDescription>,
    gen_id: Arc<GraphIdGenerator>,
}

// Main functions
impl TsGlobalClassBuilder {
    const CLASS_NAME: &'static str = "GlobalClass";

    pub fn global_class(
        functions: Vec<(Vec<String>, FnDecl)>,
        variables: Vec<(Vec<String>, Box<VarDecl>)>,
        exports: HashSet<String>,
        gen_id: Arc<GraphIdGenerator>,
    ) -> Self {
        let mut fields = HashMap::default();
        let mut methods = HashMap::default();

        for func in functions {
            let func_name = get_name_with_namespace(&func.0, func.1.ident.sym.as_str());
            let mut desc = MethodDescription::from(&func.1);
            if exports.contains(&func_name) {
                assert!(desc
                    .params
                    .iter()
                    .all(|p| !matches!(p.value_type, ValueType::Null)));
                desc.is_private = false;
            }
            methods.insert(func_name, desc);
        }
        for var in variables {
            for decl in var.1.decls.iter() {
                let var_name =
                    get_name_with_namespace(&var.0, decl.name.as_ident().unwrap().sym.as_str());
                let mut desc = JsFieldDescription::from(decl);
                if exports.contains(&var_name) {
                    desc.is_private = false;
                }
                fields.insert(var_name, desc);
            }
        }

        let class = Self {
            id: gen_id.get_id_for_graph() as u64,
            fields,
            methods,
            gen_id,
        };

        class
    }

    pub fn finalize(self, classes: &mut TsClasses, graphs_map: &mut GraphsMap) {
        let static_graph_obj_type = StaticGraphBuilder::static_graph_obj_type(Self::CLASS_NAME);

        let root_node = classes.static_classes_gen_id.get_id_for_node();

        let static_graph = if self.fields.values().any(|field| field.is_static) {
            let static_graph_builder = StaticGraphBuilder {
                id: self.id,
                class_name: &class_name!(Self::CLASS_NAME),
                gen_id: &self.gen_id,
            };
            Some(static_graph_builder.populate_static_graph(
                self.fields.values(),
                root_node,
                graphs_map,
                classes,
            ))
        } else {
            None
        };

        let mut class = TsGlobalClass {
            fields: self.fields.clone(),
            methods: self.methods.clone(),
            variables_opcodes: Default::default(),
            function_opcodes: Default::default(),
            static_graph,
            root_node,
            static_fields: Default::default(),
            static_graph_obj_type,
        };

        self.populate_opcodes(&mut class);
        classes.set_global_class(class);
    }
}

impl TsGlobalClassBuilder {
    fn populate_opcodes(&self, class: &mut TsGlobalClass) {
        class.function_opcodes.extend(
            self.methods
                .values()
                .filter_map(|m| self.get_function_opcode(m)),
        );
    }

    fn get_function_opcode(&self, method: &MethodDescription) -> Option<Arc<dyn ExprOpcode>> {
        if method.is_private {
            return None;
        }

        Some(Arc::new(GlobalFunctionOp::new(method)))
    }
}
