use dashmap::DashSet;
use futures::{future::BoxFuture, FutureExt};
use itertools::Itertools;
use std::{mem::replace, sync::Arc};
use tokio::task::JoinError;
use tokio_util::sync::CancellationToken;

use crate::{bank::ProgBank, opcode::ExprOpcode, prog::SubProgram};

type WorkHandler =
    Arc<dyn Fn(Arc<dyn ExprOpcode>, Vec<Arc<SubProgram>>) -> Option<Arc<SubProgram>> + Send + Sync>;

pub struct WorkGather {
    chunk_size: usize,
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
            handler: handler,
            tasks: tokio::task::JoinSet::new(),
            cancel_token: cancel_token,
        }
    }

    pub async fn gather_work_for_next_iteration(
        &mut self,
        bank: &ProgBank,
        op: &Arc<dyn ExprOpcode>,
    ) {
        self.add_all_tasks(bank, op).await;
    }

    pub async fn wait_for_all_tasks(&mut self) -> Option<Arc<SubProgram>> {
        let mut found_prog = None;
        while let Some(res) = self.wait_for_next().await {
            if let Ok(Some(p)) = res {
                found_prog = Some(p);
                self.tasks.abort_all();
            }
        }
        self.tasks.abort_all();
        return found_prog;
    }

    async fn wait_for_next(&mut self) -> Option<Result<Option<Arc<SubProgram>>, JoinError>> {
        tokio::select! {
            _ = self.cancel_token.cancelled() => None,
            v = self.tasks.join_next() => v
        }
    }

    async fn add_all_tasks(&mut self, bank: &ProgBank, op: &Arc<dyn ExprOpcode>) {
        for i in 0..op.arg_types().len() {
            self.gather_work_with_cutoff(bank, op, i).await;
        }
        if self.chunk.len() > 0 {
            self.perform_work(op);
        }
    }

    async fn gather_work_with_cutoff(
        &mut self,
        bank: &ProgBank,
        op: &Arc<dyn ExprOpcode>,
        cutoff: usize,
    ) {
        let last_iteration = bank.bank.len() - 1;
        let iterations_iterator = (0..op.arg_types().len())
            .map(|i| {
                if last_iteration == 0 {
                    0..=0
                } else if i == cutoff {
                    last_iteration..=last_iteration
                } else if i < cutoff {
                    0..=(last_iteration - 1)
                } else {
                    0..=last_iteration
                }
            })
            .multi_cartesian_product();

        for iterations in iterations_iterator {
            self.gather_work_for_iterations(bank, op, iterations).await
        }
    }

    async fn gather_work_for_iterations(
        &mut self,
        bank: &ProgBank,
        op: &Arc<dyn ExprOpcode>,
        iterations: Vec<usize>,
    ) {
        let mut programs = Vec::with_capacity(op.arg_types().len());

        let arg_types = op.arg_types();

        for i in 0..op.arg_types().len() {
            if let Some(values) = bank.bank[iterations[i]].0.get(&arg_types[i]) {
                programs.push(values.value().clone())
            } else {
                return;
            }
        }

        let mut children = Vec::with_capacity(programs.len());
        self.gather_work_for_maps(op, &programs, &mut children)
            .await;
    }

    fn gather_work_for_maps<'a>(
        &'a mut self,
        op: &'a Arc<dyn ExprOpcode>,
        maps: &'a [Arc<DashSet<Arc<SubProgram>>>],
        children: &'a mut Vec<Arc<SubProgram>>,
    ) -> BoxFuture<'a, ()> {
        let i = children.len();

        if i == maps.len() {
            return async move {
                self.gather_work(op, children.clone()).await;
            }
            .boxed();
        }

        async move {
            let map = &maps[i];
            for p in map.iter() {
                if let Some(prev) = children.last() {
                    if prev.post_ctx() != p.pre_ctx() {
                        continue;
                    }
                }
                children.push(p.clone());
                self.gather_work_for_maps(op, maps, children).await;
                children.pop();
            }
        }
        .boxed()
    }

    async fn gather_work(&mut self, op: &Arc<dyn ExprOpcode>, children: Vec<Arc<SubProgram>>) {
        self.chunk.push(children);
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
            tokio::select! {
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
