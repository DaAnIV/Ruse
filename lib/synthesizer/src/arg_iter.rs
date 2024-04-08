use std::sync::Arc;

use crate::{
    bank::{ProgBank, ProgIterator}, context::Context, opcode::ExprAst, prog::SubProgram, value::ValueType,
};

pub struct ArgIterator<'a, T: ExprAst + Default, const N: usize> {
    bank: &'a ProgBank<T, N>,
    cur: Vec<Arc<SubProgram<T, N>>>,
    iters: Vec<ProgIterator<'a, T, N>>,
    iter_sizes: Vec<usize>,
    remaining_size: Vec<usize>,
    arg_types: Vec<ValueType>,
    ended: bool,
}

impl<'a, T: ExprAst + Default, const N: usize> ArgIterator<'a, T, N> {
    pub fn new(
        bank: &'a ProgBank<T, N>,
        ctx: &[Context; N],
        n: usize,
        arg_types: &'a [ValueType],
    ) -> Self {
        let mut iter = Self {
            bank: bank,
            cur: Default::default(),
            iters: Default::default(),
            iter_sizes: vec![0; arg_types.len() - 1],
            remaining_size: vec![0; arg_types.len()],
            arg_types: arg_types.to_vec(),
            ended: false,
        };

        iter.ended = !iter.fill_iterators(0, n, ctx);

        return iter;
    }

    fn fill_iterators(&mut self, i: usize, n: usize, ctx: &[Context; N]) -> bool {
        if i == self.arg_types.len() {
            return true;
        }

        if i == self.arg_types.len() - 1 {
            let mut iter = self.bank.progs(n, self.arg_types[i], ctx);
            let item = iter.next();
            if item.is_none() {
                return false;
            }
            self.iters.push(iter);
            self.cur.push(item.unwrap().clone());
            return true;
        }

        if self.iter_sizes[i] == 0 {
            self.iter_sizes[i] = n - (self.arg_types.len() - i - 1);
        } else {
            self.iter_sizes[i] -= 1;            
        }

        loop {
            if self.iter_sizes[i] == 0 {
                return false;
            }

            self.remaining_size[i] = n - self.iter_sizes[i];

            let mut iter = self.bank.progs(self.iter_sizes[i], self.arg_types[i], ctx);
            let item = iter.next();
            if item.is_none() {
                continue;
            }

            self.cur.push(item.unwrap().clone());
            self.iters.push(iter);

            if self.fill_iterators(i + 1, self.remaining_size[i], item.unwrap().post_ctx()) {
                return true;
            }
            
            self.iters.pop();
            self.cur.pop();

            self.iter_sizes[i] -= 1;
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
                let i = self.iters.len() - 1;

                self.cur.push(val.clone());
                if self.fill_iterators(i + 1, self.remaining_size[i], val.post_ctx()) {
                    break;
                }
            } else {
                self.iters.pop();
                if self.iters.is_empty() {
                    self.ended = true;
                    return
                }
            }
        }
    }
}

impl<'a, T: ExprAst + Default, const N: usize> Iterator for ArgIterator<'a, T, N> {
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
