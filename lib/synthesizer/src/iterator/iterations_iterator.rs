use std::ops::RangeInclusive;

use itertools::{Itertools, MultiProduct};

pub struct IterationsIterator {
    iteration_count: usize,
    arg_count: usize,

    cutoff: usize,
    iterations_iter: MultiProduct<RangeInclusive<usize>>,
}

impl IterationsIterator {
    pub fn new(iteration_count: usize, arg_count: usize) -> Self {
        let cutoff = if iteration_count == 1 {
            0
        } else {
            arg_count - 1
        };

        Self {
            iteration_count,
            arg_count,

            cutoff,
            iterations_iter: Self::get_iterations_iter(iteration_count, arg_count, cutoff),
        }
    }

    fn get_iterations_iter(
        iteration_count: usize,
        arg_count: usize,
        cutoff: usize,
    ) -> MultiProduct<RangeInclusive<usize>> {
        let last_iteration = iteration_count - 1;
        (0..arg_count)
            .map(|i| match i {
                n if n == cutoff => last_iteration..=last_iteration,
                n if n < cutoff => 0..=(last_iteration - 1),
                _ => 0..=last_iteration,
            })
            .multi_cartesian_product()
    }

    fn set_next_iterations_iter(&mut self) -> bool {
        if self.cutoff == 0 {
            return false;
        }
        self.cutoff -= 1;

        self.iterations_iter =
            Self::get_iterations_iter(self.iteration_count, self.arg_count, self.cutoff);

        true
    }
}

impl Iterator for IterationsIterator {
    type Item = Vec<usize>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(iterations) = self.iterations_iter.next() {
            Some(iterations)
        } else if self.set_next_iterations_iter() {
            self.iterations_iter.next()
        } else {
            None
        }
    }
}
