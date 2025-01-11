use crate::{
    bank::*,
    bank_iterator::{bank_iterator, BankIterator},
    context::{ContextArray, SynthesizerContext},
    multi_programs_map_product::ProgramChildrenIterator,
    opcode::*,
    prog::SubProgram,
    prog_triplet::ProgTriplet,
    prog_triplet_iterator::{prog_triplet_iterator, ProgTripletIterator},
};
use dashmap::DashSet;
use ruse_object_graph::{
    value::{Value, ValueType},
    CachedString,
};
use serde::ser::SerializeStruct;
use std::{
    fmt::Display,
    ops::Index,
    sync::{atomic::*, Arc},
};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, trace};

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

pub type SynthesizerPredicate = Box<dyn Fn(&Arc<SubProgram>) -> bool + Send + Sync>;

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

    fn init_context<const IS_START_CTX: bool>(
        &self,
        iteration_map: &TypeMap<P::T>,
        ctx: &ContextArray,
    ) -> Option<Arc<SubProgram>> {
        trace!(target: "ruse::synthesizer", "Initializing context");
        trace!(target: "ruse::synthesizer", "{}", ctx);

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

            if !self.check_program(&p) {
                continue;
            }

            if !self.insert_program(p.clone(), iteration_map) {
                continue;
            }

            if IS_START_CTX && (self.predicate)(&p) {
                res = Some(p);
                break;
            }
        }

        trace!(target: "ruse::synthesizer", "Finished initializing context");
        res
    }

    pub async fn run_iteration(self: &mut Arc<Self>) -> Option<Arc<SubProgram>> {
        let current_iteration_map: Arc<TypeMap<P::T>> = Default::default();

        let found_prog =
            Self::run_iteration_inner(self.clone(), current_iteration_map.clone()).await;

        Self::insert_iteration(self, current_iteration_map);

        found_prog
    }

    async fn run_iteration_inner(
        self: Arc<Self>,
        current_iteration_map: Arc<TypeMap<P::T>>,
    ) -> Option<Arc<SubProgram>> {
        debug!(target: "ruse::synthesizer", "Starting iteration {}", self.bank.iteration_count());

        let res = tokio::spawn(async move {
            if self.bank.iteration_count() == 0 {
                self.run_init_iteration(current_iteration_map)
            } else {
                Self::run_composite_iteration(self, current_iteration_map).await
            }
        })
        .await;

        match res {
            Ok(iter_output) => iter_output,
            Err(err) => {
                error!(target: "ruse::synthesizer", "Got error {}", err);
                panic!("{}", err);
            }
        }
    }

    fn run_init_iteration(
        &self,
        current_iteration_map: Arc<TypeMap<P::T>>,
    ) -> Option<Arc<SubProgram>> {
        self.init_context::<true>(&current_iteration_map, &self.context.start_context)
    }

    fn composite_iter_batch(
        &self,
        triplet: &ProgTriplet,
        ops: &OpcodesList,
        current_batch_map: &TypeMap<P::T>,
    ) -> Option<Arc<SubProgram>> {
        for op in ops {
            let Some(p) = self.get_program_from_composite_opcode(op.clone(), triplet) else {
                continue;
            };

            if !self.check_program(&p) {
                continue;
            }

            if !self.insert_program(p.clone(), current_batch_map) {
                continue;
            }

            if p.pre_ctx().subset(&self.context.start_context) && (self.predicate)(&p) {
                debug!(target: "ruse::synthesizer", "Found program \"{}\"", p.get_code());
                trace!(target: "ruse::synthesizer", "{}", p);
                return Some(p);
            }
        }

        None
    }

    async fn should_end_worker(&self) -> bool {
        self.cancel_token.is_cancelled() || self.found_token.is_cancelled()
    }

    fn worker_triple_iterator<'a>(
        &'a self,
        i: usize,
        arg_types: &'a Vec<ValueType>,
    ) -> ProgTripletIterator<BankIterator<'a, P>> {
        let mut children_iterator = bank_iterator(&self.bank, arg_types);
        let total_size = children_iterator.remaining();
        let skip = (total_size / self.worker_count) * i;
        let take = if i == self.worker_count - 1 {
            usize::MAX
        } else {
            total_size / self.worker_count
        };
        children_iterator.skip(skip);
        children_iterator.take(take);

        prog_triplet_iterator(children_iterator)
    }

    async fn composite_iteration_worker(
        self: Arc<Self>,
        i: usize,
    ) -> (TypeMap<P::T>, Option<Arc<SubProgram>>) {
        let type_map = TypeMap::default();

        for (arg_types, ops) in self.composite_opcodes() {
            for triple in self.worker_triple_iterator(i, arg_types) {
                if self.should_end_worker().await {
                    return (type_map, None);
                }
                let found = self.composite_iter_batch(&triple, &ops, &type_map);
                if found.is_some() {
                    self.found_token.cancel();
                    return (type_map, found);
                }
            }
        }

        (type_map, None)
    }

    async fn run_composite_iteration(
        self: Arc<Self>,
        current_iteration_map: Arc<TypeMap<P::T>>,
    ) -> Option<Arc<SubProgram>> {
        let mut workers = JoinSet::new();
        for i in 0..self.worker_count {
            workers.spawn(Self::composite_iteration_worker(self.clone(), i));
        }

        while let Some(Ok((worker_type_map, found))) = workers.join_next().await {
            if self.cancel_token.is_cancelled() {
                return None;
            }
            if found.is_some() {
                workers.abort_all();
                while workers.join_next().await.is_some() {}
                return found;
            }
            current_iteration_map.take_minimal_prog(worker_type_map);
        }

        debug!(target: "ruse::synthesizer", "Initializing new contexts!");

        let new_ctx = TypeMap::default();
        for programs_map in current_iteration_map.iter() {
            for p in programs_map.iter() {
                if self.cancel_token.is_cancelled() {
                    return None;
                }
                if self.found_contexts.insert(p.post_ctx().clone()) {
                    trace!(target: "ruse::synthesizer", "New post context found by program \"{}\"", p.get_code());
                    self.init_context::<false>(&new_ctx, p.post_ctx());
                }
            }
        }
        current_iteration_map.take_minimal_prog(new_ctx);

        None
    }

    fn insert_iteration(self: &mut Arc<Self>, current_iteration_map: Arc<TypeMap<P::T>>) {
        Arc::get_mut(self)
            .unwrap()
            .bank
            .insert(current_iteration_map);
    }

    fn evaluate_program(&self, p: &mut Arc<SubProgram>) -> bool {
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

    fn check_program(&self, p: &Arc<SubProgram>) -> bool {
        if p.post_ctx().depth > self.max_context_depth {
            return false;
        }
        if !p.out_value().iter().all(|x| self.check_out_value(x.val())) {
            return false;
        }
        if !(self.valid)(p) {
            return false;
        }

        if self.bank.output_exists(p) {
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

    fn insert_program(&self, p: Arc<SubProgram>, iteration_map: &TypeMap<P::T>) -> bool {
        if p.is_terminal() {
            return true;
        }

        if iteration_map.insert_program(p.clone()) {
            trace!(target: "ruse::synthesizer", "Inserted program \"{}\"", p.get_code());
            trace!(target: "ruse::synthesizer", "{}", p);

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

    pub fn set_immutable(&mut self, var: &CachedString) {
        self.context.set_immutable(var);
    }

    pub fn print_all_programs(&self) {
        self.bank.print_all_programs()
    }
}
