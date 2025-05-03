use ruse_object_graph::{value::*, *};
use ruse_synthesizer::{
    context::{Context, SynthesizerContext},
    dirty,
    location::*,
    opcode::{EvalResult, ExprAst, ExprOpcode},
    pure,
};
use tracing::trace;

use crate::{
    engine_context::EngineContext,
    opcode::{member_call_ast, member_field_ast, new_obj_ast, static_member_call_ast},
    ts_class::{MethodDescription, MethodKind, TsClasses},
};

pub struct ClassMethodOp {
    class_name: ClassName,
    desc: MethodDescription,
    full_method_name: String,
    arg_types: Vec<ValueType>,
}

impl ClassMethodOp {
    pub fn new(class_name: ClassName, method_desc: &MethodDescription) -> Self {
        let full_method_name = if method_desc.kind == MethodKind::GlobalFunction {
            format!("{}", &method_desc.name)
        } else {
            format!("{}.{}", &class_name, &method_desc.name)
        };
        let mut arg_types = vec![];
        if !method_desc.is_static {
            arg_types.push(ValueType::Object(ObjectType::Class(class_name.clone())));
        };
        arg_types.extend(method_desc.params.iter().map(|x| x.value_type.clone()));

        Self {
            class_name,
            desc: method_desc.clone(),
            full_method_name,
            arg_types,
        }
    }
}

impl ExprOpcode for ClassMethodOp {
    fn op_name(&self) -> &str {
        &self.full_method_name
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }

    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        let classes = syn_ctx.data.downcast_ref::<TsClasses>().unwrap();
        let mut boa_ctx = EngineContext::new_boa_ctx();
        let mut engine_ctx = EngineContext::create_engine_ctx(&mut boa_ctx, classes);
        engine_ctx.reset_with_mut_context(post_ctx, classes, &syn_ctx.cache);
        let class = classes.get_user_class(&self.class_name).unwrap();

        let result = if self.desc.is_static {
            class.call_static_method(
                &self.desc.name,
                args.iter().map(|x| x.val()),
                classes,
                &syn_ctx.cache,
                &mut engine_ctx,
            )
        } else {
            class.call_method(
                &self.desc,
                args[0].val(),
                args.iter().skip(1).map(|x| x.val()),
                classes,
                &syn_ctx.cache,
                &mut engine_ctx,
            )
        };

        match result {
            // Need to check if func changed the context
            Ok(res) => {
                let output = post_ctx.temp_value(res);
                if engine_ctx.is_dirty() {
                    dirty!(output)
                } else {
                    pure!(output)
                }
            }
            Err(err) => {
                trace!(
                    "Failed to evaluate {}. error: {}",
                    self.full_method_name,
                    err
                );
                Err(())
            }
        }
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), self.arg_types.len());
        if self.desc.kind == MethodKind::Getter {
            member_field_ast(&children[0], &self.desc.name)
        } else if self.desc.kind == MethodKind::Setter {
            todo!()
        } else if self.desc.is_static {
            static_member_call_ast(&self.class_name.as_str(), self.desc.name.as_str(), children)
        } else {
            member_call_ast(self.desc.name.as_str(), children)
        }
    }
}

impl std::fmt::Debug for ClassMethodOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClassMethodOp")
            .field("desc", &self.desc)
            .finish()
    }
}

pub struct ClassConstructorOp {
    obj_type: CachedString,
    full_method_name: String,
    arg_types: Vec<ValueType>,
}

impl ClassConstructorOp {
    pub fn new(obj_type: CachedString, desc: MethodDescription) -> Self {
        let full_method_name = format!("new {}", &obj_type);

        let mut arg_types = vec![];
        arg_types.extend(desc.params.iter().map(|x| x.value_type.clone()));

        Self {
            obj_type,
            full_method_name,
            arg_types,
        }
    }
}

impl ExprOpcode for ClassConstructorOp {
    fn op_name(&self) -> &str {
        &self.full_method_name
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }

    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        let classes = syn_ctx.data.downcast_ref::<TsClasses>().unwrap();
        let mut boa_ctx = EngineContext::new_boa_ctx();
        let mut engine_ctx = EngineContext::create_engine_ctx(&mut boa_ctx, classes);
        engine_ctx.reset_with_mut_context(post_ctx, classes, &syn_ctx.cache);
        let class = classes.get_user_class(&self.obj_type).unwrap();

        let result = class.call_constructor(args.iter().map(|x| x.val()), classes, &mut engine_ctx);

        match result {
            // Need to check if func changed the context
            Ok(res) => {
                let output = post_ctx.temp_value(Value::Object(res));
                if engine_ctx.is_dirty() {
                    dirty!(output)
                } else {
                    pure!(output)
                }
            }
            Err(err) => {
                trace!(
                    "Failed to evaluate {}. error: {}",
                    self.full_method_name,
                    err
                );
                Err(())
            }
        }
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), self.arg_types.len());
        new_obj_ast(&self.obj_type, children)
    }
}

impl std::fmt::Debug for ClassConstructorOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClassConstructorOp")
            .field("obj", &self.obj_type)
            .field("arg_types", &self.arg_types)
            .finish()
    }
}
