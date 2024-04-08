use std::cmp::max;
use std::sync::Arc;

use ruse_object_graph::Cache;

use crate::context::*;
use crate::opcode::*;
use crate::value::*;

pub struct SubProgram<T, const N: usize>
where
    T: ExprAst,    
{
    pub opcode: Arc<dyn SynthesizerExprOpcode<T>>,
    pub children: Vec<Arc<SubProgram<T, N>>>,

    size: u32,
    depth: u32,
    out_type: Option<ValueType>,
    
    pre_ctx: [Context; N],
    post_ctx: Option<[Context; N]>,
    out_value: Option<[LocValue; N]>
}

fn verify_children<T: ExprAst, const N: usize>(opcode: &Arc<dyn SynthesizerExprOpcode<T>>, children: &[Arc<SubProgram<T, N>>]) -> bool {
    let pre_context = &children[0].pre_ctx()[0];

    // Verify all of the examples start from the same pre context keys
    if !children[0].pre_ctx().into_iter().any(|x| {
        for (a, b) in x.get_keys().zip(pre_context.get_keys()) {
            if a != b { return false; }
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
    if children.into_iter().zip(opcode.arg_types().into_iter()).any(|(c, t)| c.out_type.unwrap() != *t) {
        return false;
    }

    // Verify each children pre context is equal to the previous post context for all examples
    for i in 1..children.len() {
        let prev = &children[i-1];
        let cur = &children[i];
        if prev.post_ctx().into_iter().zip(cur.pre_ctx().into_iter())
            .any(|(post, pre)| post != pre) {
            return false;
        }
    }

    return true;
}

impl<T, const N: usize> SubProgram<T, N>
where
    T: ExprAst,
{
    pub fn with_opcode_and_children(opcode: Arc<dyn SynthesizerExprOpcode<T>>, children: Vec<Arc<SubProgram<T, N>>>) -> Self {
        assert!(children.len() > 0);
        debug_assert!(verify_children(&opcode, &children));

        let size = (&children).into_iter().fold(0, |acc, x| acc + x.size) + 1;
        let depth = (&children).into_iter().fold(0, |acc, x| max(acc, x.depth)) + 1;
        let pre_ctx = children[0].pre_ctx().clone();

        Self {
            opcode: opcode,
            children: children,

            size: size,
            depth: depth,

            out_type: None,
            pre_ctx: pre_ctx,
            post_ctx: None,
            out_value: None
        }
    }

    pub fn with_opcode_and_context(opcode: Arc<dyn SynthesizerExprOpcode<T>>, context: &[Context; N]) -> Self {
        Self {
            opcode: opcode,
            children: vec![],

            size: 1,
            depth: 1,

            out_type: None,
            pre_ctx: context.clone(),
            post_ctx: None,
            out_value: None
        }    
    }

    pub fn evaluate(&mut self, cache: &mut Cache) {
        let mut out_type: Option<ValueType> = None;
        let mut post_ctx = Vec::with_capacity(N);
        let mut out_value = Vec::with_capacity(N);

        for i in 0..N {
            let mut out_ctx = self.pre_ctx[i].clone();
            
            // Gather arguments
            let mut args = Vec::<&LocValue>::with_capacity(self.children.len());
            for c in &self.children {
                args.push(&c.out_value()[i]);
            }

            // Evaluate and verify the output type is consistent
            let out_val = self.opcode.eval(&mut out_ctx, &args, cache);
            assert!(out_type.is_none() || out_type.unwrap() == out_val.val.val_type());
            let _ = out_type.get_or_insert(out_val.val.val_type());

            post_ctx.push(out_ctx);
            out_value.push(out_val);
        }

        self.out_type = out_type;
        self.post_ctx = Some(post_ctx.try_into().unwrap());
        self.out_value = Some(out_value.try_into().unwrap());
    }

    pub fn get_ast(&self) -> T {
        let child_asts = (&self.children).into_iter().map(|x| x.get_ast()).collect();
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
        self.out_type.unwrap()
    }
    
    #[inline]
    pub fn pre_ctx(&self) -> &[Context; N] {
        &self.pre_ctx
    }

    #[inline]
    pub fn post_ctx(&self) -> &[Context; N] {
        self.post_ctx.as_ref().unwrap()
    }

    #[inline]
    pub fn out_value(&self) -> &[LocValue; N] {
        self.out_value.as_ref().unwrap()
    }
}
