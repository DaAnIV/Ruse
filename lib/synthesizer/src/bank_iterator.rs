use itertools::{Itertools, MultiProduct};
use ruse_object_graph::value::ValueType;
use std::marker::PhantomData;
use std::ops::RangeInclusive;
use Option::{self as State, None as ProductEnded, Some as ProductInProgress};

use crate::multi_programs_map_product::MultiProgramsMaps;
use crate::{
    bank::ProgBank,
    multi_programs_map_product::{multi_programs_map_end, multi_programs_map_product},
    prog_triplet::ProgTriplet
};

pub struct BankIterator<'a>(State<BankIteratorInner<'a>>);

/// Internals for `MultiProduct`.
struct BankIteratorInner<'a> {
    bank: &'a ProgBank,
    arg_types: &'a [ValueType],

    cutoff: usize,
    iterations_iter: MultiProduct<RangeInclusive<usize>>,

    /// Holds the iterators.
    iter: MultiProgramsMaps<'a>,
}

impl<'a> BankIteratorInner<'a> {
    fn set_programs_iter(&mut self, iterations: &[usize]) -> bool {
        if (0..self.arg_types.len())
            .any(|i| self.bank[iterations[i]].get(&self.arg_types[i]).is_none())
        {
            return false;
        }
        let program_maps = (0..self.arg_types.len()).map(|i| {
            let map_ref = self.bank[iterations[i]]
                .get(&self.arg_types[i]).unwrap();
            std::ptr::from_ref(map_ref.value())
        });

        self.iter = multi_programs_map_product(program_maps);
        true
    }

    fn get_next_iterations_iter(&mut self) -> bool {
        self.cutoff += 1;
        if self.cutoff >= self.arg_types.len() || self.bank.iteration_count() == 1 {
            return false;
        }

        let last_iteration = self.bank.iteration_count() - 1;
        self.iterations_iter = (0..self.arg_types.len())
            .map(|i| match i {
                n if n == self.cutoff => last_iteration..=last_iteration,
                n if n < self.cutoff => 0..=(last_iteration - 1),
                _ => 0..=last_iteration,
            })
            .multi_cartesian_product();

        true
    }

    fn get_next_programs_iter(&mut self) -> bool {
        loop {
            while let Some(iterations) = self.iterations_iter.next() {
                if self.set_programs_iter(&iterations) {
                    return true;
                }
            }

            if !self.get_next_iterations_iter() {
                break;
            }
        }

        false
    }
}

pub fn bank_iterator<'a>(bank: &'a ProgBank, arg_types: &'a [ValueType]) -> BankIterator<'a> {
    let last_iteration = bank.iteration_count() - 1;
    let iterations_iter = (0..arg_types.len())
        .map(|i| match i {
            n if n == 0 => last_iteration..=last_iteration,
            _ => 0..=last_iteration,
        })
        .multi_cartesian_product();

    let inner: BankIteratorInner<'a> = BankIteratorInner {
        bank,
        arg_types,

        cutoff: 0,
        iterations_iter,

        iter: multi_programs_map_end(PhantomData),
    };
    BankIterator(ProductInProgress(inner))
}

impl<'a> Iterator for BankIterator<'a> {
    type Item = ProgTriplet;

    fn next(&mut self) -> Option<Self::Item> {
        let inner = self.0.as_mut()?;
        loop {
            if let Some(triplet) = inner.iter.next() {
                return Some(triplet);
            }

            if !inner.get_next_programs_iter() {
                break;
            }
        }

        self.0 = ProductEnded;
        None
    }
}

impl<'a> std::iter::FusedIterator for BankIterator<'a> {}
