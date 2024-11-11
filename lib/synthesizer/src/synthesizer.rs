use crate::{
    bank::*,
    bank_iterator::bank_iterator,
    context::{ContextArray, SynthesizerContext},
    multi_programs_map_product::ProgTriplet,
    opcode::ExprOpcode,
    prog::SubProgram,
};
use dashmap::DashSet;
use ruse_object_graph::{
    value::{Value, ValueType},
    Cache, CachedString,
};
use serde::ser::SerializeStruct;
use std::{
    collections::HashMap,
    fmt::Display,
    ops::Index,
    sync::{atomic::*, Arc},
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, trace};

pub type OpcodesList = Vec<Arc<dyn ExprOpcode>>;
const ALLOW_NON_FINITE_NUMBER: bool = false;

#[repr(usize)]
#[derive(Clone, Copy, Debug)]
pub enum StatisticsTypes {
    Evaluated,
    BankSize,
    ContextSize,
    MaxDepth,
    MaxSize,
    __MaxType,
}

impl StatisticsTypes {
    pub fn iterator() -> impl Iterator<Item = StatisticsTypes> {
        [
            StatisticsTypes::Evaluated,
            StatisticsTypes::BankSize,
            StatisticsTypes::ContextSize,
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
            StatisticsTypes::ContextSize => "ContextSize",
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
        values[StatisticsTypes::ContextSize as usize] -= rhs[StatisticsTypes::ContextSize];
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
type OpcodesMap = HashMap<Vec<ValueType>, Vec<Arc<dyn ExprOpcode>>>;

pub struct Synthesizer {
    bank: ProgBank,
    opcodes: OpcodesMap,
    context: SynthesizerContext,
    found_contexts: DashSet<ContextArray>,
    max_context_depth: usize,
    cancel_token: CancellationToken,

    predicate: SynthesizerPredicate,
    valid: SynthesizerPredicate,

    statistics: Statistics,
}

impl Synthesizer {
    pub fn new(
        start_context: ContextArray,
        opcodes: OpcodesList,
        predicate: SynthesizerPredicate,
        valid: SynthesizerPredicate,
        max_context_depth: usize,
        cache: Arc<Cache>,
    ) -> Self {
        Self {
            bank: Default::default(),
            opcodes: Self::sort_opcodes(opcodes),
            context: SynthesizerContext::from_context_array(start_context.clone(), cache),
            found_contexts: DashSet::new(),
            max_context_depth,
            cancel_token: CancellationToken::new(),
            predicate,
            valid,
            statistics: Default::default(),
        }
    }

    fn sort_opcodes(opcodes: OpcodesList) -> OpcodesMap {
        let mut sorted_opcodes: OpcodesMap = OpcodesMap::default();
        for op in opcodes {
            if let Some(list) = sorted_opcodes.get_mut(op.arg_types()) {
                list.push(op);
            } else {
                sorted_opcodes.insert(op.arg_types().to_vec(), vec![op]);
            }
        }

        sorted_opcodes.shrink_to_fit();

        sorted_opcodes
    }

    fn init_opcodes(&self) -> impl Iterator<Item = &Arc<dyn ExprOpcode>> {
        self.opcodes[&vec![]].iter()
    }

    fn composite_opcodes(
        &self,
    ) -> impl Iterator<Item = (&Vec<ValueType>, &Vec<Arc<dyn ExprOpcode>>)> {
        self.opcodes
            .iter()
            .filter(|(arg_types, _)| !arg_types.is_empty())
    }

    pub fn get_cancel_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }

    fn init_context<const IS_START_CTX: bool>(
        &self,
        iteration_map: &TypeMap,
        ctx: &ContextArray,
    ) -> Option<Arc<SubProgram>> {
        debug!(target: "ruse::synthesizer", "Initializing context {}", ctx);

        self.found_contexts.insert(ctx.clone());
        for op in self.init_opcodes() {
            let p = match self.get_program_from_init_opcode(op.clone(), ctx) {
                Some(p) => p,
                None => continue,
            };
            if self.found_contexts.insert(p.pre_ctx().clone()) {
                self.statistics.inc_value(StatisticsTypes::ContextSize);
            }
            if !self.check_and_insert_program(p.clone(), iteration_map) {
                continue;
            }
            if IS_START_CTX && (self.predicate)(&p) {
                return Some(p);
            }
        }

        None
    }

    fn handler(
        this: Arc<Self>,
        current_iteration_map: Arc<TypeMap>,
    ) -> Arc<impl Fn(Arc<dyn ExprOpcode>, ProgTriplet) -> Option<Arc<SubProgram>>> {
        Arc::new(move |op: Arc<dyn ExprOpcode>, triplet: ProgTriplet| {
            trace!(target: "ruse::synthesizer", "Evaluating");
            trace!(target: "ruse::synthesizer", "pre: {}", triplet.pre_ctx);
            triplet
                .children
                .iter()
                .for_each(|c| trace!(target: "ruse::synthesizer", "{}", c));
            trace!(target: "ruse::synthesizer", "post: {}", triplet.post_ctx);

            let p = match this.get_program_from_composite_opcode(
                triplet.pre_ctx,
                op,
                triplet.post_ctx,
                triplet.children,
            ) {
                Some(p) => p,
                None => return None,
            };
            if !this.check_and_insert_program(p.clone(), current_iteration_map.as_ref()) {
                return None;
            }

            if this.found_contexts.insert(p.post_ctx().clone()) {
                this.init_context::<false>(current_iteration_map.as_ref(), p.post_ctx());
            }
            if p.pre_ctx().subset(&this.context.start_context) && (this.predicate)(&p) {
                return Some(p);
            }

            None
        })
    }

    pub async fn run_iteration(this: &mut Arc<Self>) -> Option<Arc<SubProgram>> {
        let current_iteration_map: Arc<TypeMap> = Default::default();

        let found_prog =
            Self::run_iteration_inner(this.clone(), current_iteration_map.clone()).await;

        Self::insert_iteration(this, current_iteration_map);

        found_prog
    }

    pub async fn run_iteration_inner(
        this: Arc<Self>,
        current_iteration_map: Arc<TypeMap>,
    ) -> Option<Arc<SubProgram>> {
        debug!(target: "ruse::synthesizer", "Starting iteration {}", this.bank.iteration_count());

        tokio::spawn(async move {
            if this.bank.iteration_count() == 0 {
                this.run_init_iteration(current_iteration_map)
            } else {
                Self::run_composite_iteration(this, current_iteration_map).await
            }
        })
        .await
        .unwrap()
    }

    fn run_init_iteration(&self, current_iteration_map: Arc<TypeMap>) -> Option<Arc<SubProgram>> {
        self.init_context::<true>(&current_iteration_map, &self.context.start_context)
    }

    async fn is_cancelled(&self) -> bool {
        tokio::task::yield_now().await;
        self.cancel_token.is_cancelled()
    }

    async fn run_composite_iteration(
        this: Arc<Self>,
        current_iteration_map: Arc<TypeMap>,
    ) -> Option<Arc<SubProgram>> {
        let handler = Self::handler(this.clone(), current_iteration_map);

        for (arg_types, ops) in this.composite_opcodes() {
            for triplet in bank_iterator(&this.bank, arg_types) {
                if this.is_cancelled().await {
                    return None;
                }
                for op in ops {
                    if let Some(found) = handler(op.clone(), triplet.clone()) {
                        return Some(found);
                    }
                }
            }
        }

        None
    }

    fn insert_iteration(this: &mut Arc<Self>, current_iteration_map: Arc<TypeMap>) {
        Arc::get_mut(this)
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
        pre_ctx: ContextArray,
        op: Arc<dyn ExprOpcode>,
        post_ctx: ContextArray,
        args: Vec<Arc<SubProgram>>,
    ) -> Option<Arc<SubProgram>> {
        debug_assert!(!op.arg_types().is_empty());

        let mut p = SubProgram::with_opcode_and_children(op, args, pre_ctx, post_ctx);
        let res = match self.evaluate_program(&mut p) {
            true => Some(p),
            false => None,
        };

        res
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

        if self.bank.output_exists(p) {
            return false;
        }
        if !(self.valid)(p) {
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

    fn check_and_insert_program(&self, p: Arc<SubProgram>, iteration_map: &TypeMap) -> bool {
        if !self.check_program(&p) {
            return false;
        }

        if iteration_map.insert_program(p.clone()) {
            trace!(target: "ruse::synthesizer", "Inserted program");
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
