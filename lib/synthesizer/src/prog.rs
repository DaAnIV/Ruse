use std::cmp::max;
use std::fmt::Debug;
use std::fmt::Display;
use std::hash::Hash;
use std::sync::Arc;

use ruse_object_graph::Cache;

use crate::context::ContextArray;
use crate::context::SynthesizerContext;
use crate::opcode::*;
use crate::value::*;

pub struct SubProgram {
    pub opcode: Arc<dyn ExprOpcode>,
    pub children: Vec<Arc<SubProgram>>,

    size: u32,
    depth: u32,
    out_type: Option<ValueType>,

    pre_ctx: ContextArray,
    post_ctx: ContextArray,
    out_value: Option<ValueArray>,
}

fn verify_children(opcode: &Arc<dyn ExprOpcode>, children: &[Arc<SubProgram>]) -> bool {
    let pre_context = &children[0].pre_ctx()[0];

    // Verify all of the examples start from the same pre context keys
    if !children[0].pre_ctx().iter().any(|x| {
        for (a, b) in x.get_keys().zip(pre_context.get_keys()) {
            if a != b {
                return false;
            }
        }
        return true;
    }) {
        return false;
    }

    // Verify the opcode arguments count match the children count
    if opcode.arg_types().len() != children.len() {
        return false;
    }

    // Verify the opcode arguments types match the children types
    if children
        .into_iter()
        .zip(opcode.arg_types().into_iter())
        .any(|(c, t)| c.out_type.as_ref().unwrap() != t)
    {
        return false;
    }

    // Verify each children pre context is equal to the previous post context for all examples
    for i in 1..children.len() {
        let prev = &children[i - 1];
        let cur = &children[i];
        if prev
            .post_ctx()
            .iter()
            .zip(cur.pre_ctx().iter())
            .any(|(post, pre)| post != pre)
        {
            return false;
        }
    }

    return true;
}

impl SubProgram {
    pub fn with_opcode_and_children(
        opcode: Arc<dyn ExprOpcode>,
        children: Vec<Arc<SubProgram>>,
    ) -> Arc<Self> {
        assert!(children.len() > 0);
        debug_assert!(verify_children(&opcode, &children));

        let size = children.iter().fold(0, |acc, x| acc + x.size) + 1;
        let depth = children.iter().fold(0, |acc, x| max(acc, x.depth)) + 1;
        let pre_ctx = children.first().unwrap().pre_ctx().clone();
        let post_ctx = children.last().unwrap().post_ctx().clone();

        Arc::new(Self {
            opcode: opcode,
            children: children,

            size: size,
            depth: depth,

            out_type: None,
            pre_ctx: pre_ctx,
            post_ctx: None,
            out_value: None,
        })
    }

    pub fn with_opcode_and_context(
        opcode: Arc<dyn ExprOpcode>,
        context: &ContextArray,
    ) -> Arc<Self> {
        Arc::new(Self {
            opcode: opcode,
            children: Default::default(),

            size: 1,
            depth: 1,

            out_type: None,
            pre_ctx: context.clone(),
            post_ctx: context.clone(),
            out_value: None,
        })
    }

    pub fn evaluate(&mut self, context: &SynthesizerContext) -> bool {
        let mut out_type: Option<ValueType> = None;
        let examples_count = self.pre_ctx().len();
        let mut out_value = Vec::with_capacity(examples_count);
        let mut dirty = false;

        for i in 0..examples_count {
            // Gather arguments
            let args: Vec<&LocValue> = self.children.iter().map(|c| &c.out_value()[i]).collect();
            let out_ctx = self.post_ctx.get_mut(i).unwrap();

            // Evaluate and verify the output type is consistent
            let out_val = match self.opcode.eval(&args, out_ctx, context) {
                EvalResult::DirtyContext(out_val) => {
                    dirty = true;
                    out_val
                }
                EvalResult::NoModification(out_val) => out_val,
                EvalResult::None => return false,
            };

            debug_assert!(out_type.is_none() || out_type == Some(out_val.val.val_type()));
            let _ = out_type.get_or_insert(out_val.val.val_type());

            out_value.push(out_val);
        }

        if dirty {
            self.post_ctx.depth += 1;
        }
        self.out_type = out_type.clone();
        self.out_value = Some(Arc::new(out_value));

        return true;
    }

    pub fn get_ast(&self) -> Box<dyn ExprAst> {
        let child_asts = self.children.iter().map(|x| x.get_ast()).collect();
        self.opcode.to_ast(&child_asts)
    }

    pub fn get_code(&self) -> String {
        self.get_ast().to_string()
    }

    #[inline]
    pub fn size(&self) -> u32 {
        self.size
    }

    #[inline]
    pub fn depth(&self) -> u32 {
        self.depth
    }

    #[inline]
    pub fn out_type(&self) -> ValueType {
        self.out_type.as_ref().unwrap().clone()
    }

    #[inline]
    pub fn pre_ctx(&self) -> &ContextArray {
        &self.pre_ctx
    }

    #[inline]
    pub fn post_ctx(&self) -> &ContextArray {
        &self.post_ctx
    }

    #[inline]
    pub fn out_value(&self) -> &ValueArray {
        self.out_value.as_ref().unwrap()
    }
}

impl Hash for SubProgram {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.pre_ctx().hash(state);
        self.out_type().hash(state);
        self.post_ctx().hash(state);
        self.out_value().hash(state);
    }
}

impl Eq for SubProgram {}

impl PartialEq for SubProgram {
    fn eq(&self, other: &Self) -> bool {
        self.out_type == other.out_type
            && self.pre_ctx == other.pre_ctx
            && self.post_ctx == other.post_ctx
            && self.out_value == other.out_value
    }
}

impl Debug for SubProgram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubProgram")
            .field("code", &self.get_code())
            .field("out_type", &self.out_type)
            .field("pre_ctx", &self.pre_ctx)
            .field("post_ctx", &self.post_ctx)
            .field("out_value", &self.out_value)
            .finish()
    }
}

impl Display for SubProgram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "({}) {{ {} }} ({}; {})",
            self.pre_ctx()[0],
            self.get_code(),
            self.out_value()[0].val(),
            self.post_ctx()[0]
        )
    }
}
