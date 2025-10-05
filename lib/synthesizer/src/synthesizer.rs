use crate::{
    bank::*,
    context::{ContextArray, ContextSubsetResult, VariableName},
    iterator::{
        bank_iterator, BankIterator,
        ProgramChildrenIterator,
        SeqTriple,
        seq_triple_iterator, SeqTripleIterator,
    },
    opcode::*,
    partial_context::PartialContextBuilder,
    prog::SubProgram,
    synthesizer_context::{
        SynthesizerContext, SynthesizerContextJsonDisplay, SynthesizerWorkerContext,
    },
    trace_context_array, trace_prog,
};
use dashmap::DashSet;
use futures::FutureExt;
use ruse_object_graph::ValueType;
use serde::ser::SerializeStruct;
use std::{
    fmt::Display,
    fs::OpenOptions,
    io::Write,
    marker::PhantomData,
    ops::Index,
    panic,
    path::{Path, PathBuf},
    sync::{atomic::*, Arc},
    time::Instant,
};
use tokio::{runtime::Handle, select, task::JoinSet};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info_span, trace, Instrument};

#[repr(usize)]
#[derive(Clone, Copy, Debug)]
pub enum StatisticsTypes {
    Evaluated,
    BankSize,
    FoundContextCount,
    MaxMutatingOpcodes,
    MaxDepth,
    MaxSize,
    __MaxType,
}

impl StatisticsTypes {
    pub fn iterator() -> impl Iterator<Item = StatisticsTypes> {
        [
            StatisticsTypes::Evaluated,
            StatisticsTypes::BankSize,
            StatisticsTypes::FoundContextCount,
            StatisticsTypes::MaxMutatingOpcodes,
            StatisticsTypes::MaxDepth,
            StatisticsTypes::MaxSize,
        ]
        .iter()
        .copied()
    }
    const fn count() -> usize {
        Self::__MaxType as usize
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            StatisticsTypes::Evaluated => "Evaluated",
            StatisticsTypes::BankSize => "BankSize",
            StatisticsTypes::FoundContextCount => "FoundContextCount",
            StatisticsTypes::MaxMutatingOpcodes => "MaxMutatingOpcodes",
            StatisticsTypes::MaxDepth => "MaxDepth",
            StatisticsTypes::MaxSize => "MaxSize",
            StatisticsTypes::__MaxType => unreachable!(),
        }
    }
}

#[derive(Default, Debug)]
struct Statistics {
    values: [AtomicU64; StatisticsTypes::count()],
}

#[derive(Debug, Clone)]
pub struct CurrentStatistics {
    values: Vec<u64>,
}

impl Default for CurrentStatistics {
    fn default() -> Self {
        Self {
            values: vec![0; StatisticsTypes::count()],
        }
    }
}

impl Statistics {
    #[inline]
    pub fn get_value(&self, stype: StatisticsTypes) -> u64 {
        self.values[stype as usize].load(Ordering::Relaxed)
    }

    #[inline]
    pub fn inc_value(&self, stype: StatisticsTypes) {
        self.values[stype as usize].fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn max_value(&self, stype: StatisticsTypes, new_val: u64) {
        self.values[stype as usize].fetch_max(new_val, Ordering::Relaxed);
    }

    #[inline]
    pub fn current(&self) -> CurrentStatistics {
        CurrentStatistics {
            values: StatisticsTypes::iterator()
                .map(|x| self.get_value(x))
                .collect(),
        }
    }
}

impl CurrentStatistics {
    pub fn get_diff(&self, rhs: &Self) -> Self {
        let mut values = self.values.clone();
        values[StatisticsTypes::Evaluated as usize] -= rhs[StatisticsTypes::Evaluated];
        values[StatisticsTypes::BankSize as usize] -= rhs[StatisticsTypes::BankSize];
        values[StatisticsTypes::FoundContextCount as usize] -=
            rhs[StatisticsTypes::FoundContextCount];
        Self { values }
    }
}

impl Display for CurrentStatistics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut comma_separated = String::new();
        for stype in StatisticsTypes::iterator() {
            comma_separated
                .push_str(format!("{:?}: {}", stype, self.values[stype as usize]).as_str());
            comma_separated.push_str(", ");
        }
        comma_separated.pop();
        comma_separated.pop();
        write!(f, "{}", comma_separated)
    }
}

impl Index<StatisticsTypes> for CurrentStatistics {
    type Output = u64;

    fn index(&self, index: StatisticsTypes) -> &Self::Output {
        &self.values[index as usize]
    }
}

impl serde::Serialize for CurrentStatistics {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut stats = serializer.serialize_struct("Statistics", StatisticsTypes::count())?;
        for (t, v) in StatisticsTypes::iterator().zip(self.values.iter()) {
            stats.serialize_field(t.as_str(), v)?;
        }

        stats.end()
    }
}

pub type SynthesizerPredicate = Box<
    dyn Fn(&SubProgram, &SynthesizerContext, &mut SynthesizerWorkerContext) -> bool + Send + Sync,
>;

pub trait WorkerContextCreator: Send + Sync {
    fn create_worker_ctx(index: usize) -> SynthesizerWorkerContext;
}

#[derive(Debug, Clone)]
pub struct SynthesizerOptions {
    pub worker_count: usize,
    pub max_mutations: u32,
    pub output_embedding_overhead: Option<PathBuf>,
}

struct SynthesizerInner<P: ProgBank, W: WorkerContextCreator + 'static> {
    task: String,

    bank: P,
    opcodes: OpcodesMap,
    context: SynthesizerContext,
    found_contexts: DashSet<ContextArray>,
    cancel_token: CancellationToken,

    predicate: SynthesizerPredicate,
    valid: SynthesizerPredicate,

    stop_workers_token: CancellationToken,

    statistics: Statistics,

    options: SynthesizerOptions,

    _worker_context_creator_phantom: PhantomData<W>,
}

impl<P: ProgBank + 'static, W: WorkerContextCreator + 'static> SynthesizerInner<P, W> {
    pub fn new(
        task: String,
        bank: P,
        syn_ctx: SynthesizerContext,
        opcodes: OpcodesList,
        predicate: SynthesizerPredicate,
        valid: SynthesizerPredicate,
        options: SynthesizerOptions,
    ) -> Self {
        Self {
            task,
            bank,
            opcodes: sort_opcodes(opcodes),
            context: syn_ctx,
            found_contexts: DashSet::new(),
            cancel_token: CancellationToken::new(),
            predicate,
            valid,
            stop_workers_token: CancellationToken::new(),
            statistics: Default::default(),
            options,
            _worker_context_creator_phantom: PhantomData,
        }
    }

    pub fn get_cancel_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }

    async fn init_context<const IS_START_CTX: bool>(
        &self,
        iteration_map: &mut <P::IterationBuilderType as BankIterationBuilder>::BatchBuilderType,
        ctx: &ContextArray,
        worker_ctx: &mut SynthesizerWorkerContext,
    ) -> Option<Arc<SubProgram>> {
        trace_context_array!(target: "ruse::synthesizer", ctx, "Initializing context");

        let mut res = None;

        self.found_contexts.insert(ctx.clone());
        self.statistics
            .inc_value(StatisticsTypes::FoundContextCount);
        let partial_context_builder = PartialContextBuilder::new(ctx);

        for op in self.opcodes.init_opcodes() {
            let p = match self.get_program_from_init_opcode(
                op.clone(),
                &partial_context_builder,
                worker_ctx,
            ) {
                Some(p) => p,
                None => continue,
            };
            if self.found_contexts.insert(p.pre_ctx().clone()) {
                self.statistics
                    .inc_value(StatisticsTypes::FoundContextCount);
            }

            if !self.check_program(&p, worker_ctx).await {
                continue;
            }

            if !self.insert_program(p.clone(), iteration_map).await {
                continue;
            }

            if IS_START_CTX && (self.predicate)(&p, &self.context, worker_ctx) {
                res = Some(p);
                break;
            }
        }

        trace!(target: "ruse::synthesizer", "Finished initializing context");
        res
    }

    pub async fn run_iteration(self: &mut Arc<Self>) -> anyhow::Result<Option<Arc<SubProgram>>> {
        let _span = info_span!(target: "ruse::synthesizer", "run_iteration", iteration = self.bank.iteration_count()).entered();
        let (current_iteration_map, found_prog) = self.clone().run_iteration_inner().await?;

        self.insert_iteration(current_iteration_map).await;

        Ok(found_prog)
    }

    async fn run_iteration_inner(
        self: Arc<Self>,
    ) -> anyhow::Result<(P::IterationBuilderType, Option<Arc<SubProgram>>)> {
        let iteration = self.bank.iteration_count();

        debug!(target: "ruse::synthesizer", "Starting iteration {}", iteration);

        let prev_bank_size = self.statistics.get_value(StatisticsTypes::BankSize);
        let prev_evaluated = self.statistics.get_value(StatisticsTypes::Evaluated);

        let started = Instant::now();

        let self_clone = self.clone();
        let res = tokio::spawn(
            panic::AssertUnwindSafe(async move {
                let mut current_iteration_map = self_clone.bank.create_iteration_builder();
                let found = if self_clone.bank.iteration_count() == 0 {
                    tokio::task::block_in_place(|| {
                        Handle::current()
                            .block_on(self_clone.run_init_iteration(&mut current_iteration_map))
                            .in_current_span()
                    })
                    .into_inner()
                } else {
                    self_clone
                        .run_composite_iteration(&mut current_iteration_map)
                        .await
                };
                Ok((current_iteration_map, found?))
            })
            .catch_unwind()
            .in_current_span(),
        )
        .await;

        let took = started.elapsed();

        let new_bank_size = self.statistics.get_value(StatisticsTypes::BankSize);
        let new_evaluated = self.statistics.get_value(StatisticsTypes::Evaluated);

        debug!(target: "ruse::synthesizer", "Finished iteration {} in {:.2?}, evaluated {}, found {} new programs",
            iteration,
            took,
            new_evaluated - prev_evaluated,
            new_bank_size - prev_bank_size);
        res?.map_err(|_e| anyhow::anyhow!("Panic in run_iteration_inner"))?
    }

    fn create_worker_ctx(&self, index: usize) -> SynthesizerWorkerContext {
        W::create_worker_ctx(index)
    }

    async fn run_init_iteration(
        &self,
        current_iteration_map: &mut P::IterationBuilderType,
    ) -> anyhow::Result<Option<Arc<SubProgram>>> {
        let mut worker_ctx = self.create_worker_ctx(0);
        let mut batch_builder = current_iteration_map.create_batch_builder();
        let found = self
            .init_context::<true>(
                &mut batch_builder,
                &self.context.start_context,
                &mut worker_ctx,
            )
            .await;
        current_iteration_map.add_batch(batch_builder).await;
        Ok(found)
    }

    async fn composite_iter_batch(
        &self,
        triple: &SeqTriple,
        ops: &OpcodesList,
        worker_ctx: &mut SynthesizerWorkerContext,
        current_batch_map: &mut <P::IterationBuilderType as BankIterationBuilder>::BatchBuilderType,
    ) -> Option<Arc<SubProgram>> {
        for op in ops {
            let Some(p) = self.get_program_from_composite_opcode(op.clone(), triple, worker_ctx)
            else {
                continue;
            };

            if !self.check_program(&p, worker_ctx).await {
                continue;
            }

            if !self.insert_program(p.clone(), current_batch_map).await {
                continue;
            }

            if p.pre_ctx().subset(&self.context.start_context) != ContextSubsetResult::NotSubset
                && (self.predicate)(&p, &self.context, worker_ctx)
            {
                trace_prog!(tracing::Level::DEBUG, target: "ruse::synthesizer", &p, "Found");
                return Some(p);
            }
        }

        None
    }

    async fn should_end_worker(&self) -> bool {
        self.cancel_token.is_cancelled() || self.stop_workers_token.is_cancelled()
    }

    async fn worker_children_iterator<'a>(
        &'a self,
        i: usize,
        arg_types: &'a [ValueType],
    ) -> BankIterator<'a, P> {
        let mut children_iterator = bank_iterator(&self.bank, arg_types).await;
        let total_size = children_iterator.remaining();
        let skip = (total_size / self.options.worker_count) * i;
        let take = if i == self.options.worker_count - 1 {
            usize::MAX
        } else {
            total_size / self.options.worker_count
        };
        children_iterator.skip(skip).await;
        children_iterator.take(take);

        children_iterator
    }

    async fn worker_triple_iterator<'a>(
        &'a self,
        i: usize,
        arg_types: &'a [ValueType],
    ) -> SeqTripleIterator<BankIterator<'a, P>> {
        let children_iterator = self.worker_children_iterator(i, arg_types).await;
        seq_triple_iterator(children_iterator)
    }

    async fn composite_iteration_worker(
        self: Arc<Self>,
        batch_builder: &mut <P::IterationBuilderType as BankIterationBuilder>::BatchBuilderType,
        i: usize,
    ) -> Option<Arc<SubProgram>> {
        let worker_span =
            info_span!(target: "ruse::synthesizer", "composite iteration worker", worker_index = i);
        let _worker_span_guard = worker_span.enter();
        let mut worker_ctx = self.create_worker_ctx(i);
        for (arg_types, ops) in self.opcodes.composite_opcodes() {
            let span = info_span!(target: "ruse::synthesizer", "composite opcodes", arg_types = ?arg_types, ops = ?ops.iter().map(|op| op.op_name()).collect::<Vec<_>>());
            let _span_guard = span.enter();
            let mut iter = self.worker_triple_iterator(i, arg_types).await;
            while let Some(triple) = iter.next().await {
                if self.should_end_worker().await {
                    return None;
                }
                let found = self
                    .composite_iter_batch(&triple, &ops, &mut worker_ctx, batch_builder)
                    .await;
                if found.is_some() {
                    self.stop_workers_token.cancel();
                    return found;
                }
                worker_ctx.data.gc();
            }
        }

        None
    }

    async fn init_new_found_ctx(
        self: Arc<Self>,
        current_iteration_map: &mut P::IterationBuilderType,
    ) {
        let mut worker_ctx = self.create_worker_ctx(0);
        let mut new_ctx = current_iteration_map.create_batch_builder();
        for p in current_iteration_map.iter_programs().await {
            if self.should_end_worker().await {
                return;
            }
            if self.found_contexts.insert(p.post_ctx().clone()) {
                trace!(target: "ruse::synthesizer", "New post context found by program \"{}\"", p.get_code());
                self.init_context::<false>(&mut new_ctx, p.post_ctx(), &mut worker_ctx)
                    .await;
            }
        }
        current_iteration_map.add_batch(new_ctx).await;
    }

    async fn stop_workers_and_wait<T: 'static>(&self, workers: &mut JoinSet<T>) {
        self.stop_workers_token.cancel();
        workers.shutdown().await;
    }

    async fn run_composite_iteration(
        self: Arc<Self>,
        current_iteration_map: &mut P::IterationBuilderType,
    ) -> anyhow::Result<Option<Arc<SubProgram>>> {
        if let Some(output_embedding_csv) = &self.options.output_embedding_overhead {
            let self_clone = self.clone();
            let _ = tokio::task::block_in_place(|| {
                Handle::current()
                    .block_on(self_clone.output_embedding_overhead_statistics(output_embedding_csv))
                    .in_current_span()
            });
        }
        let mut workers = JoinSet::new();
        for i in 0..self.options.worker_count {
            let mut batch_builder = current_iteration_map.create_batch_builder();
            let self_clone = self.clone();

            let current_span = tracing::Span::current();
            workers.spawn_blocking(move || {
                Handle::current().block_on(
                    panic::AssertUnwindSafe(async move {
                        let _span = current_span.enter();
                        let found = self_clone
                            .composite_iteration_worker(&mut batch_builder, i)
                            .await;
                        (batch_builder, found)
                    })
                    .catch_unwind(),
                )
            });
        }

        while let Some(worker_res) = select! {
            res = workers.join_next() => res,
            _ = self.cancel_token.cancelled() => None,
        } {
            match worker_res {
                Ok(Ok((worker_type_map, found))) => {
                    if found.is_some() {
                        self.stop_workers_and_wait(&mut workers).await;
                        return Ok(found);
                    }
                    if self.cancel_token.is_cancelled() {
                        return Ok(None);
                    }
                    let _ = tokio::task::block_in_place(|| {
                        Handle::current()
                            .block_on(current_iteration_map.add_batch(worker_type_map))
                            .in_current_span()
                    });
                }
                Ok(Err(_e)) => {
                    self.stop_workers_and_wait(&mut workers).await;
                    return Err(anyhow::anyhow!("Panic in composite_iteration_worker"));
                }
                Err(_e) => {
                    self.stop_workers_and_wait(&mut workers).await;
                    return Err(anyhow::anyhow!("Failed to join worker"));
                }
            }
        }

        if self.cancel_token.is_cancelled() {
            return Err(anyhow::anyhow!("Synthesizer cancelled"));
        }

        debug!(target: "ruse::synthesizer", "Initializing new contexts!");

        let _ = tokio::task::block_in_place(|| {
            Handle::current()
                .block_on(self.init_new_found_ctx(current_iteration_map))
                .in_current_span()
        });
        Ok(None)
    }

    async fn insert_iteration(
        self: &mut Arc<Self>,
        current_iteration_map: P::IterationBuilderType,
    ) {
        Arc::get_mut(self)
            .unwrap()
            .bank
            .end_iteration(current_iteration_map)
            .await;
    }

    fn evaluate_program(
        &self,
        p: &mut Arc<SubProgram>,
        worker_ctx: &mut SynthesizerWorkerContext,
    ) -> bool {
        // trace!(target: "ruse::synthesizer", "Evaluating program {}", p.get_code());
        self.statistics.inc_value(StatisticsTypes::Evaluated);
        unsafe { Arc::get_mut(p).unwrap_unchecked() }.evaluate(&self.context, worker_ctx)
    }

    fn get_program_from_composite_opcode(
        &self,
        op: Arc<dyn ExprOpcode>,
        triple: &SeqTriple,
        worker_ctx: &mut SynthesizerWorkerContext,
    ) -> Option<Arc<SubProgram>> {
        debug_assert!(!op.arg_types().is_empty());

        let triple_clone = triple.clone();
        let mut p = SubProgram::with_opcode_and_children(
            op,
            triple_clone.children,
            triple_clone.pre_ctx,
            triple_clone.post_ctx,
        );
        if p.num_mutations() > self.options.max_mutations {
            return None;
        }
        self.evaluate_program(&mut p, worker_ctx).then(|| p)
    }

    fn get_program_from_init_opcode(
        &self,
        op: Arc<dyn ExprOpcode>,
        partial_context_builder: &PartialContextBuilder,
        worker_ctx: &mut SynthesizerWorkerContext,
    ) -> Option<Arc<SubProgram>> {
        debug_assert!(op.arg_types().is_empty());

        let pre_ctx = partial_context_builder.get_partial_context(op.required_variables())?;
        let post_ctx = pre_ctx.clone();
        let mut p = SubProgram::with_opcode(op.clone(), pre_ctx, post_ctx);
        match self.evaluate_program(&mut p, worker_ctx) {
            true => Some(p),
            false => None,
        }
    }

    async fn check_program(
        &self,
        p: &Arc<SubProgram>,
        worker_ctx: &mut SynthesizerWorkerContext,
    ) -> bool {
        if p.num_mutations() > self.options.max_mutations {
            return false;
        }

        // Depth is an underestimate of the number of mutations from the initial context
        if p.post_ctx().depth() > self.options.max_mutations {
            return false;
        }

        if !(self.valid)(p, &self.context, worker_ctx) {
            return false;
        }

        if !p.is_terminal() && self.bank.output_exists(p).await {
            return false;
        }

        true
    }

    async fn insert_program(
        &self,
        p: Arc<SubProgram>,
        batch_builder: &mut <P::IterationBuilderType as BankIterationBuilder>::BatchBuilderType,
    ) -> bool {
        if p.is_terminal() {
            trace_prog!(target: "ruse::synthesizer", p, "Terminal program.");
            return true;
        }

        if batch_builder.add_program(&p).await {
            trace_prog!(target: "ruse::synthesizer", p, "Inserted");

            self.statistics.inc_value(StatisticsTypes::BankSize);
            self.statistics
                .max_value(StatisticsTypes::MaxDepth, p.depth().into());
            self.statistics
                .max_value(StatisticsTypes::MaxSize, p.size().into());
            self.statistics.max_value(
                StatisticsTypes::MaxMutatingOpcodes,
                p.num_mutations() as u64,
            );

            return true;
        }
        false
    }

    #[inline]
    pub fn statistics(&self) -> CurrentStatistics {
        self.statistics.current()
    }

    pub fn set_immutable(&mut self, var: &VariableName) {
        self.context.set_immutable(var);
    }

    // pub fn print_all_programs(&self) {
    //     self.bank.print_all_programs()
    // }
}

impl<P: ProgBank + 'static, W: WorkerContextCreator + 'static> SynthesizerInner<P, W> {
    async fn count_embedded_programs_worker(&self, worker_index: usize) -> usize {
        let mut embedded_programs_count = 0;
        for (arg_types, _) in self.opcodes.composite_opcodes() {
            let mut iter = self.worker_triple_iterator(worker_index, arg_types).await;
            while let Some(_) = iter.next().await {
                embedded_programs_count += 1;
            }
        }
        embedded_programs_count
    }

    async fn count_embedded_programs(self: &Arc<Self>) -> usize {
        let mut workers = JoinSet::new();
        for i in 0..self.options.worker_count {
            let self_clone: Arc<Self> = self.clone();

            let current_span = tracing::Span::current();
            workers.spawn_blocking(move || {
                Handle::current().block_on(async move {
                    let _span = current_span.enter();
                    self_clone.count_embedded_programs_worker(i).await
                })
            });
        }

        workers.join_all().await.into_iter().sum()
    }

    async fn count_programs_without_embedding_worker(&self, worker_index: usize) -> usize {
        let mut programs_count = 0;
        for (arg_types, _) in self.opcodes.composite_opcodes() {
            let mut iter = self.worker_children_iterator(worker_index, arg_types).await;
            while let Some(_) = iter.next().await {
                programs_count += 1;
            }
        }
        programs_count
    }

    async fn count_programs_without_embedding(self: &Arc<Self>) -> usize {
        let mut workers = JoinSet::new();
        for i in 0..self.options.worker_count {
            let self_clone: Arc<Self> = self.clone();

            let current_span = tracing::Span::current();
            workers.spawn_blocking(move || {
                Handle::current().block_on(async move {
                    let _span = current_span.enter();
                    self_clone.count_programs_without_embedding_worker(i).await
                })
            });
        }

        workers.join_all().await.into_iter().sum()
    }

    async fn output_embedding_overhead_statistics(self: Arc<Self>, csv_path: &Path) {
        let iteration = self.bank.iteration_count();
        let bank_size = self.statistics.get_value(StatisticsTypes::BankSize);

        let span =
            tracing::info_span!(target: "ruse::synthesizer", "embedding overhead calculation");
        let _span_guard = span.enter();

        tracing::info!(target: "ruse::synthesizer", "Starting calculating embedding overhead for iteration {}", iteration);

        let started_without_embedding = Instant::now();
        let total_programs_count = self.count_programs_without_embedding().await;
        let total_took_without_embedding = started_without_embedding.elapsed();

        let started_with_embedding = Instant::now();
        let embedded_programs_count = self.count_embedded_programs().await;
        let total_took_with_embedding = started_with_embedding.elapsed();

        let mut csv_file = OpenOptions::new()
            .append(true)
            .open(csv_path)
            .expect("Failed to open CSV file");
        csv_file
            .write_all(
                format!(
                    "{},{},{},{},{},{:.2},{:.2}\n",
                    self.task,
                    iteration,
                    bank_size,
                    total_programs_count,
                    embedded_programs_count,
                    total_took_without_embedding.as_secs_f64(),
                    total_took_with_embedding.as_secs_f64(),
                )
                .as_bytes(),
            )
            .expect("Failed to write to CSV file");

        tracing::info!(target: "ruse::synthesizer", "Finished calculating embedding overhead iteration {}", iteration);
    }
}

#[derive(serde::Serialize)]
pub struct SynthesizerJsonDisplay {
    opcodes: serde_json::Value,
    context: SynthesizerContextJsonDisplay,
    max_mutations: u32,
    worker_count: usize,
}

impl Display for SynthesizerJsonDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = serde_json::to_string_pretty(self)
            .unwrap_or_else(|e| format!("Failed to serialize synthesizer: {}", e));
        write!(f, "{}", value)
    }
}

impl<P: ProgBank + 'static, W: WorkerContextCreator + 'static> SynthesizerInner<P, W> {
    pub fn json_display(&self) -> impl Display {
        self.json_display_struct()
    }

    pub(crate) fn json_display_struct(&self) -> SynthesizerJsonDisplay {
        SynthesizerJsonDisplay {
            opcodes: self.opcodes.to_json(),
            context: self.context.json_display_struct(),
            max_mutations: self.options.max_mutations,
            worker_count: self.options.worker_count,
        }
    }
}

pub struct Synthesizer<P: ProgBank + 'static, W: WorkerContextCreator + 'static>(
    Arc<SynthesizerInner<P, W>>,
);

impl<P: ProgBank + 'static, W: WorkerContextCreator + 'static> Synthesizer<P, W> {
    pub fn new(
        task: String,
        bank: P,
        syn_ctx: SynthesizerContext,
        opcodes: OpcodesList,
        predicate: SynthesizerPredicate,
        valid: SynthesizerPredicate,
        options: SynthesizerOptions,
    ) -> Synthesizer<P, W> {
        let inner = SynthesizerInner::new(task, bank, syn_ctx, opcodes, predicate, valid, options);

        Self(inner.into())
    }

    #[inline]
    pub async fn run_iteration(&mut self) -> anyhow::Result<Option<Arc<SubProgram>>> {
        SynthesizerInner::run_iteration(&mut self.0).await
    }

    #[inline]
    pub fn statistics(&self) -> CurrentStatistics {
        self.0.statistics()
    }

    pub fn get_cancel_token(&self) -> CancellationToken {
        self.0.get_cancel_token()
    }

    pub fn set_immutable(&mut self, var: &VariableName) {
        Arc::get_mut(&mut self.0).unwrap().set_immutable(var);
    }

    pub fn json_display(&self) -> impl Display + '_ {
        self.0.json_display()
    }

    pub fn synthesizer_context(&self) -> &SynthesizerContext {
        &self.0.context
    }
}
