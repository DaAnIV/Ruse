use std::{hash::RandomState, sync::Arc};

use indexmap::IndexMap;
use ruse_object_graph::ValueType;

use ruse_synthesizer::{bank::*, prog::SubProgram};

use crate::subsumption_programs_map::SubsumptionProgramsMap;

#[derive(Debug)]
pub struct SubsumptionTypeMap {
    maps: IndexMap<ValueType, SubsumptionProgramsMap>,
}

impl SubsumptionTypeMap {
    pub fn with_hasher(hash_builder: RandomState) -> Self {
        Self {
            maps: IndexMap::with_hasher(hash_builder.clone()),
        }
    }

    pub(crate) fn insert_program(&mut self, p: Arc<SubProgram>) -> bool {
        let hash_builder = self.maps.hasher().clone();
        let programs_map = self
            .maps
            .entry(p.out_type().clone())
            .or_insert(SubsumptionProgramsMap::with_hasher(hash_builder));
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

    pub(crate) fn len(&self) -> usize {
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
        SubsumptionTypeMap::with_hasher(self.maps.hasher().clone())
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
