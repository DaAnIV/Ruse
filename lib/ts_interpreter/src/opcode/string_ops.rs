use std::sync::Arc;

use ruse_object_graph::{str_cached, Cache, Number};
use ruse_synthesizer::opcode::{ExprAst, ExprOpcode};
use ruse_synthesizer::value::*;
use ruse_synthesizer::{context::*, vnum, vstr};

use crate::opcode::member_call_ast;

#[derive(Debug)]
pub struct SplitOp {
    arg_types: [ValueType; 2],
}

impl SplitOp {
    pub fn new() -> Self {
        Self {
            arg_types: [ValueType::String, ValueType::String],
        }
    }
}

impl ExprOpcode for SplitOp {
    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        cache: &Arc<Cache>,
    ) -> Option<LocValue> {
        debug_assert_eq!(args.len(), 2);

        let string = args[0].val().string_value().unwrap();
        let pattern = args[1].val().string_value().unwrap();

        let substrings = string.split(pattern.as_str());
        let array = Value::create_primitive_array_object(
            &ValueType::String,
            substrings.map(|x| str_cached!(cache; x)),
            cache,
        );

        Some(post_ctx.temp_value(array))
    }

    fn to_ast(&self, children: &Vec<Box<dyn ExprAst>>) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), 2);
        member_call_ast("split", children)
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}

#[derive(Debug)]
pub struct ConcatOp {
    arg_types: [ValueType; 2],
}

impl ConcatOp {
    pub fn new() -> Self {
        Self {
            arg_types: [ValueType::String, ValueType::String],
        }
    }
}

impl ExprOpcode for ConcatOp {
    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        cache: &Arc<Cache>,
    ) -> Option<LocValue> {
        debug_assert_eq!(args.len(), 2);

        let string1 = args[0].val().string_value().unwrap();
        let string2 = args[1].val().string_value().unwrap();

        let mut new_string = String::with_capacity(string1.len() + string2.len());
        new_string.push_str(&string1);
        new_string.push_str(&string2);

        Some(post_ctx.temp_value(vstr!(cache; &new_string)))
    }

    fn to_ast(&self, children: &Vec<Box<dyn ExprAst>>) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), 2);
        member_call_ast("concat", children)
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}

#[derive(Debug)]
pub struct SliceOp {
    arg_types: [ValueType; 2],
}

impl SliceOp {
    pub fn new() -> Self {
        Self {
            arg_types: [ValueType::String, ValueType::Number],
        }
    }
}

impl ExprOpcode for SliceOp {
    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        cache: &Arc<Cache>,
    ) -> Option<LocValue> {
        debug_assert_eq!(args.len(), 2);

        let string = args[0].val().string_value().unwrap();
        let index_start = args[1].val().number_value().unwrap().0 as usize;
        if index_start >= string.len() {
            return None;
        }

        let substring = &string[index_start..];

        Some(post_ctx.temp_value(vstr!(cache; substring)))
    }

    fn to_ast(&self, children: &Vec<Box<dyn ExprAst>>) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), 2);

        member_call_ast("slice", children)
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}

#[derive(Debug)]
pub struct SliceWithEndOp {
    arg_types: [ValueType; 3],
}

impl SliceWithEndOp {
    pub fn new() -> Self {
        Self {
            arg_types: [ValueType::String, ValueType::Number, ValueType::Number],
        }
    }
}

impl ExprOpcode for SliceWithEndOp {
    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        cache: &Arc<Cache>,
    ) -> Option<LocValue> {
        debug_assert_eq!(args.len(), 2);

        let string = args[0].val().string_value().unwrap();
        let index_start = args[1].val().number_value().unwrap().0 as usize;
        let index_end = args[2].val().number_value().unwrap().0 as usize;
        if index_start >= string.len() || index_end < index_start || index_end > string.len() {
            return None;
        }

        let substring = &string[index_start..index_end];

        Some(post_ctx.temp_value(vstr!(cache; substring)))
    }

    fn to_ast(&self, children: &Vec<Box<dyn ExprAst>>) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), 2);

        member_call_ast("slice", children)
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}

#[derive(Debug)]
pub struct LastIndexOfOp {
    arg_types: [ValueType; 2],
}

impl LastIndexOfOp {
    pub fn new() -> Self {
        Self {
            arg_types: [ValueType::String, ValueType::String],
        }
    }
}

impl ExprOpcode for LastIndexOfOp {
    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        _cache: &Arc<Cache>,
    ) -> Option<LocValue> {
        debug_assert_eq!(args.len(), 2);

        let string = args[0].val().string_value().unwrap();
        let pat = args[1].val().string_value().unwrap();

        let index = match string.rfind(pat.as_str()) {
            Some(i) => Number::from(i),
            None => Number::from(-1),
        };

        Some(post_ctx.temp_value(vnum!(index)))
    }

    fn to_ast(&self, children: &Vec<Box<dyn ExprAst>>) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), 2);

        member_call_ast("lastIndexOf", children)
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}
