use futures::{future::BoxFuture, FutureExt};
use itertools::Itertools;
use std::{mem::replace, sync::Arc};
use tokio_util::sync::CancellationToken;

use crate::{
    bank::{ProgBank, ProgramsMap},
    context::{ContextArray, SynthesizerContext},
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

type ProgTriplet = (ContextArray, Vec<Arc<SubProgram>>, ContextArray);

pub struct WorkGather {
    chunk_size: usize,
    chunk: Vec<ProgTriplet>,
    handler: WorkHandler,
    tasks: tokio::task::JoinSet<Option<Arc<SubProgram>>>,
    cancel_token: CancellationToken,

    children: Vec<Arc<SubProgram>>,
    ctx: Vec<(ContextArray, ContextArray)>,
}

impl WorkGather {
    pub fn new(handler: WorkHandler, chunk_size: usize, cancel_token: CancellationToken) -> Self {
        Self {
            chunk_size,
            chunk: Vec::with_capacity(chunk_size),
            handler,
            tasks: tokio::task::JoinSet::new(),
            cancel_token,

            children: Default::default(),
            ctx: Default::default(),
        }
    }

    pub async fn gather_work_for_next_iteration(
        &mut self,
        bank: &ProgBank,
        op: &Arc<dyn ExprOpcode>,
        syn_ctx: &SynthesizerContext,
    ) {
        self.children = Vec::with_capacity(op.arg_types().len());
        self.ctx = Vec::with_capacity(op.arg_types().len());
        self.add_all_tasks(bank, op, syn_ctx).await;
    }

    pub async fn wait_for_all_tasks(&mut self) -> Option<Arc<SubProgram>> {
        let mut found_prog = None;
        while let Some(res) = self.tasks.join_next().await {
            if let Ok(Some(p)) = res {
                found_prog = Some(p);
                self.tasks.abort_all();
            }
        }
        found_prog
    }

    async fn add_all_tasks(
        &mut self,
        bank: &ProgBank,
        op: &Arc<dyn ExprOpcode>,
        syn_ctx: &SynthesizerContext,
    ) {
        if bank.iteration_count() == 1 {
            self.gather_work_for_iterations(bank, op, vec![0; op.arg_types().len()], syn_ctx)
                .await
        } else {
            for i in 0..op.arg_types().len() {
                self.gather_work_with_cutoff(bank, op, i, syn_ctx).await;
            }
        }
        if !self.chunk.is_empty() {
            self.perform_work(op);
        }
    }

    async fn gather_work_with_cutoff(
        &mut self,
        bank: &ProgBank,
        op: &Arc<dyn ExprOpcode>,
        cutoff: usize,
        syn_ctx: &SynthesizerContext,
    ) {
        if self.cancel_token.is_cancelled() {
            return;
        }
        let last_iteration = bank.iteration_count() - 1;
        let iterations_iterator = (0..op.arg_types().len())
            .map(|i| match i {
                n if n == cutoff => last_iteration..=last_iteration,
                n if n < cutoff => 0..=(last_iteration - 1),
                _ => 0..=last_iteration,
            })
            .multi_cartesian_product();

        for iterations in iterations_iterator {
            self.gather_work_for_iterations(bank, op, iterations, syn_ctx)
                .await
        }
    }

    async fn gather_work_for_iterations(
        &mut self,
        bank: &ProgBank,
        op: &Arc<dyn ExprOpcode>,
        iterations: Vec<usize>,
        syn_ctx: &SynthesizerContext,
    ) {
        if self.cancel_token.is_cancelled() {
            return;
        }
        let mut programs = Vec::with_capacity(op.arg_types().len());

        let arg_types = op.arg_types();

        for i in 0..op.arg_types().len() {
            if let Some(values) = bank[iterations[i]].get(&arg_types[i]) {
                programs.push(values.value().clone())
            } else {
                return;
            }
        }

        self.gather_work_for_maps(op, &programs, syn_ctx).await;
    }

    fn gather_work_for_maps<'a>(
        &'a mut self,
        op: &'a Arc<dyn ExprOpcode>,
        maps: &'a [Arc<ProgramsMap>],
        syn_ctx: &'a SynthesizerContext,
    ) -> BoxFuture<'a, ()> {
        let i = self.children.len();

        if i == maps.len() {
            return async move {
                let (pre_ctx, post_ctx) = self.ctx.last().unwrap().clone();
                self.gather_work(op, pre_ctx, post_ctx).await;
            }
            .boxed();
        }

        async move {
            let map = &maps[i];
            for p in map.iter() {
                if self.cancel_token.is_cancelled() {
                    return;
                }
                if let Some((pre_ctx, post_ctx)) = self.ctx.last() {
                    if !post_ctx.check_compatibility(p.pre_ctx(), syn_ctx) {
                        continue;
                    }
                    self.ctx
                        .push((pre_ctx.merge(p.pre_ctx()), p.post_ctx().merge(&post_ctx)));
                } else {
                    self.ctx.push((p.pre_ctx().clone(), p.post_ctx().clone()));
                }

                self.children.push(p.value().clone());
                self.gather_work_for_maps(op, maps, syn_ctx).await;
                self.children.pop();
                self.ctx.pop();
            }
        }
        .boxed()
    }

    async fn gather_work(
        &mut self,
        op: &Arc<dyn ExprOpcode>,
        pre_context: ContextArray,
        post_context: ContextArray,
    ) {
        if self.cancel_token.is_cancelled() {
            return;
        }

        self.chunk
            .push((pre_context, self.children.clone(), post_context));
        if self.chunk.len() == self.chunk_size {
            self.perform_work(op);
        }
    }

    fn perform_work(&mut self, op: &Arc<dyn ExprOpcode>) {
        let chunk = replace(&mut self.chunk, Vec::with_capacity(self.chunk_size));
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
        chunk: Vec<ProgTriplet>,
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
                    None
                } => v
            }
        });
    }
}
