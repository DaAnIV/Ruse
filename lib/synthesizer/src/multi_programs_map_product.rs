use std::cell::Cell;
use std::future::Future;
use std::marker::PhantomData;
use std::sync::Arc;

use itertools::Itertools;
use ruse_object_graph::ValueType;
use Option::{self as State, None as ProductEnded, Some as ProductInProgress};
use Option::{self as CurrentItems, None as NotYetPopulated, Some as Populated};

use crate::bank::ProgBank;
use crate::prog::SubProgram;

type ProgramsMapIter<'a> = Box<dyn Iterator<Item = &'a Arc<SubProgram>> + Send + 'a>;

pub struct MultiProgramsMaps<'a, B: ProgBank> {
    inner: State<MultiProgramsMapsInner<'a, B>>,
    remaining: usize,
}

pub trait ProgramChildrenIterator {
    fn next(&mut self)
        -> impl Future<Output = Option<(usize, *const Vec<Arc<SubProgram>>)>> + Send;
    fn bad_children(&mut self, fail: usize);
    fn take(&mut self, n: usize);
    fn skip(&mut self, n: usize) -> impl Future<Output = ()> + Send;
    fn remaining(&self) -> usize;
}

/// Internals for `MultiProduct`.
struct MultiProgramsMapsInner<'a, B: ProgBank> {
    bank: &'a B,
    /// Holds the iterators.
    iters: Vec<MultiProgramsMapsIter<'a>>,
    /// Not populated at the beginning then it holds the current item of each iterator.
    cur: CurrentItems<Cell<Vec<Arc<SubProgram>>>>,

    total_size: usize,
}

/// Holds the state of a single iterator within a `MultiProduct`.
struct MultiProgramsMapsIter<'a> {
    iteration: usize,
    output_type: ValueType,
    number_of_programs: usize,
    iter: ProgramsMapIter<'a>,
    i: usize,
    restart: bool,
}

impl<'a> MultiProgramsMapsIter<'a> {
    async fn new<B: ProgBank>(bank: &'a B, iteration: usize, output_type: ValueType) -> Self {
        let iter = Box::new(bank.iter_programs(iteration, &output_type).await);
        let number_of_programs = bank.number_of_programs(iteration, &output_type).await;
        Self {
            iteration,
            output_type,
            number_of_programs,
            iter,
            i: 0,
            restart: false,
        }
    }

    async fn reset_iter<B: ProgBank>(&mut self, bank: &'a B) {
        self.iter = Box::new(bank.iter_programs(self.iteration, &self.output_type).await);
        self.restart = false;
    }
}

/// Create a new cartesian product iterator over an arbitrary number
/// of iterators of the same type.
///
/// Iterator element is of type `Vec<H::Item::Item>`.
pub async fn multi_programs_map_product<'a, I, B>(bank: &'a B, maps: I) -> MultiProgramsMaps<'a, B>
where
    B: ProgBank,
    I: Iterator<Item = (usize, ValueType)>,
{
    let mut total_size = 1;
    let mut iters = Vec::new();
    for (iteration, output_type) in maps {
        let iter = MultiProgramsMapsIter::new(bank, iteration, output_type).await;
        total_size *= iter.number_of_programs;
        iters.push(iter);
    }

    let inner = MultiProgramsMapsInner {
        bank,
        iters,
        cur: NotYetPopulated,
        total_size,
    };
    MultiProgramsMaps {
        remaining: inner.total_size,
        inner: ProductInProgress(inner),
    }
}

pub fn multi_programs_map_end<'a, B: ProgBank>(
    _marker: PhantomData<&'a bool>,
) -> MultiProgramsMaps<'a, B> {
    MultiProgramsMaps {
        remaining: 0,
        inner: ProductEnded,
    }
}

impl<'a, B: ProgBank> MultiProgramsMapsInner<'a, B> {
    async fn advance_progs(&mut self) -> Option<usize> {
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

                    iter.reset_iter(self.bank).await;
                    iter.i = 0;
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
                    let progs = next.unwrap().into_iter().cloned().collect_vec();
                    self.cur = Populated(progs.into());
                }
                Some(0)
            }
        }
    }

    async fn skip(&mut self, n: usize) {
        if n == 0 {
            return;
        }
        let mut remaining = n;

        if self.cur.is_none() {
            self.advance_progs().await;
            remaining -= 1;
        }
        let cur_progs = unsafe { self.cur.as_mut().unwrap_unchecked() };

        let mut rev_iterators = self.iters.iter_mut().enumerate().rev();

        while remaining > 0 {
            // Can unsafe unwrap because of the assert!(n < (self.total_size - self.i))
            let (i, iter) = rev_iterators.next().unwrap();

            let remainder = remaining % iter.number_of_programs;
            remaining /= iter.number_of_programs;

            if remainder == 0 {
                continue;
            }
            if iter.i + remainder > iter.number_of_programs {
                let cur_iter_n = iter.i + remainder - iter.number_of_programs;
                remaining += 1;
                iter.reset_iter(self.bank).await;
                iter.i = cur_iter_n;
                cur_progs.get_mut()[i] = (*iter.iter.nth(cur_iter_n).unwrap()).clone();
            } else {
                iter.i = remainder;
                cur_progs.get_mut()[i] = (*iter.iter.nth(remainder - 1).unwrap()).clone();
            }
        }
    }
}

impl<'a, B: ProgBank> ProgramChildrenIterator for MultiProgramsMaps<'a, B> {
    async fn next(&mut self) -> Option<(usize, *const Vec<Arc<SubProgram>>)> {
        if self.remaining == 0 {
            self.inner = ProductEnded;
        }

        // This fuses the iterator.
        let inner = self.inner.as_mut()?;
        if let Some(i) = inner.advance_progs().await {
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

    async fn skip(&mut self, n: usize) {
        if n >= self.remaining() {
            self.inner = ProductEnded;
            return;
        }

        let inner = match self.inner.as_mut() {
            ProductInProgress(inner) => inner,
            ProductEnded => return,
        };
        inner.skip(n).await;
    }

    fn take(&mut self, n: usize) {
        self.remaining = self.remaining.min(n);
    }
}
