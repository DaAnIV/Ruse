use crate::{
    bank::*,
    bank_iterator::{bank_iterator, BankIterator},
    context::{ContextArray, SynthesizerContext, SynthesizerContextJsonDisplay, VariableName},
    multi_programs_map_product::ProgramChildrenIterator,
    opcode::*,
    prog::SubProgram,
    prog_triplet::ProgTriplet,
    prog_triplet_iterator::{prog_triplet_iterator, ProgTripletIterator}, trace_context_array, trace_prog
};
use dashmap::DashSet;
use futures::FutureExt;
use ruse_object_graph::{
    mermaid::MermaidConfig,
    value::Value,
    ValueType,
};
use serde::ser::SerializeStruct;
use std::{
    fmt::Display, ops::Index, panic, sync::{atomic::*, Arc}, time::Instant
};
use tokio::{runtime::Handle, task::JoinSet};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info_span, trace, Instrument};

const ALLOW_NON_FINITE_NUMBER: bool = false;

#[repr(usize)]
#[derive(Clone, Copy, Debug)]
pub enum StatisticsTypes {
    Evaluated,
    BankSize,
    FoundContextCount,
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

pub type SynthesizerPredicate = Box<dyn Fn(&SubProgram, &SynthesizerContext) -> bool + Send + Sync>;

pub struct Synthesizer<P: ProgBank> {
    bank: P,
    opcodes: OpcodesMap,
    context: SynthesizerContext,
    found_contexts: DashSet<ContextArray>,
    max_context_depth: usize,
    cancel_token: CancellationToken,

    predicate: SynthesizerPredicate,
    valid: SynthesizerPredicate,

    worker_count: usize,
    found_token: CancellationToken,

    statistics: Statistics,
}

impl<P: ProgBank + 'static> Synthesizer<P> {
    pub fn new(
        bank: P,
        syn_ctx: SynthesizerContext,
        opcodes: OpcodesList,
        predicate: SynthesizerPredicate,
        valid: SynthesizerPredicate,
        max_context_depth: usize,
        iteration_workers_count: usize,
    ) -> Self {
        Self {
            bank,
            opcodes: sort_opcodes(opcodes),
            context: syn_ctx,
            found_contexts: DashSet::new(),
            max_context_depth,
            cancel_token: CancellationToken::new(),
            predicate,
            valid,
            worker_count: iteration_workers_count,
            found_token: CancellationToken::new(),
            statistics: Default::default(),
        }
    }

    fn init_opcodes(&self) -> impl Iterator<Item = &Arc<dyn ExprOpcode>> {
        self.opcodes[&vec![]].iter()
    }

    fn composite_opcodes(&self) -> impl Iterator<Item = (&Vec<ValueType>, &Arc<OpcodesList>)> {
        self.opcodes
            .iter()
            .filter(|(arg_types, _)| !arg_types.is_empty())
    }

    pub fn get_cancel_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }

    async fn init_context<const IS_START_CTX: bool>(
        &self,
        iteration_map: &mut <P::IterationBuilderType as BankIterationBuilder>::BatchBuilderType,
        ctx: &ContextArray,
    ) -> Option<Arc<SubProgram>> {
        trace_context_array!(target: "ruse::synthesizer", ctx, "Initializing context");

        let mut res = None;

        self.found_contexts.insert(ctx.clone());
        self.statistics
            .inc_value(StatisticsTypes::FoundContextCount);
        for op in self.init_opcodes() {
            let p = match self.get_program_from_init_opcode(op.clone(), ctx) {
                Some(p) => p,
                None => continue,
            };
            if self.found_contexts.insert(p.pre_ctx().clone()) {
                self.statistics
                    .inc_value(StatisticsTypes::FoundContextCount);
            }

            if !self.check_program(&p).await {
                continue;
            }

            if !self.insert_program(p.clone(), iteration_map).await {
                continue;
            }

            if IS_START_CTX && (self.predicate)(&p, &self.context) {
                res = Some(p);
                break;
            }
        }

        trace!(target: "ruse::synthesizer", "Finished initializing context");
        res
    }

    pub async fn run_iteration(self: &mut Arc<Self>) -> Option<Arc<SubProgram>> {
        let _span = info_span!(target: "ruse::synthesizer", "run_iteration", iteration = self.bank.iteration_count()).entered();
        let (current_iteration_map, found_prog) = Self::run_iteration_inner(self.clone()).await;

        Self::insert_iteration(self, current_iteration_map).await;

        found_prog
    }

    async fn run_iteration_inner(self: Arc<Self>) -> (P::IterationBuilderType, Option<Arc<SubProgram>>) {
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
                    tokio::task::block_in_place(|| Handle::current().block_on(self_clone.run_init_iteration(&mut current_iteration_map)).in_current_span()).into_inner()
                } else {
                    self_clone.run_composite_iteration(&mut current_iteration_map)
                        .await
                };
                (current_iteration_map, found)
            })
            .catch_unwind().in_current_span(),
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
        res.unwrap().unwrap()
    }

    async fn run_init_iteration(
        &self,
        current_iteration_map: &mut P::IterationBuilderType,
    ) -> Option<Arc<SubProgram>> {
        let mut batch_builder = current_iteration_map.create_batch_builder();
        let found = self.init_context::<true>(&mut batch_builder, &self.context.start_context).await;
        current_iteration_map.add_batch(batch_builder).await;
        found
    }

    fn trace_program(p: &SubProgram) {
        debug!(target: "ruse::synthesizer", "Program \"{}\"", p.get_code());

        for i in 0..p.post_ctx().len() {
            let pre_ctx = &p.pre_ctx()[i];
            let post_ctx = &p.post_ctx()[i];
            let out_value = &p.out_value()[i];
            for (name, value) in pre_ctx.variables() {
                debug!(target: "ruse::synthesizer", "pre_ctx[{}] {}:\n{}", i,
                    name, 
                    value.mermaid_display_with_config(&pre_ctx.graphs_map, 
                    MermaidConfig::subgraph_config(&format!("{}[{}]_{}", "pre", i, name))));
            }
            for (name, value) in post_ctx.variables() {
                debug!(target: "ruse::synthesizer", "post_ctx[{}] {}:\n{}", i,
                    name, 
                    value.mermaid_display_with_config(&post_ctx.graphs_map, 
                    MermaidConfig::subgraph_config(&format!("{}[{}]_{}", "post", i, name))))
            }
            debug!(target: "ruse::synthesizer", "output[{}]: {}", i ,
                out_value.val().mermaid_display_with_config(&post_ctx.graphs_map, 
                MermaidConfig::subgraph_config(&format!("output[{}]", i))));
        }
    }

    async fn composite_iter_batch(
        &self,
        triplet: &ProgTriplet,
        ops: &OpcodesList,
        current_batch_map: &mut <P::IterationBuilderType as BankIterationBuilder>::BatchBuilderType,
    ) -> Option<Arc<SubProgram>> {
        for op in ops {
            let Some(p) = self.get_program_from_composite_opcode(op.clone(), triplet) else {
                continue;
            };

            if !self.check_program(&p).await {
                continue;
            }

            if !self.insert_program(p.clone(), current_batch_map).await {
                continue;
            }

            if p.pre_ctx().subset(&self.context.start_context)
                && (self.predicate)(&p, &self.context)
            {
                debug!(target: "ruse::synthesizer", "Found!");
                Self::trace_program(&p);
                return Some(p);
            }
        }

        None
    }

    async fn should_end_worker(&self) -> bool {
        self.cancel_token.is_cancelled() || self.found_token.is_cancelled()
    }

    async fn worker_triple_iterator<'a>(
        &'a self,
        i: usize,
        arg_types: &'a Vec<ValueType>,
    ) -> ProgTripletIterator<BankIterator<'a, P>> {
        let mut children_iterator = bank_iterator(&self.bank, arg_types).await;
        let total_size = children_iterator.remaining();
        let skip = (total_size / self.worker_count) * i;
        let take = if i == self.worker_count - 1 {
            usize::MAX
        } else {
            total_size / self.worker_count
        };
        children_iterator.skip(skip).await;
        children_iterator.take(take);

        prog_triplet_iterator(children_iterator)
    }

    async fn composite_iteration_worker(
        self: Arc<Self>,
        batch_builder: &mut <P::IterationBuilderType as BankIterationBuilder>::BatchBuilderType,
        i: usize,
    ) -> Option<Arc<SubProgram>> {
        for (arg_types, ops) in self.composite_opcodes() {
            let mut iter = self.worker_triple_iterator(i, arg_types).await;
            while let Some(triple) = iter.next().await {
                if self.should_end_worker().await {
                    return None;
                }
                let found = self.composite_iter_batch(&triple, &ops, batch_builder).await;
                if found.is_some() {
                    self.found_token.cancel();
                    return found;
                }
            }
        }

        None
    }

    async fn init_new_found_ctx(self: Arc<Self>, current_iteration_map: &mut P::IterationBuilderType) {
        let mut new_ctx = current_iteration_map.create_batch_builder();
        for p in current_iteration_map.iter_programs().await {
            if self.cancel_token.is_cancelled() {
                return;
            }
            if self.found_contexts.insert(p.post_ctx().clone()) {
                trace!(target: "ruse::synthesizer", "New post context found by program \"{}\"", p.get_code());
                self.init_context::<false>(&mut new_ctx, p.post_ctx()).await;
            }
        }
        current_iteration_map.add_batch(new_ctx).await;
    }

    async fn run_composite_iteration(
        self: Arc<Self>,
        current_iteration_map: &mut P::IterationBuilderType,
    ) -> Option<Arc<SubProgram>> {
        let mut workers = JoinSet::new();
        for i in 0..self.worker_count {
            let mut batch_builder = current_iteration_map.create_batch_builder();
            let self_clone = self.clone();
            workers.spawn(
                panic::AssertUnwindSafe(async move {
                    let found = Self::composite_iteration_worker(self_clone, &mut batch_builder, i).await;
                    (batch_builder, found)
                })
                    .catch_unwind().in_current_span(),
            );
        }

        while let Some(worker_res) = workers.join_next().await {
            let (worker_type_map, found) = worker_res.unwrap().unwrap();
            if self.cancel_token.is_cancelled() {
                return None;
            }
            if found.is_some() {
                workers.abort_all();
                while workers.join_next().await.is_some() {}
                return found;
            }
            tokio::task::block_in_place(|| Handle::current().block_on(current_iteration_map.add_batch(worker_type_map)));
        }

        debug!(target: "ruse::synthesizer", "Initializing new contexts!");

        tokio::task::block_in_place(|| Handle::current().block_on(self.init_new_found_ctx(current_iteration_map)));
        None
    }

    async fn insert_iteration(self: &mut Arc<Self>, current_iteration_map: P::IterationBuilderType) {
        Arc::get_mut(self)
            .unwrap()
            .bank
            .end_iteration(current_iteration_map).await;
    }

    fn evaluate_program(&self, p: &mut Arc<SubProgram>) -> bool {
        // trace!(target: "ruse::synthesizer", "Evaluating program {}", p.get_code());
        self.statistics.inc_value(StatisticsTypes::Evaluated);
        unsafe { Arc::get_mut(p).unwrap_unchecked() }.evaluate(&self.context)
    }

    fn get_program_from_composite_opcode(
        &self,
        op: Arc<dyn ExprOpcode>,
        triplet: &ProgTriplet,
    ) -> Option<Arc<SubProgram>> {
        debug_assert!(!op.arg_types().is_empty());

        let triplet_clone = triplet.clone();
        let mut p = SubProgram::with_opcode_and_children(
            op,
            triplet_clone.children,
            triplet_clone.pre_ctx,
            triplet_clone.post_ctx,
        );
        self.evaluate_program(&mut p).then(|| p)
    }

    fn get_program_from_init_opcode(
        &self,
        op: Arc<dyn ExprOpcode>,
        ctx: &ContextArray,
    ) -> Option<Arc<SubProgram>> {
        debug_assert!(op.arg_types().is_empty());

        let pre_ctx = ctx.get_partial_context(op.required_variables())?;
        let post_ctx = pre_ctx.clone();
        let mut p = SubProgram::with_opcode(op.clone(), pre_ctx, post_ctx);
        match self.evaluate_program(&mut p) {
            true => Some(p),
            false => None,
        }
    }

    async fn check_program(&self, p: &Arc<SubProgram>) -> bool {
        if p.post_ctx().depth > self.max_context_depth {
            return false;
        }
        if !p.out_value().iter().all(|x| self.check_out_value(x.val())) {
            return false;
        }
        if !(self.valid)(p, &self.context) {
            return false;
        }

        if self.bank.output_exists(p).await {
            return false;
        }

        true
    }

    fn check_out_value(&self, val: &Value) -> bool {
        if let Some(num) = val.number_value() {
            if !ALLOW_NON_FINITE_NUMBER && !num.0.is_finite() {
                return false;
            }
        }

        true
    }

    async fn insert_program(&self, p: Arc<SubProgram>, batch_builder: &mut <P::IterationBuilderType as BankIterationBuilder>::BatchBuilderType) -> bool {
        if p.is_terminal() {
            return true;
        }

        if batch_builder.add_program(&p).await {
            trace_prog!(target: "ruse::synthesizer", p, "Inserted");

            self.statistics.inc_value(StatisticsTypes::BankSize);
            self.statistics
                .max_value(StatisticsTypes::MaxDepth, p.depth().into());
            self.statistics
                .max_value(StatisticsTypes::MaxSize, p.size().into());

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

#[derive(serde::Serialize)]
pub struct SynthesizerJsonDisplay {
    opcodes: Vec<String>,
    context: SynthesizerContextJsonDisplay,
    max_context_depth: usize,
    worker_count: usize,
}

impl Display for SynthesizerJsonDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = serde_json::to_string_pretty(self).unwrap_or_else(|e| format!("Failed to serialize synthesizer: {}", e));
        write!(f, "{}", value)
    }
}

impl<P: ProgBank + 'static> Synthesizer<P> {
    pub fn json_display(&self) -> impl Display {
        self.json_display_struct()
    }

    pub(crate) fn json_display_struct(&self) -> SynthesizerJsonDisplay {
        SynthesizerJsonDisplay {
            opcodes: self.opcodes.values().flat_map(|ops| ops.iter().map(|op| format!("{}:{:?}", op.op_name(), op))).collect(),
            context: self.context.json_display_struct(),
            max_context_depth: self.max_context_depth,
            worker_count: self.worker_count,
        }
    }
}