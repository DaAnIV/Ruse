use ruse_object_graph::Number;
use ruse_object_graph::{value::*, *};
use ruse_synthesizer::context::*;
use ruse_synthesizer::location::*;
use ruse_synthesizer::opcode::{EvalResult, ExprAst, ExprOpcode};

use crate::opcode::{get_end_index, get_start_index, member_call_ast};

#[derive(Debug)]
pub struct StringSplitOp {
    arg_types: [ValueType; 2],
}

impl StringSplitOp {
    pub fn new() -> Self {
        Self {
            arg_types: [ValueType::String, ValueType::String],
        }
    }
}

impl Default for StringSplitOp {
    fn default() -> Self {
        Self::new()
    }
}

impl ExprOpcode for StringSplitOp {
    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 2);

        let string = args[0].val().string_value().unwrap();
        let pattern = args[1].val().string_value().unwrap();

        let substrings = string.split(pattern.as_str());
        let array = post_ctx.create_output_primitive_array(
            &ValueType::String,
            substrings.map(|x| syn_ctx.cached_string(x)),
            &syn_ctx,
        );

        EvalResult::NoModification(post_ctx.temp_value(Value::Object(array)))
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), 2);
        member_call_ast("split", children)
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}

#[derive(Debug)]
pub struct StringConcatOp {
    arg_types: [ValueType; 2],
}

impl StringConcatOp {
    pub fn new() -> Self {
        Self {
            arg_types: [ValueType::String, ValueType::String],
        }
    }
}

impl Default for StringConcatOp {
    fn default() -> Self {
        Self::new()
    }
}

impl ExprOpcode for StringConcatOp {
    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 2);

        let string1 = args[0].val().string_value().unwrap();
        let string2 = args[1].val().string_value().unwrap();

        let mut new_string = String::with_capacity(string1.len() + string2.len());
        new_string.push_str(&string1);
        new_string.push_str(&string2);

        EvalResult::NoModification(
            post_ctx.temp_value(vcstring!(syn_ctx.cached_string(&new_string))),
        )
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), 2);
        member_call_ast("concat", children)
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}

#[derive(Debug)]
pub struct StringSliceOp {
    arg_types: Vec<ValueType>,
}

impl StringSliceOp {
    pub fn new(with_end: bool) -> Self {
        let mut arg_types = vec![ValueType::String, ValueType::Number];
        if with_end {
            arg_types.push(ValueType::Number);
        }
        Self { arg_types }
    }
}

impl ExprOpcode for StringSliceOp {
    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        let string = args[0].val().string_value().unwrap();

        let start = get_start_index(
            args[1].val().number_value().unwrap().0 as isize,
            string.len(),
        );
        let end = match args.get(2) {
            Some(v) => get_end_index(v.val().number_value().unwrap().0 as isize, string.len()),
            None => string.len(),
        };
        if start >= end {
            return EvalResult::NoModification(
                post_ctx.temp_value(vcstring!(syn_ctx.cached_string(""))),
            );
        }

        let substring = &string[start..end];

        EvalResult::NoModification(post_ctx.temp_value(vcstring!(syn_ctx.cached_string(substring))))
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        member_call_ast("slice", children)
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}

#[derive(Debug)]
pub struct StringLastIndexOfOp {
    arg_types: [ValueType; 2],
}

impl StringLastIndexOfOp {
    pub fn new() -> Self {
        Self {
            arg_types: [ValueType::String, ValueType::String],
        }
    }
}

impl Default for StringLastIndexOfOp {
    fn default() -> Self {
        Self::new()
    }
}

impl ExprOpcode for StringLastIndexOfOp {
    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        _: &SynthesizerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 2);

        let string = args[0].val().string_value().unwrap();
        let pat = args[1].val().string_value().unwrap();

        let index = match string.rfind(pat.as_str()) {
            Some(i) => Number::from(i),
            None => Number::from(-1),
        };

        EvalResult::NoModification(post_ctx.temp_value(vnum!(index)))
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        member_call_ast("lastIndexOf", children)
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}
