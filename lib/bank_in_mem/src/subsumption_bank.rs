use std::{hash::RandomState, sync::Arc};

use itertools::Either;

use ruse_synthesizer::{bank::*, prog::SubProgram};

use crate::{
    config::SubsumptionBankConfig,
    subsumption_type_map::SubsumptionTypeMap,
};

// The bank is hierarchical
// iteration -> out_type -> sub_prog

#[derive(Default)]
pub struct SubsumptionProgBank {
    iterations: Vec<SubsumptionTypeMap>,
    hash_builder: RandomState,
}

impl ProgBank for SubsumptionProgBank {
    type IterationBuilderType = SubsumptionTypeMap;
    type BankConfigType = SubsumptionBankConfig;

    async fn new_with_config(_config: Self::BankConfigType) -> Self {
        Self {
            hash_builder: RandomState::new(),
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
        SubsumptionTypeMap::with_hasher(self.hash_builder.clone())
    }

    async fn end_iteration(&mut self, iteration: Self::IterationBuilderType) {
        self.iterations.push(iteration);
    }
}
