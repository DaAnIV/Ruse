use std::cmp::max;
use std::fmt::Debug;
use std::fmt::Display;
use std::hash::Hash;
use std::sync::Arc;

use ruse_object_graph::Cache;

use crate::bank::ContextArray;
use crate::bank::ValueArray;
use crate::opcode::*;
use crate::value::*;

pub struct SubProgram<T: ExprAst, const N: usize>
{
    pub opcode: Arc<dyn ExprOpcode<T>>,
    pub children: Vec<Arc<SubProgram<T, N>>>,

    size: u32,
    depth: u32,
    out_type: Option<ValueType>,

    pre_ctx: ContextArray<N>,
    post_ctx: Option<ContextArray<N>>,
    out_value: Option<ValueArray<N>>,
}

fn verify_children<T: ExprAst, const N: usize>(
    opcode: &Arc<dyn ExprOpcode<T>>,
    children: &[Arc<SubProgram<T, N>>],
) -> bool {
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

impl<T: ExprAst, const N: usize> SubProgram<T, N>
{
    pub fn with_opcode_and_children(
        opcode: Arc<dyn ExprOpcode<T>>,
        children: Vec<Arc<SubProgram<T, N>>>,
    ) -> Arc<Self> {
        assert!(children.len() > 0);
        debug_assert!(verify_children(&opcode, &children));

        let size = (&children).into_iter().fold(0, |acc, x| acc + x.size) + 1;
        let depth = (&children).into_iter().fold(0, |acc, x| max(acc, x.depth)) + 1;
        let pre_ctx = children[0].pre_ctx().clone();

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
        opcode: Arc<dyn ExprOpcode<T>>,
        context: &ContextArray<N>,
    ) -> Arc<Self> {
        Arc::new(Self {
            opcode: opcode,
            children: vec![],

            size: 1,
            depth: 1,

            out_type: None,
            pre_ctx: context.clone(),
            post_ctx: None,
            out_value: None,
        })
    }

    pub fn evaluate(&mut self, cache: &Cache) -> bool {
        let mut out_type: Option<ValueType> = None;
        let mut post_ctx = Vec::with_capacity(N);
        let mut out_value = Vec::with_capacity(N);

        for i in 0..N {
            // Gather arguments
            let mut args = Vec::<&LocValue>::with_capacity(self.children.len());
            for c in &self.children {
                args.push(&c.out_value()[i]);
            }

            let mut out_ctx = match self.children.last() {
                Some(last) => last.post_ctx()[i].clone(),
                None => self.pre_ctx[i].clone()
            };

            // Evaluate and verify the output type is consistent
            match self.opcode.eval(&args, &mut out_ctx, cache) {
                Some(out_val) => {
                    debug_assert!(out_type.is_none() || out_type == Some(out_val.val.val_type()));
                    let _ = out_type.get_or_insert(out_val.val.val_type());

                    post_ctx.push(out_ctx);
                    out_value.push(out_val);
                },
                None => return false
            };
        }

        self.out_type = out_type.clone();
        self.post_ctx = Some(Arc::new(post_ctx.try_into().unwrap()));
        self.out_value = Some(Arc::new(out_value.try_into().unwrap()));
        
        return true;
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
        self.out_type.as_ref().unwrap().clone()
    }

    #[inline]
    pub fn pre_ctx(&self) -> &ContextArray<N> {
        &self.pre_ctx
    }

    #[inline]
    pub fn post_ctx(&self) -> &ContextArray<N> {
        self.post_ctx.as_ref().unwrap()
    }

    #[inline]
    pub fn out_value(&self) -> &ValueArray<N> {
        self.out_value.as_ref().unwrap()
    }
}

impl<T: ExprAst, const N: usize> Hash for SubProgram<T, N>
{
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.pre_ctx().hash(state);
        self.out_type().hash(state);
        self.post_ctx().hash(state);
        self.out_value().hash(state);
    }
}

impl<T: ExprAst, const N: usize> Eq for SubProgram<T, N> {}

impl<T: ExprAst, const N: usize> PartialEq for SubProgram<T, N>
{
    fn eq(&self, other: &Self) -> bool {
        self.out_type == other.out_type
            && self.pre_ctx == other.pre_ctx
            && self.post_ctx == other.post_ctx
            && self.out_value == other.out_value
    }
}

impl<T: ExprAst, const N: usize> Debug for SubProgram<T, N>
{
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

impl<T: ExprAst, const N: usize> Display for SubProgram<T, N>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}) {{ {} }} ({}; {})", self.pre_ctx()[0], self.get_code(), self.out_value()[0].val(), self.post_ctx()[0])
    }
}

