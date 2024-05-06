use ruse_object_graph::Cache;

use crate::{
    bank::*,
    opcode::{ExprAst, ExprOpcode},
};
use std::{
    collections::HashSet,
    fmt::Display,
    sync::{atomic::*, Arc},
};

pub type OpcodesList<T> = Vec<Arc<dyn ExprOpcode<T>>>;

#[derive(Default, Debug)]
pub struct Statistics {
    generated: AtomicU64,
    bank_size: AtomicU64,
    context_count: AtomicU64,
}

impl Statistics {
    #[inline]
    pub fn generated(&self) -> u64 {
        self.generated.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn bank_size(&self) -> u64 {
        self.bank_size.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn context_count(&self) -> u64 {
        self.context_count.load(Ordering::Relaxed)
    }
}

impl Display for Statistics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Generated: {}, bank size: {}, context count: {}",
            self.generated(),
            self.bank_size(),
            self.context_count()
        )
    }
}

pub struct Synthesizer<T: ExprAst + Default, const N: usize, const MAX_DEPTH: usize = 2> {
    bank: ProgBank<T, N>,
    init_opcodes: OpcodesList<T>,
    composite_opcodes: OpcodesList<T>,
    start_context: ContextArray<N>,
    found_contexts: HashSet<ContextArray<N>>,

    statistics: Statistics,
}

impl<T: ExprAst + Default, const N: usize, const MAX_DEPTH: usize> Synthesizer<T, N, MAX_DEPTH> {
    pub fn with_context_and_opcodes(
        start_context: ContextArray<N>,
        opcodes: OpcodesList<T>,
    ) -> Self {
        let (init_opcodes, composite_opcodes) =
            opcodes.into_iter().partition(|x| x.arg_types().len() == 0);

        let mut new_obj = Self {
            bank: Default::default(),
            init_opcodes: init_opcodes,
            composite_opcodes: composite_opcodes,
            start_context: start_context.clone(),
            found_contexts: HashSet::new(),
            statistics: Default::default(),
        };

        new_obj.found_contexts.insert(start_context);
        new_obj
            .statistics
            .context_count
            .fetch_add(1, Ordering::Relaxed);
        new_obj
    }

    fn init_context(
        &self,
        iteration_map: &mut ContextMap<T, N>,
        ctx: &ContextArray<N>,
        cache: &Cache,
    ) {
        let mut type_map = new_type_map::<T, N>();
        for op in &self.init_opcodes {
            let p = self.get_program_from_init_opcode(op.clone(), ctx, cache);

            self.statistics.generated.fetch_add(1, Ordering::Relaxed);
            if type_map[p.out_type() as usize].insert(p) {
                self.statistics.bank_size.fetch_add(1, Ordering::Relaxed);
            }
        }

        self.statistics
            .context_count
            .fetch_add(1, Ordering::Relaxed);
        iteration_map.insert(ctx, type_map);
    }

    pub fn run_iteration<F, V>(
        &mut self,
        cache: &Cache,
        predicate: F,
        valid: V,
    ) -> Option<Arc<SubProgram<T, N>>>
    where
        F: Fn(&Arc<SubProgram<T, N>>) -> bool,
        V: Fn(&Arc<SubProgram<T, N>>) -> bool,
    {
        let iteration = self.bank.iteration_count();
        let mut current_iteration_map: ContextMap<T, N> = Default::default();

        let mut found_prog = None;

        if iteration == 0 {
            for op in &self.init_opcodes {
                let p = self.get_program_from_init_opcode(op.clone(), &self.start_context, &cache);
                if !self.check_and_insert_program(p.clone(), &valid, &mut current_iteration_map) {
                    continue;
                }
                if self.found_contexts.insert(p.post_ctx().clone()) {
                    // println!("{} initializes a new context", p.get_code());
                    self.init_context(&mut current_iteration_map, p.post_ctx(), cache);
                }
                if predicate(&p) {
                    found_prog = Some(p);
                    break;
                }
            }
        } else {
            for op in &self.composite_opcodes {
                let children = ChildrenIterator::new(&self.bank, iteration, op.arg_types());
                for args in children {
                    let p = self.get_program_from_composite_opcode(op.clone(), args, &cache);
                    if p.post_ctx()[0].number_of_changes() > MAX_DEPTH {
                        continue;
                    }
                    if !self.check_and_insert_program(p.clone(), &valid, &mut current_iteration_map)
                    {
                        continue;
                    }
                    if self.found_contexts.insert(p.post_ctx().clone()) {
                        // println!("{} initializes a new context {:?}", p.get_code(), p.post_ctx());
                        self.init_context(&mut current_iteration_map, p.post_ctx(), cache);
                    }

                    if &self.start_context == p.pre_ctx() && predicate(&p) {
                        found_prog = Some(p);
                        break;
                    }
                }
            }
        }

        self.bank.insert(current_iteration_map);

        found_prog
    }

    fn evaluate_program(&self, p: &mut Arc<SubProgram<T, N>>, cache: &Cache) {
        unsafe { Arc::get_mut(p).unwrap_unchecked() }.evaluate(cache);
        self.statistics.generated.fetch_add(1, Ordering::Relaxed);
        // println!("{{{}}} generated", p.get_code());
    }

    fn get_program_from_composite_opcode(
        &self,
        op: Arc<dyn ExprOpcode<T>>,
        args: Vec<Arc<SubProgram<T, N>>>,
        cache: &Cache,
    ) -> Arc<SubProgram<T, N>> {
        debug_assert!(op.arg_types().len() > 0);

        let mut p = SubProgram::with_opcode_and_children(op.clone(), args);
        self.evaluate_program(&mut p, cache);

        p
    }

    fn get_program_from_init_opcode(
        &self,
        op: Arc<dyn ExprOpcode<T>>,
        ctx: &ContextArray<N>,
        cache: &Cache,
    ) -> Arc<SubProgram<T, N>> {
        debug_assert!(op.arg_types().len() == 0);

        let mut p = SubProgram::with_opcode_and_context(op.clone(), ctx);
        self.evaluate_program(&mut p, cache);

        p
    }

    fn check_and_insert_program<V>(
        &self,
        p: Arc<SubProgram<T, N>>,
        valid: V,
        iteration_map: &mut ContextMap<T, N>,
    ) -> bool
    where
        V: Fn(&Arc<SubProgram<T, N>>) -> bool,
    {
        if self.bank.output_exists(&p) {
            return false;
        }
        if !valid(&p) {
            return false;
        }

        if iteration_map.insert_program(p.clone()) {
            self.statistics.bank_size.fetch_add(1, Ordering::Relaxed);

            return true;
        }
        return false;
    }

    #[inline]
    pub fn statistics(&self) -> &Statistics {
        &self.statistics
    }
}
