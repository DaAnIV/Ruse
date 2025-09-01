use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    hash::Hash,
    sync::Arc,
};

use itertools::Either;
use ruse_object_graph::ValueType;

use ruse_synthesizer::{
    bank::*, bank_hasher::BankHasherBuilder, context::ContextArray, prog::SubProgram,
    value_array::ValueArray,
};

use crate::config::SubsumptionBankConfig;

// The bank is hierarchical
// iteration -> out_type -> sub_prog

#[derive(Debug, Clone)]
pub(crate) struct ProgOutput(Arc<SubProgram>);

impl ProgOutput {
    fn out_type(&self) -> &ValueType {
        self.0.out_type()
    }
    fn out_value(&self) -> &ValueArray {
        self.0.out_value()
    }
    fn effect(&self) -> Option<&ContextArray> {
        if self.0.dirty() {
            Some(self.0.post_ctx())
        } else {
            None
        }
    }
}

impl From<Arc<SubProgram>> for ProgOutput {
    fn from(value: Arc<SubProgram>) -> Self {
        Self(value)
    }
}

impl Eq for ProgOutput {}

impl PartialEq for ProgOutput {
    fn eq(&self, other: &Self) -> bool {
        self.out_type() == other.out_type()
            && self
                .out_value()
                .eq(self.0.post_ctx(), other.out_value(), other.0.post_ctx())
            && self.effect() == other.effect()
    }
}

impl Hash for ProgOutput {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.out_type().hash(state);
        self.out_value().wrap(self.0.post_ctx()).hash(state);
        self.effect().hash(state);
    }
}

#[derive(Debug)]
pub struct SubsumptionProgramsMap {
    map: HashMap<ProgOutput, Vec<Arc<SubProgram>>, BankHasherBuilder>,
    set: HashSet<Arc<SubProgram>, BankHasherBuilder>,
}

enum MinmalProgResult<'a> {
    LargerProg(&'a mut Arc<SubProgram>),
    SmallerProg,
    NonComparable,
}

impl SubsumptionProgramsMap {
    pub fn new_with_hasher(hash_builder: BankHasherBuilder) -> Self {
        Self {
            map: HashMap::with_hasher(hash_builder),
            set: HashSet::with_hasher(hash_builder),
        }
    }

    fn find_minmal<'a>(
        p: &Arc<SubProgram>,
        existing_progs: &'a mut Vec<Arc<SubProgram>>,
    ) -> MinmalProgResult<'a> {
        for ep in existing_progs {
            if ep.pre_ctx() == p.pre_ctx() {
                if p.size() < ep.size() {
                    return MinmalProgResult::LargerProg(ep);
                } else {
                    return MinmalProgResult::SmallerProg;
                }
            } else if ep.pre_ctx().subset(p.pre_ctx()) {
                return MinmalProgResult::SmallerProg;
            }
        }
        return MinmalProgResult::NonComparable;
    }

    pub(crate) fn insert(&mut self, p: Arc<SubProgram>) -> bool {
        let output: ProgOutput = p.clone().into();
        match self.map.entry(output) {
            Entry::Occupied(mut existing_progs) => {
                for ep in existing_progs.get().iter() {
                    if ep.pre_ctx().subset(p.pre_ctx()) {
                        return false;
                    }
                }
                existing_progs.get_mut().push(p.clone());
                self.set.insert(p);
                true
            }
            Entry::Vacant(vacant_entry) => {
                vacant_entry.insert(vec![p.clone()]);
                self.set.insert(p);
                true
            }
        }
    }

    fn contains(&self, p: &Arc<SubProgram>) -> bool {
        let output: ProgOutput = p.clone().into();
        if let Some(progs) = self.map.get(&output) {
            progs
                .iter()
                .any(|other| other.pre_ctx().subset(p.pre_ctx()))
        } else {
            false
        }
    }

    fn len(&self) -> usize {
        self.set.len()
    }

    fn iter(&self) -> impl Iterator<Item = &Arc<SubProgram>> + Send {
        self.set.iter()
    }

    fn take_minimal_prog(&mut self, mut other: Self) {
        for (out, progs) in other.map {
            match self.map.entry(out) {
                Entry::Occupied(mut existing_progs) => {
                    for p in progs {
                        other.set.remove(&p);
                        match Self::find_minmal(&p, existing_progs.get_mut()) {
                            MinmalProgResult::LargerProg(larger_p) => {
                                self.set.remove(larger_p);
                                *larger_p = p.clone();
                                self.set.insert(p);
                            }
                            MinmalProgResult::SmallerProg => (),
                            MinmalProgResult::NonComparable => {
                                existing_progs.get_mut().push(p.clone());
                                self.set.insert(p);
                            }
                        }
                    }
                }
                Entry::Vacant(vacant_entry) => {
                    self.set.extend(progs.iter().cloned());
                    vacant_entry.insert(progs);
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct SubsumptionTypeMap {
    hash_builder: BankHasherBuilder,
    maps: HashMap<ValueType, SubsumptionProgramsMap, BankHasherBuilder>,
}

impl SubsumptionTypeMap {
    pub fn new_with_hasher(hash_builder: BankHasherBuilder) -> Self {
        Self {
            hash_builder,
            maps: HashMap::with_hasher(hash_builder),
        }
    }

    pub(crate) fn insert_program(&mut self, p: Arc<SubProgram>) -> bool {
        let programs_map = self
            .maps
            .entry(p.out_type().clone())
            .or_insert(SubsumptionProgramsMap::new_with_hasher(self.hash_builder));
        programs_map.insert(p)
    }

    pub(crate) fn contains(&self, p: &Arc<SubProgram>) -> bool {
        match self.maps.get(p.out_type()) {
            None => false,
            Some(values) => values.contains(p),
        }
    }

    pub(crate) fn get(&self, value_type: &ValueType) -> Option<&SubsumptionProgramsMap> {
        self.maps.get(value_type)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&ValueType, &SubsumptionProgramsMap)> {
        self.maps.iter()
    }

    fn len(&self) -> usize {
        self.maps.iter().map(|(_, map)| map.len()).sum()
    }
}

impl BatchBuilder for SubsumptionTypeMap {
    async fn add_program(&mut self, p: &Arc<SubProgram>) -> bool {
        self.insert_program(p.clone())
    }
}

impl BankIterationBuilder for SubsumptionTypeMap {
    type BatchBuilderType = SubsumptionTypeMap;

    fn create_batch_builder(&self) -> Self::BatchBuilderType {
        SubsumptionTypeMap::new_with_hasher(self.hash_builder)
    }

    async fn add_batch(&mut self, batch: Self::BatchBuilderType) {
        for (value_type, programs_map) in batch.maps.into_iter() {
            if let Some(cur_map) = self.maps.get_mut(&value_type) {
                cur_map.take_minimal_prog(programs_map);
            } else {
                self.maps.insert(value_type, programs_map);
            }
        }
    }

    async fn iter_programs(&self) -> impl Iterator<Item = &Arc<SubProgram>> {
        self.iter().flat_map(|(_, map)| map.iter())
    }
}

#[derive(Default)]
pub struct SubsumptionProgBank {
    hash_builder: BankHasherBuilder,
    iterations: Vec<SubsumptionTypeMap>,
}

impl ProgBank for SubsumptionProgBank {
    type IterationBuilderType = SubsumptionTypeMap;
    type BankConfigType = SubsumptionBankConfig;

    async fn new_with_config(config: Self::BankConfigType) -> Self {
        Self {
            hash_builder: config.hash_builder,
            iterations: Vec::new(),
        }
    }

    async fn output_exists(&self, p: &Arc<SubProgram>) -> bool {
        self.iterations.iter().any(|i| i.contains(p))
    }

    fn iteration_count(&self) -> usize {
        self.iterations.len()
    }

    fn total_number_of_programs(&self) -> usize {
        self.iterations.iter().map(|i| i.len()).sum()
    }

    async fn number_of_programs(
        &self,
        iteration: usize,
        output_type: &ruse_object_graph::ValueType,
    ) -> usize {
        match self.iterations[iteration].get(output_type) {
            None => 0,
            Some(map) => map.len(),
        }
    }

    async fn iter_programs<'a, 'b>(
        &'a self,
        iteration: usize,
        output_type: &'b ruse_object_graph::ValueType,
    ) -> impl Iterator<Item = &'a Arc<SubProgram>> + Send + 'a {
        let either: Either<_, _> = match self
            .iterations
            .get(iteration)
            .and_then(|i| i.get(output_type))
        {
            None => Either::Left(std::iter::empty::<&Arc<SubProgram>>()),
            Some(map) => Either::Right(map.iter()),
        };

        either.into_iter()
    }

    fn create_iteration_builder(&self) -> SubsumptionTypeMap {
        SubsumptionTypeMap::new_with_hasher(self.hash_builder)
    }

    async fn end_iteration(&mut self, iteration: Self::IterationBuilderType) {
        self.iterations.push(iteration);
    }
}
