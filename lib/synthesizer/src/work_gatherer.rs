use dashmap::DashMap;
use futures::{future::BoxFuture, FutureExt};
use itertools::Itertools;
use std::{collections::HashSet, mem::replace, sync::Arc};
use tokio_util::sync::CancellationToken;

use crate::{
    bank::{Output, ProgBank},
    context::{self, ContextArray},
    opcode::ExprOpcode,
    prog::SubProgram,
};

type WorkHandler = Arc<
    dyn Fn(
            Arc<dyn ExprOpcode>,
            ContextArray,
            Vec<Arc<SubProgram>>,
            ContextArray,
        ) -> Option<Arc<SubProgram>>
        + Send
        + Sync,
>;

pub struct WorkGather {
    chunk_size: usize,
    chunk: Box<Vec<(ContextArray, Vec<Arc<SubProgram>>, ContextArray)>>,
    handler: WorkHandler,
    tasks: tokio::task::JoinSet<Option<Arc<SubProgram>>>,
    cancel_token: CancellationToken,

    children: Vec<Arc<SubProgram>>,
}

impl WorkGather {
    pub fn new(handler: WorkHandler, chunk_size: usize, cancel_token: CancellationToken) -> Self {
        Self {
            chunk_size: chunk_size,
            chunk: Vec::with_capacity(chunk_size).into(),
            handler: handler,
            tasks: tokio::task::JoinSet::new(),
            cancel_token: cancel_token,

            children: Default::default(),
        }
    }

    pub async fn gather_work_for_next_iteration(
        &mut self,
        bank: &ProgBank,
        op: &Arc<dyn ExprOpcode>,
    ) {
        self.children = Vec::with_capacity(op.arg_types().len());
        self.add_all_tasks(bank, op).await;
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

    async fn add_all_tasks(&mut self, bank: &ProgBank, op: &Arc<dyn ExprOpcode>) {
        if bank.bank.len() == 1 {
            self.gather_work_for_iterations(bank, op, vec![0; op.arg_types().len()])
                .await
        } else {
            for i in 0..op.arg_types().len() {
                self.gather_work_with_cutoff(bank, op, i).await;
            }
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
                if i == cutoff {
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

        self.gather_work_for_maps(op, &programs).await;
    }

    fn gather_work_for_maps<'a>(
        &'a mut self,
        op: &'a Arc<dyn ExprOpcode>,
        maps: &'a [Arc<DashMap<Output, Arc<SubProgram>>>],
    ) -> BoxFuture<'a, ()> {
        let i = self.children.len();

        if i == maps.len() {
            return async move {
                let (pre_context, post_context) = self.get_children_context();
                self.gather_work(op, pre_context, post_context).await;
            }
            .boxed();
        }

        async move {
            let map = &maps[i];
            for p in map.iter() {
                if let Some(prev) = self.children.last() {
                    if prev.post_ctx().matches(p.value().pre_ctx()) == context::Matches::CONFLICT {
                        continue;
                    }
                }
                self.children.push(p.value().clone());
                self.gather_work_for_maps(op, maps).await;
                self.children.pop();
            }
        }
        .boxed()
    }

    fn get_children_context(&self) -> (ContextArray, ContextArray) {
        let children = &self.children;
        let mut pre_context = children.first().unwrap().pre_ctx().clone();
        let mut post_context = children.last().unwrap().post_ctx().clone();
        let mut variables = HashSet::new();

        for c in children {
            variables.extend(c.pre_ctx()[0].variables())
        }

        for c in children.iter().skip(1) {
            if pre_context[0].variable_count() == variables.len() {
                break;
            }
            pre_context.merge_in_place(c.pre_ctx())
        }

        for c in children.iter().rev().skip(1) {
            if post_context[0].variable_count() == variables.len() {
                break;
            }
            post_context.merge_in_place(c.post_ctx())
        }

        (pre_context, post_context)
    }

    async fn gather_work(
        &mut self,
        op: &Arc<dyn ExprOpcode>,
        pre_context: ContextArray,
        post_context: ContextArray,
    ) {
        self.chunk
            .push((pre_context, self.children.clone(), post_context));
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
        chunk: Box<Vec<(ContextArray, Vec<Arc<SubProgram>>, ContextArray)>>,
        op: Arc<dyn ExprOpcode>,
        handler: WorkHandler,
        cancel_token: CancellationToken,
    ) {
        tasks.spawn(async move {
            tokio::select! {
                _ = cancel_token.cancelled() => None,
                v = async {
                    for c in chunk.into_iter() {
                        let found_prog = handler(op.clone(), c.0, c.1, c.2);
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
