use std::{mem::replace, sync::Arc};

use crate::{
    bank::ProgBank,
    opcode::ExprOpcode,
    prog::SubProgram,
};

type WorkHandler = Arc<
    dyn Fn(&Arc<dyn ExprOpcode>, &Vec<Arc<SubProgram>>) -> Option<Arc<SubProgram>>
        + Send
        + Sync
>;

pub struct WorkGather
{
    chunk_size: usize,
    chunk: Box<Vec<Vec<Arc<SubProgram>>>>,
    handler: WorkHandler,
    tasks: tokio::task::JoinSet<Option<Arc<SubProgram>>>,
}

impl WorkGather
{
    pub fn new(handler: WorkHandler, chunk_size: usize) -> Self {
        Self {
            chunk_size: chunk_size,
            chunk: Vec::with_capacity(chunk_size).into(),
            handler: handler,
            tasks: tokio::task::JoinSet::new(),
        }
    }

    pub fn gather_work_for_next_iteration(
        &mut self,
        bank: &ProgBank,
        op: &Arc<dyn ExprOpcode>,
    ) {
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
        let mut iterations = Vec::with_capacity(op.arg_types().len());
        self.gather_work_with_cutoff_and_iterations(bank, op, cutoff, &mut iterations);
    }

    fn gather_work_with_cutoff_and_iterations(
        &mut self,
        bank: &ProgBank,
        op: &Arc<dyn ExprOpcode>,
        cutoff: usize,
        iterations: &mut Vec<usize>,
    ) {
        let i = iterations.len();
        let last_iteration = bank.bank.len() - 1;

        if i == op.arg_types().len() {
            self.gather_work_for_iterations(bank, op, iterations);
            return;
        }

        if i == cutoff {
            iterations.push(last_iteration);
            self.gather_work_with_cutoff_and_iterations(bank, op, cutoff, iterations);
            iterations.pop();
        } else if i < cutoff {
            for j in 0..last_iteration {
                iterations.push(j);
                self.gather_work_with_cutoff_and_iterations(bank, op, cutoff, iterations);
                iterations.pop();
            }
        } else {
            for j in 0..=last_iteration {
                iterations.push(j);
                self.gather_work_with_cutoff_and_iterations(bank, op, cutoff, iterations);
                iterations.pop();
            }
        }
    }

    fn gather_work_for_iterations(
        &mut self,
        bank: &ProgBank,
        op: &Arc<dyn ExprOpcode>,
        iterations: &[usize],
    ) {
        let arg_types = op.arg_types();
        let mut children = Vec::with_capacity(arg_types.len());
        for t in bank.bank[iterations[0]].0.iter() {
            let values = t.0.get(&arg_types[0]);
            if values.is_none() { continue; }
            for p in unsafe { values.unwrap_unchecked().iter() } {
                children.push(p.clone());
                self.gather_work_for_iterations_with_progs(bank, op, iterations, &mut children);
                children.pop();
            }
        }
    }

    fn gather_work_for_iterations_with_progs(
        &mut self,
        bank: &ProgBank,
        op: &Arc<dyn ExprOpcode>,
        iterations: &[usize],
        children: &mut Vec<Arc<SubProgram>>,
    ) {
        let i = children.len();
        if i == op.arg_types().len() {
            return self.gather_work(op, children);
        }

        let ctx = children.last().unwrap().post_ctx();
        if let Some(type_map) = bank.bank[iterations[i]].get(ctx) {
            let values = type_map.0.get(&op.arg_types()[i]);
            if values.is_none() { return; }
            for p in unsafe { values.unwrap_unchecked().iter() } {
                children.push(p.clone());
                self.gather_work_for_iterations_with_progs(bank, op, iterations, children);
                children.pop();
            }
        }
    }

    fn gather_work(&mut self, op: &Arc<dyn ExprOpcode>, children: &Vec<Arc<SubProgram>>) {
        self.chunk.push(children.clone());
        if self.chunk.len() == self.chunk_size {
            self.perform_work(op);
        }
    }

    fn perform_work(&mut self, op: &Arc<dyn ExprOpcode>) {
        let chunk = replace(&mut self.chunk, Vec::with_capacity(self.chunk_size).into());
        WorkGather::spawn(&mut self.tasks, chunk, op.clone(), self.handler.clone());
    }

    fn spawn(
        tasks: &mut tokio::task::JoinSet<Option<Arc<SubProgram>>>,
        chunk: Box<Vec<Vec<Arc<SubProgram>>>>,
        op: Arc<dyn ExprOpcode>,
        handler: WorkHandler,
    ) {
        tasks.spawn(async move {
            for c in chunk.iter() {
                let found_prog = handler(&op, c);
                if found_prog.is_some() {
                    return found_prog;
                }
            }
            return None;
        });
    }
}
