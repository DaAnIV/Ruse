use itertools::{Itertools, MultiProduct};
use ruse_object_graph::ValueType;
use std::marker::PhantomData;
use std::ops::RangeInclusive;
use std::sync::Arc;
use Option::{self as State, None as ProductEnded, Some as ProductInProgress};

use crate::multi_programs_map_product::{MultiProgramsMaps, ProgramChildrenIterator};
use crate::prog::SubProgram;
use crate::{
    bank::ProgBank,
    multi_programs_map_product::{multi_programs_map_end, multi_programs_map_product},
};

pub struct BankIterator<'a, B: ProgBank> {
    inner: State<BankIteratorInner<'a, B>>,
    remaining: usize,
}

/// Internals for `MultiProduct`.
struct BankIteratorInner<'a, B: ProgBank> {
    bank: &'a B,
    arg_types: &'a [ValueType],

    cutoff: usize,
    iterations_iter: MultiProduct<RangeInclusive<usize>>,

    /// Holds the iterators.
    iter: MultiProgramsMaps<'a, B>,

    total_size: usize,
}

impl<'a, B: ProgBank> BankIteratorInner<'a, B> {
    fn get_iterations_iter(
        bank: &'a B,
        arg_types: &'a [ValueType],
        cutoff: usize,
    ) -> MultiProduct<RangeInclusive<usize>> {
        let last_iteration = bank.iteration_count() - 1;
        (0..arg_types.len())
            .map(|i| match i {
                n if n == cutoff => last_iteration..=last_iteration,
                n if n < cutoff => 0..=(last_iteration - 1),
                _ => 0..=last_iteration,
            })
            .multi_cartesian_product()
    }

    async fn calculate_size(bank: &'a B, arg_types: &'a [ValueType]) -> usize {
        let n = if bank.iteration_count() == 1 {
            1
        } else {
            arg_types.len()
        };

        let mut size: usize = 0;
        for i in 0..n {
            for iterations in Self::get_iterations_iter(bank, arg_types, i) {
                let mut acc = 1;
                for i in 0..iterations.len() {
                    acc *= bank.number_of_programs(iterations[i], &arg_types[i]).await;
                }
                size += acc;
            }
        }

        size
    }
}

impl<'a, P: ProgBank> BankIteratorInner<'a, P> {
    async fn new(bank: &'a P, arg_types: &'a [ValueType]) -> Self {
        Self {
            bank,
            arg_types,

            cutoff: 0,
            iterations_iter: BankIteratorInner::get_iterations_iter(bank, arg_types, 0),

            iter: multi_programs_map_end(PhantomData),

            total_size: BankIteratorInner::calculate_size(bank, arg_types).await,
        }
    }

    async fn set_programs_iter(&mut self, iterations: &[usize]) -> bool {
        for i in 0..self.arg_types.len() {
            let num = self
                .bank
                .number_of_programs(iterations[i], &self.arg_types[i])
                .await;

            if num == 0 {
                return false;
            }
        }

        self.iter = multi_programs_map_product(
            self.bank,
            iterations
                .iter()
                .zip(self.arg_types.iter())
                .map(|(iteration, output_type)| (*iteration, output_type.clone())),
        )
        .await;
        true
    }

    fn get_next_iterations_iter(&mut self) -> bool {
        self.cutoff += 1;
        if self.cutoff >= self.arg_types.len() || self.bank.iteration_count() == 1 {
            return false;
        }

        self.iterations_iter = Self::get_iterations_iter(self.bank, self.arg_types, self.cutoff);

        true
    }

    async fn get_next_programs_iter(&mut self) -> bool {
        loop {
            while let Some(iterations) = self.iterations_iter.next() {
                if self.set_programs_iter(&iterations).await {
                    return true;
                }
            }

            if !self.get_next_iterations_iter() {
                break;
            }
        }

        false
    }

    async fn skip(&mut self, n: usize) {
        let mut remaining_to_skip = n;
        while remaining_to_skip > 0 {
            if remaining_to_skip > self.iter.remaining() {
                remaining_to_skip -= self.iter.remaining();
                self.get_next_programs_iter().await;
            } else {
                self.iter.skip(remaining_to_skip).await;
                break;
            }
        }
    }
}

pub async fn bank_iterator<'a, P: ProgBank>(
    bank: &'a P,
    arg_types: &'a [ValueType],
) -> BankIterator<'a, P> {
    let inner = BankIteratorInner::new(bank, arg_types).await;
    BankIterator {
        remaining: inner.total_size,
        inner: ProductInProgress(inner),
    }
}

impl<'a, P: ProgBank> ProgramChildrenIterator for BankIterator<'a, P> {
    async fn next(&mut self) -> Option<(usize, *const Vec<Arc<SubProgram>>)> {
        if self.remaining == 0 {
            self.inner = ProductEnded;
        }
        let inner = self.inner.as_mut()?;
        loop {
            if let Some(children) = inner.iter.next().await {
                self.remaining -= 1;
                return Some(children);
            }

            if !inner.get_next_programs_iter().await {
                break;
            }
        }

        self.inner = ProductEnded;
        None
    }

    fn bad_children(&mut self, fail: usize) {
        let inner = match self.inner.as_mut() {
            ProductInProgress(inner) => inner,
            ProductEnded => return,
        };

        inner.iter.bad_children(fail);
    }

    fn take(&mut self, n: usize) {
        self.remaining = self.remaining.min(n);
    }

    fn remaining(&self) -> usize {
        self.remaining
    }

    async fn skip(&mut self, n: usize) {
        if n >= self.remaining {
            self.inner = ProductEnded;
            return;
        }

        let inner = match self.inner.as_mut() {
            ProductInProgress(inner) => inner,
            ProductEnded => return,
        };
        inner.skip(n).await;
        self.remaining -= n;
    }
}
