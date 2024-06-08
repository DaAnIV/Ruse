use dashmap::DashSet;
use ruse_object_graph::Cache;

use crate::{bank::*, opcode::ExprOpcode, prog::SubProgram, work_gatherer::WorkGather};
use std::{
    fmt::Display,
    ops::Index,
    sync::{atomic::*, Arc},
};

use serde::ser::SerializeStruct;

use tokio_util::sync::CancellationToken;

pub type OpcodesList = Vec<Arc<dyn ExprOpcode>>;

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
        Self { values: values }
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

pub struct Synthesizer {
    bank: ProgBank,
    init_opcodes: OpcodesList,
    composite_opcodes: OpcodesList,
    start_context: ContextArray,
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
    ) -> Self {
        let (init_opcodes, composite_opcodes) =
            opcodes.into_iter().partition(|x| x.arg_types().len() == 0);

        let new_obj = Self {
            bank: Default::default(),
            init_opcodes: init_opcodes,
            composite_opcodes: composite_opcodes,
            start_context: start_context.clone(),
            found_contexts: DashSet::new(),
            max_context_depth: max_context_depth,
            cancel_token: CancellationToken::new(),
            predicate: predicate,
            valid: valid,
            statistics: Default::default(),
        };

        new_obj.found_contexts.insert(start_context);
        new_obj.statistics.inc_value(StatisticsTypes::ContextSize);
        new_obj
    }

    pub fn get_cancel_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }

    fn init_context(&self, iteration_map: &ContextMap, ctx: &ContextArray, cache: &Arc<Cache>) {
        let type_map: TypeMap = Default::default();
        for op in &self.init_opcodes {
            let p = match self.get_program_from_init_opcode(op.clone(), ctx, cache) {
                Some(p) => p,
                None => continue,
            };
            if type_map.insert_program(p) {
                self.statistics.inc_value(StatisticsTypes::BankSize);
            }
        }

        self.statistics.inc_value(StatisticsTypes::ContextSize);
        iteration_map.insert(ctx, type_map);
    }

    fn create_work_gatherer(
        this: Arc<Self>,
        current_iteration_map: Arc<ContextMap>,
        cache: Arc<Cache>,
    ) -> WorkGather {
        let child_token = this.cancel_token.child_token();
        WorkGather::new(
            Arc::new(
                move |op: Arc<dyn ExprOpcode>, children: Vec<Arc<SubProgram>>| {
                    let p = match this.get_program_from_composite_opcode(op, children, &cache) {
                        Some(p) => p,
                        None => return None,
                    };
                    if !this.check_and_insert_program(p.clone(), current_iteration_map.as_ref()) {
                        return None;
                    }
                    // println!("Inserting {{{}}}[0] = {}", p.get_code(), p.out_value()[0].val());

                    if this.found_contexts.insert(p.post_ctx().clone()) {
                        // println!("{} initializes a new context {:?}", p.get_code(), p.post_ctx());
                        this.init_context(current_iteration_map.as_ref(), p.post_ctx(), &cache);
                    }
                    if &this.start_context == p.pre_ctx() && (this.predicate)(&p) {
                        return Some(p);
                    }

                    return None;
                },
            ),
            1000,
            child_token
        )
    }

    pub async fn run_iteration(
        this: &mut Arc<Self>,
        cache: &Arc<Cache>,
    ) -> Option<Arc<SubProgram>> {
        let iteration = this.bank.iteration_count();
        let current_iteration_map: Arc<ContextMap> = Default::default();

        let mut found_prog = None;

        if iteration == 0 {
            for op in &this.init_opcodes {
                let p =
                    match this.get_program_from_init_opcode(op.clone(), &this.start_context, cache)
                    {
                        Some(p) => p,
                        None => continue,
                    };
                if !this.check_and_insert_program(p.clone(), &current_iteration_map) {
                    continue;
                }
                if this.found_contexts.insert(p.post_ctx().clone()) {
                    // println!("{} initializes a new context", p.get_code());
                    this.init_context(&current_iteration_map, p.post_ctx(), cache);
                }
                if (this.predicate)(&p) {
                    found_prog = Some(p);
                    break;
                }
            }
        } else {
            let mut work_gatherer = Synthesizer::create_work_gatherer(
                this.clone(),
                current_iteration_map.clone(),
                cache.clone(),
            );
            for op in &this.composite_opcodes {
                work_gatherer.gather_work_for_next_iteration(&this.bank, op)
            }
            found_prog = work_gatherer.wait_for_all_tasks().await;
        }

        Synthesizer::insert_iteration(this, current_iteration_map);

        found_prog
    }

    fn insert_iteration(this: &mut Arc<Self>, current_iteration_map: Arc<ContextMap>) {
        unsafe {
            Arc::get_mut(this)
                .unwrap_unchecked()
                .bank
                .insert(current_iteration_map.into());
        }
    }

    fn evaluate_program(&self, p: &mut Arc<SubProgram>, cache: &Arc<Cache>) -> bool {
        self.statistics.inc_value(StatisticsTypes::Evaluated);
        unsafe { Arc::get_mut(p).unwrap_unchecked() }.evaluate(cache)
    }

    fn get_program_from_composite_opcode(
        &self,
        op: Arc<dyn ExprOpcode>,
        args: Vec<Arc<SubProgram>>,
        cache: &Arc<Cache>,
    ) -> Option<Arc<SubProgram>> {
        debug_assert!(op.arg_types().len() > 0);

        let mut p = SubProgram::with_opcode_and_children(op, args);
        match self.evaluate_program(&mut p, cache) {
            true => Some(p),
            false => None,
        }
    }

    fn get_program_from_init_opcode(
        &self,
        op: Arc<dyn ExprOpcode>,
        ctx: &ContextArray,
        cache: &Arc<Cache>,
    ) -> Option<Arc<SubProgram>> {
        debug_assert!(op.arg_types().len() == 0);

        let mut p = SubProgram::with_opcode_and_context(op.clone(), ctx);
        match self.evaluate_program(&mut p, cache) {
            true => Some(p),
            false => None,
        }
    }

    fn check_program(&self, p: &Arc<SubProgram>) -> bool {
        if p.post_ctx()[0].number_of_changes() > self.max_context_depth {
            return false;
        }
        if self.bank.output_exists(p) {
            return false;
        }
        if !(self.valid)(p) {
            return false;
        }

        return true;
    }

    fn check_and_insert_program(&self, p: Arc<SubProgram>, iteration_map: &ContextMap) -> bool {
        if !self.check_program(&p) {
            return false;
        }

        if iteration_map.insert_program(p.clone()) {
            self.statistics.inc_value(StatisticsTypes::BankSize);
            self.statistics
                .max_value(StatisticsTypes::MaxDepth, p.depth().into());
            self.statistics
                .max_value(StatisticsTypes::MaxSize, p.size().into());

            return true;
        }
        return false;
    }

    #[inline]
    pub fn statistics(&self) -> CurrentStatistics {
        self.statistics.current()
    }
}
