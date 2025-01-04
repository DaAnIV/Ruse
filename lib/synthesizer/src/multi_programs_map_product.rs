use std::cell::Cell;
use std::marker::PhantomData;
use std::sync::Arc;

use itertools::Itertools;
use Option::{self as State, None as ProductEnded, Some as ProductInProgress};
use Option::{self as CurrentItems, None as NotYetPopulated, Some as Populated};

use crate::bank::{ProgramsMap, ProgramsMapIter};
use crate::prog::SubProgram;

type ProgramsMapRef<'a> = &'a ProgramsMap;

pub struct MultiProgramsMaps<'a> {
    inner: State<MultiProgramsMapsInner<'a>>,
    remaining: usize,
}

pub trait ProgramChildrenIterator {
    fn next(&mut self) -> Option<(usize, *const Vec<Arc<SubProgram>>)>;
    fn bad_children(&mut self, fail: usize);
    fn take(&mut self, n: usize);
    fn skip(&mut self, n: usize);
    fn remaining(&self) -> usize;
}

/// Internals for `MultiProduct`.
struct MultiProgramsMapsInner<'a> {
    /// Holds the iterators.
    iters: Vec<MultiProgramsMapsIter<'a>>,
    /// Not populated at the beginning then it holds the current item of each iterator.
    cur: CurrentItems<Cell<Vec<Arc<SubProgram>>>>,

    total_size: usize,
}

/// Holds the state of a single iterator within a `MultiProduct`.
struct MultiProgramsMapsIter<'a> {
    iter: ProgramsMapIter<'a>,
    map_ref: ProgramsMapRef<'a>,
    i: usize,
    restart: bool,
}

impl<'a> MultiProgramsMapsIter<'a> {
    fn new(map_ref: ProgramsMapRef<'a>) -> Self {
        Self {
            iter: map_ref.iter(),
            map_ref,
            i: 0,
            restart: false,
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
    let mut total_size = 1;
    let iters = maps
        .map(|i| {
            let map_ref = unsafe { &*i };
            total_size *= map_ref.len();
            MultiProgramsMapsIter::new(map_ref)
        })
        .collect();

    let inner = MultiProgramsMapsInner {
        iters,
        cur: NotYetPopulated,
        total_size,
    };
    MultiProgramsMaps {
        remaining: inner.total_size,
        inner: ProductInProgress(inner),
    }
}

pub fn multi_programs_map_end<'a>(_marker: PhantomData<&'a bool>) -> MultiProgramsMaps<'a> {
    MultiProgramsMaps {
        remaining: 0,
        inner: ProductEnded,
    }
}

impl<'a> MultiProgramsMapsInner<'a> {
    fn advance_progs(&mut self) -> Option<usize> {
        match &mut self.cur {
            Populated(cur_progs) => {
                debug_assert!(!self.iters.is_empty());
                // Find (from the right) a non-finished iterator and
                // reset the finished ones encountered.
                for (i, iter) in self.iters.iter_mut().enumerate().rev() {
                    if !iter.restart {
                        if let Some(new) = iter.iter.next() {
                            iter.i += 1;
                            cur_progs.get_mut()[i] = (*new).clone();
                            return Some(i);
                        }
                    }

                    iter.iter = iter.map_ref.iter();
                    iter.i = 0;
                    iter.restart = false;
                    cur_progs.get_mut()[i] = (*iter.iter.next().unwrap()).clone();
                }
                None
            }
            // Only the first time.
            NotYetPopulated => {
                let next: Option<Vec<_>> = self
                    .iters
                    .iter_mut()
                    .map(|iter| {
                        iter.i += 1;
                        iter.iter.next()
                    })
                    .collect();
                if next.is_none() || self.iters.is_empty() {
                    // This cartesian product had at most one item to generate and now ends.
                    return None;
                } else {
                    let progs = next
                        .unwrap()
                        .iter()
                        .map(|p| (*p).clone())
                        .collect_vec();
                    self.cur = Populated(progs.into());
                }
                Some(0)
            }
        }
    }

    fn skip(&mut self, n: usize) {
        if n == 0 {
            return;
        }
        let mut remaining = n;

        if self.cur.is_none() {
            self.advance_progs();
            remaining -= 1;
        }
        let cur_progs = unsafe { self.cur.as_mut().unwrap_unchecked() };

        let mut rev_iterators = self.iters.iter_mut().enumerate().rev();

        while remaining > 0 {
            // Can unsafe unwrap because of the assert!(n < (self.total_size - self.i))
            let (i, iter) = rev_iterators.next().unwrap();

            let remainder = remaining % iter.map_ref.len();
            remaining /= iter.map_ref.len();

            if remainder == 0 {
                continue;
            }
            if iter.i + remainder > iter.map_ref.len() {
                let cur_iter_n = iter.i + remainder - iter.map_ref.len();
                remaining += 1;
                iter.iter = iter.map_ref.iter();
                iter.i = cur_iter_n;
                iter.restart = false;
                cur_progs.get_mut()[i] = (*iter.iter.nth(cur_iter_n).unwrap()).clone();
            } else {
                iter.i = remainder;
                cur_progs.get_mut()[i] = (*iter.iter.nth(remainder - 1).unwrap()).clone();
            }
        }
    }
}

impl<'a> ProgramChildrenIterator for MultiProgramsMaps<'a> {
    fn next(&mut self) -> Option<(usize, *const Vec<Arc<SubProgram>>)> {
        if self.remaining == 0 {
            self.inner = ProductEnded;
        }

        // This fuses the iterator.
        let inner = self.inner.as_mut()?;
        if let Some(i) = inner.advance_progs() {
            self.remaining -= 1;
            let cur = inner.cur.as_ref().unwrap().as_ptr();
            return Some((i, cur));
        }
        self.inner = ProductEnded;
        None
    }

    fn remaining(&self) -> usize {
        self.remaining
    }

    fn bad_children(&mut self, fail: usize) {
        let inner = match self.inner.as_mut() {
            ProductInProgress(inner) => inner,
            ProductEnded => return,
        };

        // restart all iterators after the failure
        for iter in inner.iters.iter_mut().skip(fail + 1) {
            iter.restart = true;
        }
    }

    fn skip(&mut self, n: usize) {
        if n >= self.remaining() {
            self.inner = ProductEnded;
            return;
        }

        let inner = match self.inner.as_mut() {
            ProductInProgress(inner) => inner,
            ProductEnded => return,
        };
        inner.skip(n);
    }

    fn take(&mut self, n: usize) {
        self.remaining = self.remaining.min(n);
    }
}
