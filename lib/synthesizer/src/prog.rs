use std::cmp::max;
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::sync::Arc;

use ruse_object_graph::{graph_map_value::*, ValueType};

use crate::context::ContextArray;
use crate::context::SynthesizerContext;
use crate::location::*;
use crate::opcode::*;
use crate::value_array::ValueArray;

#[cfg(feature = "trace_evaluations")]
macro_rules! evaluate_trace {
    ($($arg:tt)+) => { tracing::trace!(target: "ruse::prog::evaluate", $($arg)+); }
}
#[cfg(not(feature = "trace_evaluations"))]
macro_rules! evaluate_trace {
    ($($arg:tt)+) => {};
}

pub struct SubProgram {
    pub opcode: Arc<dyn ExprOpcode>,
    pub children: Vec<Arc<SubProgram>>,

    size: u32,
    depth: u32,
    out_type: Option<ValueType>,

    pre_ctx: ContextArray,
    post_ctx: ContextArray,
    out_value: Option<ValueArray>,
    pub dirty: Option<bool>,
}

fn verify_children(opcode: &Arc<dyn ExprOpcode>, children: &[Arc<SubProgram>]) -> bool {
    // Verify the opcode arguments count match the children count
    if opcode.arg_types().len() != children.len() {
        return false;
    }

    // Verify the opcode arguments types match the children types
    if children
        .iter()
        .zip(opcode.arg_types().iter())
        .any(|(c, t)| c.out_type.as_ref().unwrap() != t)
    {
        return false;
    }

    true
}

impl SubProgram {
    pub fn with_opcode_and_children(
        opcode: Arc<dyn ExprOpcode>,
        children: Vec<Arc<SubProgram>>,
        mut pre_ctx: ContextArray,
        post_ctx: ContextArray,
    ) -> Arc<Self> {
        assert!(!children.is_empty());
        debug_assert!(verify_children(&opcode, &children));

        let size = children.iter().fold(0, |acc, x| acc + x.size) + 1;
        let depth = children.iter().fold(0, |acc, x| max(acc, x.depth)) + 1;

        pre_ctx.iter_mut().for_each(|ctx| {
            ctx.clear_outputs();
        });

        Arc::new(Self {
            opcode,
            children,

            size,
            depth,

            out_type: None,
            pre_ctx,
            post_ctx,
            out_value: None,
            dirty: None,
        })
    }

    pub fn with_opcode(
        opcode: Arc<dyn ExprOpcode>,
        mut pre_ctx: ContextArray,
        post_ctx: ContextArray,
    ) -> Arc<Self> {
        pre_ctx.iter_mut().for_each(|ctx| {
            ctx.clear_outputs();
        });

        Arc::new(Self {
            opcode,
            children: Default::default(),

            size: 1,
            depth: 1,

            out_type: None,
            pre_ctx,
            post_ctx,
            out_value: None,
            dirty: None,
        })
    }

    pub fn evaluate(&mut self, syn_ctx: &SynthesizerContext) -> bool {
        let mut out_type: Option<ValueType> = None;
        let examples_count = self.pre_ctx().len();
        let mut out_value = Vec::with_capacity(examples_count);
        let mut dirty = false;
        let old_post_ctx = self.post_ctx.clone();

        evaluate_trace!({ 
            pre_ctx = %self.pre_ctx.json_display(), 
            post_ctx = %self.post_ctx.json_display() 
        }, "Evaluating: {}", self.get_code());

        for i in 0..examples_count {
            // Gather arguments
            let args: Vec<&LocValue> = self.children.iter().map(|c| &c.out_value()[i]).collect();
            let out_ctx = self.post_ctx.get_mut(i).unwrap();
            out_ctx.clear_outputs();
            evaluate_trace!({ prog_context = ?out_ctx }, "post_ctx[{}] (before)", i);

            // Evaluate and verify the output type is consistent
            let out_val = match self.opcode.eval(&args, out_ctx, syn_ctx) {
                Ok(result) => {
                    if result.dirty && out_ctx.as_ref() != &old_post_ctx[i] {
                        dirty |= true;
                    }
                    result.output
                }
                Err(_) => return false,
            };
            evaluate_trace!({ prog_context = ?out_ctx }, "post_ctx[{}] (after)", i);

            if let Some(p_out_type) = &out_type {
                if p_out_type != &out_val.val.val_type() {
                    return false;
                }
            }
            out_type.get_or_insert(out_val.val.val_type());

            out_ctx.add_output(out_val.val().clone());
            out_value.push(out_val);
        }

        if dirty {
            self.post_ctx.depth += 1;
        }
        self.dirty = Some(dirty);
        self.out_type = out_type;
        self.out_value = Some(out_value.into());

        evaluate_trace!({ 
            pre_ctx = %self.pre_ctx.json_display(), 
            post_ctx = %self.post_ctx.json_display(), 
            output = %self.out_value().wrap(self.post_ctx()) 
        }, "Finished evaluating: {}", self.get_code());

        true
    }

    pub fn get_ast(&self) -> Box<dyn ExprAst> {
        let child_asts: Vec<Box<dyn ExprAst>> = self.children.iter().map(|x| x.get_ast()).collect();
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
    pub fn out_type(&self) -> &ValueType {
        self.out_type.as_ref().unwrap()
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

    #[inline]
    pub fn dirty(&self) -> bool {
        self.dirty.unwrap()
    }

    #[inline]
    pub fn is_terminal(&self) -> bool {
        self.opcode.is_terminal()
    }
}

impl Hash for SubProgram {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.pre_ctx().hash(state);
        self.out_type().hash(state);
        self.post_ctx().hash(state);
        self.out_value().wrap(self.post_ctx()).hash(state);
    }
}

impl Eq for SubProgram {}

impl PartialEq for SubProgram {
    fn eq(&self, other: &Self) -> bool {
        self.out_type == other.out_type
            && self.pre_ctx == other.pre_ctx
            && self.post_ctx == other.post_ctx
            && match (&self.out_value, &other.out_value) {
                (None, None) => true,
                (Some(self_out_val), Some(other_out_val)) => {
                    self_out_val.eq(&self.post_ctx, other_out_val, &other.post_ctx)
                }
                (_, _) => false,
            }
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
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "pre ctx: ({})", self.pre_ctx()[0])?;
        writeln!(f, "code: {{ {} }}", self.get_code())?;
        write!(
            f,
            "(output: {}; post ctx: {})",
            self.out_value()[0]
                .val()
                .wrap(&self.post_ctx()[0].graphs_map),
            self.post_ctx()[0]
        )?;

        Ok(())
    }
}

#[macro_export]
macro_rules! trace_prog {
    (target: $target:expr, $prog:expr, $message:expr) => {      
        if tracing::enabled!(tracing::Level::TRACE) {   
            tracing::trace!(target: $target, { 
                prog = %$prog,
                pre_ctx.json = %$prog.pre_ctx().json_display(),
                post_ctx.json = %$prog.post_ctx().json_display(),
                out_value.json = %$prog.out_value().json_display($prog.post_ctx())
             }, "{}. program \"{}\"", $message, $prog.get_code());
        }
     }
}
