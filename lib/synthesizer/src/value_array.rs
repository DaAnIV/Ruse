use std::{fmt::Display, hash::Hash, ops::Index};

use itertools::izip;
use ruse_object_graph::graph_map_value::GraphMapWrap;

use crate::{context::ContextArray, location::LocValue};

#[derive(Debug)]
pub struct ValueArray(Vec<LocValue>);
impl ValueArray {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> impl std::iter::Iterator<Item = &LocValue> {
        self.0.iter()
    }
}

impl From<Vec<LocValue>> for ValueArray {
    fn from(value: Vec<LocValue>) -> Self {
        Self(value)
    }
}

impl Index<usize> for ValueArray {
    type Output = LocValue;

    fn index(&self, index: usize) -> &Self::Output {
        self.0.index(index)
    }
}

impl ValueArray {
    pub fn eq(
        &self,
        self_context_array: &ContextArray,
        other: &Self,
        other_context_array: &ContextArray,
    ) -> bool {
        debug_assert!(
            self.len() == other.len()
                && self.len() == self_context_array.len()
                && other.len() == other_context_array.len()
        );
        izip!(
            self.iter(),
            self_context_array.iter(),
            other.iter(),
            other_context_array.iter()
        )
        .all(|(self_val, self_ctx, other_val, other_ctx)| {
            self_val.wrap(&self_ctx.graphs_map) == other_val.wrap(&other_ctx.graphs_map)
        })
    }
}

impl ValueArray {
    pub fn wrap<'a>(&'a self, ctx_arr: &'a ContextArray) -> WrappedValueArray<'a> {
        debug_assert!(self.len() == ctx_arr.len());
        WrappedValueArray {
            value_arr: self,
            ctx_arr,
        }
    }
}

pub struct WrappedValueArray<'a> {
    value_arr: &'a ValueArray,
    ctx_arr: &'a ContextArray,
}

impl<'a> Hash for WrappedValueArray<'a> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        for (val, ctx) in izip!(self.value_arr.iter(), self.ctx_arr.iter(),) {
            val.wrap(&ctx.graphs_map).hash(state);
        }
    }
}

impl<'a> Display for WrappedValueArray<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (val, ctx) in izip!(self.value_arr.iter(), self.ctx_arr.iter(),) {
            write!(f, "{}", val.wrap(&ctx.graphs_map))?;
        }
        Ok(())
    }
}
