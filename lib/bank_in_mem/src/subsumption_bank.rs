use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    hash::Hash,
    sync::Arc,
};

use itertools::Either;
use ruse_object_graph::ValueType;

use ruse_synthesizer::{
    bank::*,
    bank_hasher::BankHasherBuilder,
    context::{ContextArray, ContextSubsetResult},
    prog::SubProgram,
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
        if self.0.num_mutations() > 0 {
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

fn subsumption_partial_cmp(a: &SubProgram, b: &SubProgram) -> Option<std::cmp::Ordering> {
    match a.pre_ctx().subset(b.pre_ctx()) {
        ContextSubsetResult::Subset => Some(std::cmp::Ordering::Less),
        ContextSubsetResult::Equal => Some(a.size().cmp(&b.size())),
        ContextSubsetResult::NotSubset => None,
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
    set: Vec<HashSet<Arc<SubProgram>, BankHasherBuilder>>,
}

impl SubsumptionProgramsMap {
    pub fn new_with_hasher(hash_builder: BankHasherBuilder) -> Self {
        Self {
            map: HashMap::with_hasher(hash_builder),
            set: Default::default(),
        }
    }

    fn get_set_for_prog(
        &mut self,
        p: &Arc<SubProgram>,
    ) -> &mut HashSet<Arc<SubProgram>, BankHasherBuilder> {
        let size = p.size() as usize;
        self.set.resize_with(self.set.len().max(size), || {
            HashSet::with_hasher(self.map.hasher().clone())
        });

        &mut self.set[size - 1]
    }

    pub(crate) fn insert(&mut self, p: Arc<SubProgram>) -> bool {
        let output: ProgOutput = p.clone().into();
        let mut grater_ep = None;
        match self.map.entry(output) {
            Entry::Occupied(mut existing_progs) => {
                let mut grater_ep_i = None;
                for (i, ep) in existing_progs.get().iter().enumerate() {
                    match subsumption_partial_cmp(ep, &p) {
                        Some(std::cmp::Ordering::Less) => return false,
                        Some(std::cmp::Ordering::Equal) => return false,
                        Some(std::cmp::Ordering::Greater) => {
                            // Same pre context, but p is smaller in AST size
                            // So we replace the larger program with the smaller one
                            grater_ep_i = Some(i);
                            break;
                        }
                        None => (),
                    }
                }
                if let Some(grater_ep_i) = grater_ep_i {
                    grater_ep = Some(std::mem::replace(
                        &mut existing_progs.get_mut()[grater_ep_i],
                        p.clone(),
                    ))
                } else {
                    existing_progs.get_mut().push(p.clone());
                }
            }
            Entry::Vacant(vacant_entry) => {
                vacant_entry.insert(vec![p.clone()]);
            }
        };

        if let Some(grater_ep) = grater_ep {
            self.get_set_for_prog(&grater_ep).remove(&grater_ep);
        }
        self.get_set_for_prog(&p).insert(p);
        true
    }

    pub fn extend(&mut self, other: Self) {
        for prog in other.into_iter() {
            self.insert(prog);
        }
    }

    fn contains(&self, p: &Arc<SubProgram>) -> bool {
        let output: ProgOutput = p.clone().into();
        if let Some(progs) = self.map.get(&output) {
            progs
                .iter()
                .any(|other| other.pre_ctx().subset(p.pre_ctx()) != ContextSubsetResult::NotSubset)
        } else {
            false
        }
    }

    fn len(&self) -> usize {
        self.set.iter().map(|set| set.len()).sum()
    }

    fn iter(&self) -> impl Iterator<Item = &Arc<SubProgram>> + Send {
        self.set.iter().flat_map(|set| set.iter())
    }

    fn into_iter(self) -> impl Iterator<Item = Arc<SubProgram>> {
        self.set.into_iter().flat_map(|set| set.into_iter())
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
                cur_map.extend(programs_map);
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
