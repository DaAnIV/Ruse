use std::{mem::replace, sync::Arc};
use tokio::select;
use tokio_util::sync::CancellationToken;

use crate::{bank::ProgBank, opcode::ExprOpcode, prog::SubProgram};

type WorkHandler = Arc<
    dyn Fn(Arc<dyn ExprOpcode>, Vec<Arc<SubProgram>>) -> Option<Arc<SubProgram>> + Send + Sync,
>;

pub struct WorkGather {
    chunk_size: usize,
    children: Vec<Arc<SubProgram>>,
    iterations: Box<Vec<usize>>,
    chunk: Box<Vec<Vec<Arc<SubProgram>>>>,
    handler: WorkHandler,
    tasks: tokio::task::JoinSet<Option<Arc<SubProgram>>>,
    cancel_token: CancellationToken,
}

impl WorkGather {
    pub fn new(handler: WorkHandler, chunk_size: usize, cancel_token: CancellationToken) -> Self {
        Self {
            chunk_size: chunk_size,
            chunk: Vec::with_capacity(chunk_size).into(),
            children: Default::default(),
            iterations: Default::default(),
            handler: handler,
            tasks: tokio::task::JoinSet::new(),
            cancel_token: cancel_token,
        }
    }

    pub fn gather_work_for_next_iteration(&mut self, bank: &ProgBank, op: &Arc<dyn ExprOpcode>) {
        self.children = Vec::with_capacity(op.arg_types().len()).into();
        self.iterations = Vec::with_capacity(op.arg_types().len()).into();
        self.add_all_tasks(bank, op);
    }

    pub async fn wait_for_all_tasks(&mut self) -> Option<Arc<SubProgram>> {
        let mut found_prog = None;
        while let Some(res) = self.tasks.join_next().await {
            if let Ok(Some(p)) = res {
                found_prog = Some(p);
                self.tasks.abort_all();
            }
        }
        return found_prog;
    }

    fn add_all_tasks(&mut self, bank: &ProgBank, op: &Arc<dyn ExprOpcode>) {
        for i in 0..op.arg_types().len() {
            self.gather_work_with_cutoff(bank, op, i);
        }
        if self.chunk.len() > 0 {
            self.perform_work(op);
        }
    }

    fn gather_work_with_cutoff(
        &mut self,
        bank: &ProgBank,
        op: &Arc<dyn ExprOpcode>,
        cutoff: usize,
    ) {
        let i = self.iterations.len();
        let last_iteration = bank.bank.len() - 1;

        if i == op.arg_types().len() {
            self.gather_work_for_iterations(bank, op);
            return;
        }

        if i == cutoff {
            self.iterations.push(last_iteration);
            self.gather_work_with_cutoff(bank, op, cutoff);
            self.iterations.pop();
        } else if i < cutoff {
            for j in 0..last_iteration {
                self.iterations.push(j);
                self.gather_work_with_cutoff(bank, op, cutoff);
                self.iterations.pop();
            }
        } else {
            for j in 0..=last_iteration {
                self.iterations.push(j);
                self.gather_work_with_cutoff(bank, op, cutoff);
                self.iterations.pop();
            }
        }
    }

    fn gather_work_for_iterations(&mut self, bank: &ProgBank, op: &Arc<dyn ExprOpcode>) {
        let iterations = &self.iterations;
        let arg_types = op.arg_types();
        for t in bank.bank[iterations[0]].0.iter() {
            let values = t.0.get(&arg_types[0]);
            if values.is_none() {
                continue;
            }
            for p in unsafe { values.unwrap_unchecked().iter() } {
                self.children.push(p.clone());
                self.gather_work_for_iterations_with_progs(bank, op);
                self.children.pop();
            }
        }
    }

    fn gather_work_for_iterations_with_progs(&mut self, bank: &ProgBank, op: &Arc<dyn ExprOpcode>) {
        let i = self.children.len();
        if i == op.arg_types().len() {
            return self.gather_work(op);
        }

        let ctx = self.children.last().unwrap().post_ctx();
        if let Some(type_map) = bank.bank[self.iterations[i]].get(ctx) {
            let values = type_map.0.get(&op.arg_types()[i]);
            if values.is_none() {
                return;
            }
            for p in unsafe { values.unwrap_unchecked().iter() } {
                self.children.push(p.clone());
                self.gather_work_for_iterations_with_progs(bank, op);
                self.children.pop();
            }
        }
    }

    fn gather_work(&mut self, op: &Arc<dyn ExprOpcode>) {
        self.chunk.push(self.children.clone());
        if self.chunk.len() == self.chunk_size {
            self.perform_work(op);
        }
    }

    fn perform_work(&mut self, op: &Arc<dyn ExprOpcode>) {
        let chunk = replace(&mut self.chunk, Vec::with_capacity(self.chunk_size).into());
        WorkGather::spawn(
            &mut self.tasks,
            chunk,
            op.clone(),
            self.handler.clone(),
            self.cancel_token.child_token(),
        );
    }

    fn spawn(
        tasks: &mut tokio::task::JoinSet<Option<Arc<SubProgram>>>,
        chunk: Box<Vec<Vec<Arc<SubProgram>>>>,
        op: Arc<dyn ExprOpcode>,
        handler: WorkHandler,
        cancel_token: CancellationToken,
    ) {
        tasks.spawn(async move {
            select! {
                _ = cancel_token.cancelled() => None,
                v = async {
                    for c in chunk.into_iter() {
                        let found_prog = handler(op.clone(), c);
                        if found_prog.is_some() {
                            return found_prog;
                        }
                    }
                    return None;
                } => v
            }
        });
    }
}
