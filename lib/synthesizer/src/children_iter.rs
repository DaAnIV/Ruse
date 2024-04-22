use std::sync::Arc;

use crate::{
    bank::{ContextArray, ProgBank, ProgIterator, TypeMap},
    opcode::ExprAst,
    prog::SubProgram,
    value::ValueType,
};

pub struct ChildrenIterator<'a, T: ExprAst + Default, const N: usize> {
    bank: &'a ProgBank<T, N>,
    cur: Vec<Arc<SubProgram<T, N>>>,
    iters: Vec<ProgIterator<'a, T, N>>,
    start_ctx_iter: std::collections::hash_map::Keys<'a, ContextArray<N>, TypeMap<T, N>>,
    arg_types: Vec<ValueType>,
    iterations: Vec<usize>,
    ended: bool,

    cutoff: usize,
    iteration: usize,
}

impl<'a, T: ExprAst + Default, const N: usize> ChildrenIterator<'a, T, N> {
    pub fn new(bank: &'a ProgBank<T, N>, iteration: usize, arg_types: &'a [ValueType]) -> Self {
        let mut iter = Self {
            bank: bank,
            cur: Default::default(),
            iters: Default::default(),
            arg_types: arg_types.to_vec(),
            ended: false,
            cutoff: 0,
            iteration: iteration,
            iterations: vec![0; arg_types.len()],
            start_ctx_iter: bank.get(iteration - 1).unwrap().keys(),
        };

        iter.ended = !iter.init_cutoff();

        return iter;
    }

    fn init_cutoff(&mut self) -> bool {
        if self.cutoff == self.arg_types.len() {
            return false;
        }
        if self.iteration == 0 && self.cutoff != 0 {
            return false;
        }

        self.iterations[0] = 0;
        self.iterations[self.cutoff] = self.iteration - 1;
        self.start_ctx_iter = self.bank.get(self.iterations[0]).unwrap().keys();

        return self.start_filling_iterators();
    }

    fn start_filling_iterators(&mut self) -> bool {
        match self.start_ctx_iter.next() {
            None => {
                self.iterations[0] += 1;
                if self.ended_iterations(0) {
                    self.cutoff += 1;
                    return self.init_cutoff();
                }
                self.start_ctx_iter = self.bank.get(self.iterations[0]).unwrap().keys();
                return self.start_filling_iterators();
            }
            Some(ctx) => {
                return self.fill_iterators(0, ctx);
            }
        }
    }

    fn ended_iterations(&self, i: usize) -> bool {
        if i == self.cutoff {
            return self.iterations[i] != self.iteration - 1;
        }
        if i < self.cutoff {
            return self.iterations[i] == self.iteration - 1;
        }
        return self.iterations[i] > 0;
    }

    fn fill_iterators(&mut self, i: usize, ctx: &ContextArray<N>) -> bool {
        if i == self.arg_types.len() {
            return true;
        }

        self.iterations[i] = if self.cutoff == i { self.iteration - 1 } else { 0 };

        loop {
            if self.ended_iterations(i) {
                return false;
            }

            let mut iter = if i > self.cutoff {
                self.bank.progs(self.arg_types[i], ctx)
            } else {
                self.bank.progs_for_iteration(self.iterations[i], self.arg_types[i], ctx)
            };
            let item = iter.next();
            if item.is_none() {
                self.iterations[i] += 1;
                continue;
            }

            self.cur.push(item.unwrap().clone());
            self.iters.push(iter);

            if self.fill_iterators(i + 1, item.unwrap().post_ctx()) {
                return true;
            }

            self.iters.pop();
            self.cur.pop();

            self.iterations[i] += 1;
        }
    }

    fn advance(&mut self) {
        if self.ended {
            return;
        }
        loop {
            self.cur.pop();
            let last_iter = self.iters.last_mut().unwrap();
            if let Some(val) = last_iter.next() {
                self.cur.push(val.clone());
                if self.fill_iterators(self.iters.len(), val.post_ctx()) {
                    break;
                }
            } else {
                self.iters.pop();
                if self.iters.is_empty() {
                    self.ended = !self.start_filling_iterators();
                    break;
                }
            }
        }
    }
}

impl<'a, T: ExprAst + Default, const N: usize> Iterator for ChildrenIterator<'a, T, N> {
    type Item = Vec<Arc<SubProgram<T, N>>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.ended {
            return None;
        }
        let prev = self.cur.clone();

        self.advance();

        return Some(prev);
    }
}
