use std::fmt::Display;
use std::sync::Arc;

use crate::{context::ContextArray, prog::SubProgram};

#[derive(Clone)]
pub struct ProgTriple {
    pub pre_ctx: ContextArray,
    pub children: Vec<Arc<SubProgram>>,
    pub post_ctx: ContextArray,
}

impl ProgTriple {
    pub fn new(
        pre_ctx: ContextArray,
        children: Vec<Arc<SubProgram>>,
        post_ctx: ContextArray,
    ) -> Self {
        Self {
            pre_ctx,
            children,
            post_ctx,
        }
    }
}

impl Display for ProgTriple {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "pre ctx: {}", &self.pre_ctx)?;
        writeln!(f, "children: [")?;
        for p in &self.children {
            writeln!(f, "{{")?;
            writeln!(f, "{}", p)?;
            writeln!(f, "}},")?;
        }
        writeln!(f, "]")?;
        writeln!(f, "post ctx: {}", &self.post_ctx)?;

        Ok(())
    }
}
