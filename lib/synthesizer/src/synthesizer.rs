use ruse_object_graph::Cache;

use crate::{
    arg_iter::ArgIterator,
    bank::*,
    opcode::{ExprAst, SynthesizerExprOpcode},
    prog::SubProgram,
};
use std::{
    collections::HashSet,
    sync::{atomic::*, Arc},
};

pub type OpcodesList<T> = Vec<Arc<dyn SynthesizerExprOpcode<T>>>;

#[derive(Default)]
pub struct Statistics {
    generated: AtomicU64,
    bank_size: AtomicU64,
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
}

pub struct Synthesizer<T: ExprAst + Default, const N: usize> {
    bank: ProgBank<T, N>,
    init_opcodes: OpcodesList<T>,
    composite_opcodes: OpcodesList<T>,
    found_contexts: HashSet<ContextArray<N>>,
    statistics: Statistics,
}

impl<T: ExprAst + Default, const N: usize> Synthesizer<T, N> {
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
            found_contexts: HashSet::new(),
            statistics: Default::default(),
        };

        new_obj.found_contexts.insert(start_context);
        new_obj
    }

    fn insert_program_to_type_map(
        &self,
        type_map: &mut TypeMap<T, N>,
        p: Arc<SubProgram<T, N>>,
    ) -> bool {
        let value_map = &mut type_map[p.out_type() as usize];
        if !value_map.contains_key(p.out_value()) && !self.bank.output_exists(&p) {
            value_map.insert(p.out_value().clone(), p);
            return true;
        }

        return false;
    }

    fn init_context(&mut self, ctx: &ContextArray<N>, cache: &Cache) {
        let mut type_map = new_type_map::<T, N>();
        for op in &self.init_opcodes {
            let mut p = SubProgram::<T, N>::with_opcode_and_context(op.clone(), ctx);
            Self::evaluate_program(&mut p, cache);

            self.statistics.generated.fetch_add(1, Ordering::Relaxed);
            if self.insert_program_to_type_map(&mut type_map, p.into()) {
                self.statistics.bank_size.fetch_add(1, Ordering::Relaxed);
            }
        }

        self.bank.insert(1, ctx.clone(), type_map);
    }

    pub fn synthesize_for_size<F, V>(
        &mut self,
        ctx: &ContextArray<N>,
        n: usize,
        cache: &Cache,
        predicate: F,
        valid: V,
    ) -> Option<Arc<SubProgram<T, N>>>
    where
        F: Fn(&Arc<SubProgram<T, N>>) -> bool,
        V: Fn(&Arc<SubProgram<T, N>>) -> bool,
    {
        let mut type_map = new_type_map::<T, N>();
        let mut found_contexts = HashSet::<ContextArray<N>>::new();

        let mut found_prog = None;

        if n == 1 {
            for op in &self.init_opcodes {
                let p = Self::get_program_from_init_opcode(op.clone(), ctx, &cache);
                if !self.check_and_insert_program(
                    p.clone(),
                    &mut type_map,
                    &mut found_contexts,
                    &valid,
                ) {
                    continue;
                }
                if predicate(&p) {
                    found_prog = Some(p);
                    break;
                }
            }
        } else {
            for op in &self.composite_opcodes {
                if op.arg_types().len() >= n {
                    continue;
                }
                for args in ArgIterator::new(&self.bank, ctx, n - 1, op.arg_types()) {
                    let p = Self::get_program_from_composite_opcode(op.clone(), args, &cache);
                    if !self.check_and_insert_program(
                        p.clone(),
                        &mut type_map,
                        &mut found_contexts,
                        &valid,
                    ) {
                        continue;
                    }
                    if predicate(&p) {
                        found_prog = Some(p);
                        break;
                    }
                }
            }
        }

        self.bank.insert(n, ctx.clone(), type_map);

        for ctx in found_contexts.iter() {
            if self.found_contexts.insert(ctx.clone()) {
                self.init_context(ctx, cache)
            }
        }

        found_prog
    }

    fn evaluate_program(p: &mut Arc<SubProgram<T, N>>, cache: &Cache) {
        unsafe { Arc::get_mut(p).unwrap_unchecked() }.evaluate(cache);
    }

    fn get_program_from_composite_opcode(
        op: Arc<dyn SynthesizerExprOpcode<T>>,
        args: Vec<Arc<SubProgram<T, N>>>,
        cache: &Cache,
    ) -> Arc<SubProgram<T, N>> {
        debug_assert!(op.arg_types().len() > 0);

        let mut p = SubProgram::with_opcode_and_children(op.clone(), args);
        Self::evaluate_program(&mut p, cache);

        p
    }

    fn get_program_from_init_opcode(
        op: Arc<dyn SynthesizerExprOpcode<T>>,
        ctx: &ContextArray<N>,
        cache: &Cache,
    ) -> Arc<SubProgram<T, N>> {
        debug_assert!(op.arg_types().len() == 0);

        let mut p = SubProgram::with_opcode_and_context(op.clone(), ctx);
        Self::evaluate_program(&mut p, cache);

        p
    }

    fn check_and_insert_program<V>(
        &self,
        p: Arc<SubProgram<T, N>>,
        type_map: &mut TypeMap<T, N>,
        found_contexts: &mut HashSet<ContextArray<N>>,
        valid: V,
    ) -> bool
    where
        V: Fn(&Arc<SubProgram<T, N>>) -> bool,
    {
        if !valid(&p) {
            return false;
        }
        self.statistics.generated.fetch_add(1, Ordering::Relaxed);
        found_contexts.insert(p.post_ctx().clone());

        if self.insert_program_to_type_map(type_map, p.clone()) {
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
