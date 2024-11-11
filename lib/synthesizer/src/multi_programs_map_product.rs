use std::fmt::Display;
use std::marker::PhantomData;
use std::sync::Arc;

use itertools::Itertools;
use Option::{self as State, None as ProductEnded, Some as ProductInProgress};
use Option::{self as CurrentItems, None as NotYetPopulated, Some as Populated};

use crate::bank::ProgramsMap;
use crate::context::ContextArray;
use crate::embedding::merge_context_arrays;
use crate::prog::SubProgram;

use tracing::trace;

#[derive(Clone)]
pub struct ProgTriplet {
    pub pre_ctx: ContextArray,
    pub children: Vec<Arc<SubProgram>>,
    pub post_ctx: ContextArray,
}

impl ProgTriplet {
    fn new(pre_ctx: ContextArray, children: Vec<Arc<SubProgram>>, post_ctx: ContextArray) -> Self {
        Self {
            pre_ctx,
            children,
            post_ctx,
        }
    }
}

impl Display for ProgTriplet {
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

type ProgramsMapRef<'a> = &'a ProgramsMap;
type ProgramsMapIter<'a> = <ProgramsMapRef<'a> as IntoIterator>::IntoIter;

pub struct MultiProgramsMaps<'a>(State<MultiProgramsMapsInner<'a>>);

/// Internals for `MultiProduct`.
struct MultiProgramsMapsInner<'a> {
    /// Holds the iterators.
    iters: Vec<MultiProgramsMapsIter<'a>>,
    /// Not populated at the beginning then it holds the current item of each iterator.
    cur: CurrentItems<(Vec<Arc<SubProgram>>, Vec<(ContextArray, ContextArray)>)>,
}

/// Holds the state of a single iterator within a `MultiProduct`.
struct MultiProgramsMapsIter<'a> {
    iter: ProgramsMapIter<'a>,
    orig_iter: ProgramsMapIter<'a>,
}

impl<'a> MultiProgramsMapsIter<'a> {
    fn new(iter: ProgramsMapIter<'a>) -> Self {
        Self {
            iter: iter.clone(),
            orig_iter: iter,
        }
    }
}

/// Create a new cartesian product iterator over an arbitrary number
/// of iterators of the same type.
///
/// Iterator element is of type `Vec<H::Item::Item>`.
pub fn multi_programs_map_product<'a, I>(maps: I) -> MultiProgramsMaps<'a>
where
    I: Iterator<Item = *const ProgramsMap>,
{
    let inner = MultiProgramsMapsInner {
        iters: maps.map(|i| {
            let map_ref = unsafe {&*i};
            MultiProgramsMapsIter::new(map_ref.iter())
    }).collect(),
        cur: NotYetPopulated,
    };
    MultiProgramsMaps(ProductInProgress(inner))
}

pub fn multi_programs_map_end<'a>(_marker: PhantomData<&'a bool>) -> MultiProgramsMaps<'a> {
    MultiProgramsMaps(ProductEnded)
}

impl<'a> MultiProgramsMapsInner<'a> {
    fn advance_progs(&mut self) -> Option<usize> {
        match &mut self.cur {
            Populated((cur_progs, _)) => {
                debug_assert!(!self.iters.is_empty());
                // Find (from the right) a non-finished iterator and
                // reset the finished ones encountered.
                for (i, iter) in self.iters.iter_mut().enumerate().rev() {
                    if let Some(new) = iter.iter.next() {
                        cur_progs[i] = new.value().clone();
                        return Some(i);
                    } else {
                        iter.iter = iter.orig_iter.clone();
                        cur_progs[i] = iter.iter.next().unwrap().value().clone();
                    }
                }
                None
            }
            // Only the first time.
            NotYetPopulated => {
                let next: Option<Vec<_>> = self.iters.iter_mut().map(|i| i.iter.next()).collect();
                if next.is_none() || self.iters.is_empty() {
                    // This cartesian product had at most one item to generate and now ends.
                    return None;
                } else {
                    let progs = next
                        .unwrap()
                        .iter()
                        .map(|p| p.value().clone())
                        .collect_vec();
                    let ctxs =
                        vec![(ContextArray::default(), ContextArray::default()); progs.len()];
                    self.cur = Populated((progs, ctxs));
                }
                Some(0)
            }
        }
    }

    fn set_ctxs(&mut self, mut from: usize) -> Option<ProgTriplet> {
        let (cur_progs, cur_ctxs) = self.cur.as_mut().unwrap();

        if from == 0 {
            cur_ctxs[0] = (
                cur_progs[0].pre_ctx().clone(),
                cur_progs[0].post_ctx().clone(),
            );
            from = 1;
        }

        for i in from..cur_ctxs.len() {
            let last_ctx = &cur_ctxs[i - 1];
            let p = &cur_progs[i];
            if let Ok(merged_ctx) =
                merge_context_arrays(&last_ctx.0, &last_ctx.1, p.pre_ctx(), p.post_ctx())
            {
                cur_ctxs[i] = merged_ctx;
            } else {
                return None;
            }
        }

        trace!("{} merged ctx:", cur_ctxs.len());
        cur_ctxs.iter().enumerate().for_each(|(i, c)| {
            trace!("merged: [{}]", cur_progs.iter().take(i + 1).map(|p| p.get_code()).join(", "));
            trace!("pre_hat: {}", c.0[0]); 
            trace!("post_hat: {}", c.1[0]); 
        });
        let (pre_ctx, post_ctx) = cur_ctxs.last().unwrap().clone();
        Some(ProgTriplet::new(pre_ctx, cur_progs.clone(), post_ctx))
    }
}

impl<'a> Iterator for MultiProgramsMaps<'a> {
    type Item = ProgTriplet;

    fn next(&mut self) -> Option<Self::Item> {
        // This fuses the iterator.
        let inner = self.0.as_mut()?;
        while let Some(i) = inner.advance_progs() {
            if let Some(triplet) = inner.set_ctxs(i) {
                return Some(triplet);
            }
        }
        self.0 = ProductEnded;
        None
    }
}

impl<'a> std::iter::FusedIterator for MultiProgramsMaps<'a> {}
