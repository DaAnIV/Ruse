use std::sync::Arc;

use crate::{
    context::ContextArray,
    embedding::{embeddings_trace, merge_context_arrays},
    multi_programs_map_product::ProgramChildrenIterator,
    prog::SubProgram,
    prog_triplet::ProgTriplet,
};
use Option::{self as CurrentItems, None as NotYetPopulated, Some as Populated};

#[cfg(feature = "trace_embeddings")]
use itertools::Itertools;

pub struct ProgTripletIterator<I>
where
    I: ProgramChildrenIterator,
{
    children_iterator: I,
    cur_ctxs: CurrentItems<Vec<(ContextArray, ContextArray)>>,
}

impl<I> ProgTripletIterator<I>
where
    I: ProgramChildrenIterator,
{
    fn get_ctxs(
        &mut self,
        cur_progs: &Vec<Arc<SubProgram>>,
        mut from: usize,
    ) -> Option<&(ContextArray, ContextArray)> {
        embeddings_trace!(
            "trying to get ctxs for {}",
            cur_progs
                .iter()
                .map(|p| format!("\"{}\"", p.get_code()))
                .join(", ")
        );

        if self.cur_ctxs.is_none() {
            self.cur_ctxs = Populated(vec![
                (ContextArray::default(), ContextArray::default());
                cur_progs.len()
            ]);
            from = 0;
        }

        let cur_ctxs = unsafe { self.cur_ctxs.as_mut().unwrap_unchecked() };

        if from == 0 {
            if cur_progs.len() != cur_ctxs.len() {
                cur_ctxs.resize(
                    cur_progs.len(),
                    (ContextArray::default(), ContextArray::default()),
                );
            }
            embeddings_trace!(prog: cur_progs[0], "Adding context 0");
            cur_ctxs[0] = (
                cur_progs[0].pre_ctx().clone(),
                cur_progs[0].post_ctx().clone(),
            );
            from = 1;
        }

        for i in from..cur_ctxs.len() {
            let last_ctx = &cur_ctxs[i - 1];
            let p = &cur_progs[i];

            embeddings_trace!(prog: p, "Adding context {}", i);

            if let Ok(merged_ctx) =
                merge_context_arrays(&last_ctx.0, &last_ctx.1, p.pre_ctx(), p.post_ctx())
            {
                cur_ctxs[i] = merged_ctx;
            } else {
                self.children_iterator.bad_children(i);
                return None;
            }
        }

        Some(unsafe { cur_ctxs.last().unwrap_unchecked() })
    }
}

pub fn prog_triplet_iterator<I>(children_iterator: I) -> ProgTripletIterator<I>
where
    I: ProgramChildrenIterator,
{
    ProgTripletIterator {
        children_iterator,
        cur_ctxs: NotYetPopulated,
    }
}

impl<I> ProgTripletIterator<I>
where
    I: ProgramChildrenIterator,
{
    pub async fn next(&mut self) -> Option<ProgTriplet> {
        while let Some((i, cur_progs_ptr)) = self.children_iterator.next().await {
            let cur_progs = unsafe { cur_progs_ptr.as_ref().unwrap_unchecked() };
            if let Some((pre_ctx, post_ctx)) = self.get_ctxs(cur_progs, i) {
                return Some(ProgTriplet::new(
                    pre_ctx.clone(),
                    cur_progs.clone(),
                    post_ctx.clone(),
                ));
            }
        }

        None
    }
}
