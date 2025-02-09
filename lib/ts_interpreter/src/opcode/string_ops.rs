use num_traits::cast::ToPrimitive;
use ruse_object_graph::Number;
use ruse_object_graph::{value::*, *};
use ruse_synthesizer::location::*;
use ruse_synthesizer::opcode::{EvalResult, ExprAst, ExprOpcode};
use ruse_synthesizer::{context::*, pure};
use swc_common::DUMMY_SP;
use swc_ecma_ast as ast;

use crate::opcode::{get_end_index, get_start_index, member_call_ast, member_field_ast, TsExprAst};

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
    fn op_name(&self) -> &str {
        "String.prototype.split"
    }

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

        pure!(post_ctx.temp_value(Value::Object(array)))
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
    fn op_name(&self) -> &str {
        "String.prototype.concat"
    }

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

        pure!(post_ctx.temp_value(vcstring!(syn_ctx.cached_string(&new_string))))
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
    fn op_name(&self) -> &str {
        "String.prototype.slice"
    }

    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        let string = args[0].val().string_value().unwrap();

        let start = get_start_index(&args[1].val().number_value().unwrap(), string.len())?;
        let end = match args.get(2) {
            Some(v) => get_end_index(&v.val().number_value().unwrap(), string.len())?,
            None => string.len(),
        };
        if start >= end {
            return pure!(post_ctx.temp_value(vcstring!(syn_ctx.cached_string(""))));
        }

        let substring = string.slice(start..end);

        pure!(post_ctx.temp_value(Value::Primitive(PrimitiveValue::String(substring))))
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        member_call_ast("slice", children)
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}

#[derive(Debug)]
pub struct StringLengthOp {
    arg_types: Vec<ValueType>,
}

impl StringLengthOp {
    pub fn new() -> Self {
        let arg_types = vec![ValueType::String];
        Self { arg_types }
    }
}

impl Default for StringLengthOp {
    fn default() -> Self {
        Self::new()
    }
}

impl ExprOpcode for StringLengthOp {
    fn op_name(&self) -> &str {
        "String: Length"
    }

    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        _: &SynthesizerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 1);

        let string = args[0].val().string_value().unwrap();

        pure!(post_ctx.temp_value(vnum!(string.len().into())))
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        member_field_ast(&children[0], "length")
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
    fn op_name(&self) -> &str {
        "String.prototype.lastIndexOf"
    }

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

        pure!(post_ctx.temp_value(vnum!(index)))
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        member_call_ast("lastIndexOf", children)
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}

#[derive(Debug)]
pub struct StringIndexOfOp {
    arg_types: [ValueType; 2],
}

impl StringIndexOfOp {
    pub fn new() -> Self {
        Self {
            arg_types: [ValueType::String, ValueType::String],
        }
    }
}

impl Default for StringIndexOfOp {
    fn default() -> Self {
        Self::new()
    }
}

impl ExprOpcode for StringIndexOfOp {
    fn op_name(&self) -> &str {
        "String.prototype.indexOf"
    }

    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        _: &SynthesizerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 2);

        let string = args[0].val().string_value().unwrap();
        let pat = args[1].val().string_value().unwrap();

        let index = match string.find(pat.as_str()) {
            Some(i) => Number::from(i),
            None => Number::from(-1),
        };

        pure!(post_ctx.temp_value(vnum!(index)))
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        member_call_ast("indexOf", children)
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}

#[derive(Debug)]
pub struct StringReplaceAllOp {
    arg_types: [ValueType; 3],
}

impl StringReplaceAllOp {
    pub fn new() -> Self {
        Self {
            arg_types: [ValueType::String, ValueType::String, ValueType::String],
        }
    }
}

impl ExprOpcode for StringReplaceAllOp {
    fn op_name(&self) -> &str {
        "String.prototype.replaceAll"
    }

    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 3);

        let string = args[0].val().string_value().unwrap();
        let pat = args[1].val().string_value().unwrap();
        let replacement = args[2].val().string_value().unwrap();

        let new_string = string.replace(pat.as_str(), replacement.as_str());

        pure!(post_ctx.temp_value(vstring!(syn_ctx.cache; new_string)))
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        member_call_ast("replaceAll", children)
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}

#[derive(Debug)]
pub struct StringAtOp {
    arg_types: [ValueType; 2],
}

impl StringAtOp {
    pub fn new() -> Self {
        Self {
            arg_types: [ValueType::String, ValueType::Number],
        }
    }
}

impl ExprOpcode for StringAtOp {
    fn op_name(&self) -> &str {
        "String.prototype.at"
    }

    fn eval(
        &self,
        args: &[&LocValue],
        post_ctx: &mut Context,
        _syn_ctx: &SynthesizerContext,
    ) -> EvalResult {
        debug_assert_eq!(args.len(), 2);

        let str_val = args[0].val().string_value().unwrap();
        let index = args[1].val().number_value().unwrap();
        let index_isize = index.to_isize().ok_or(())?;

        let index_usize = if index_isize >= 0 {
            let index_usize = index_isize as usize;
            if index_usize >= str_val.len() {
                return Err(());
            }
            index_usize
        } else {
            if (-index_isize) as usize > str_val.len() {
                return Err(());
            }
            (str_val.len() as isize + index_isize) as usize
        };

        let char_slice = str_val.slice(index_usize..=index_usize);

        pure!(post_ctx.temp_value(Value::Primitive(PrimitiveValue::String(char_slice))))
    }

    fn to_ast(&self, children: &[Box<dyn ExprAst>]) -> Box<dyn ExprAst> {
        debug_assert_eq!(children.len(), 2);

        let field = TsExprAst::from(children[1].as_ref());

        let expr = ast::MemberExpr {
            span: DUMMY_SP,
            obj: TsExprAst::from(children[0].as_ref()).get_paren_expr(),
            prop: ast::MemberProp::Computed(ast::ComputedPropName {
                span: DUMMY_SP,
                expr: field.node.to_owned(),
            }),
        };

        TsExprAst::create(ast::Expr::Member(expr))
    }

    fn arg_types(&self) -> &[ValueType] {
        &self.arg_types
    }
}
