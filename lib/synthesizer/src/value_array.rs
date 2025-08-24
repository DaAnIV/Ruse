use std::{collections::HashMap, fmt::Display, hash::Hash, ops::Index};

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

    pub fn json_display(&self, ctx_arr: &ContextArray) -> impl Display {
        let values = self
            .iter()
            .zip(ctx_arr.iter())
            .enumerate()
            .map(|(i, (val, ctx))| {
                (
                    format!("{}.mermaid", i.to_string()),
                    val.val()
                        .mermaid_display_with_name(&ctx.graphs_map, &format!("value_{}", i))
                        .to_string(),
                )
            })
            .collect();

        WrappedValueArrayJsonDisplay { values }
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

#[derive(serde::Serialize)]
pub struct WrappedValueArrayJsonDisplay {
    values: HashMap<String, String>,
}

impl Display for WrappedValueArrayJsonDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = serde_json::to_string_pretty(self).unwrap();
        write!(f, "{}", value)
    }
}
