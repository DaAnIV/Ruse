use ruse_object_graph::ValueType;
use std::{future::Future, sync::Arc};

use crate::prog::SubProgram;

pub trait BatchBuilder: Send + Sync {
    fn add_program(&mut self, p: &Arc<SubProgram>) -> impl Future<Output = bool> + Send;
}

pub trait BankIterationBuilder: Send + Sync {
    type BatchBuilderType: BatchBuilder;

    fn create_batch_builder(&self) -> Self::BatchBuilderType;
    fn add_batch(&mut self, batch: Self::BatchBuilderType) -> impl Future<Output = ()> + Send;
    fn iter_programs(&self) -> impl Future<Output = impl Iterator<Item = &Arc<SubProgram>> + Send> + Send;
}

pub trait BankConfig: Default + std::fmt::Debug + Clone {}

pub trait ProgBank: Send + Sync + Sized {
    type IterationBuilderType: BankIterationBuilder;
    type BankConfigType: BankConfig;

    fn new_with_config(config: Self::BankConfigType) -> impl Future<Output = Self> + Send;
    fn new() -> impl Future<Output = Self> + Send {
        Self::new_with_config(Default::default())
    }

    fn output_exists(&self, p: &Arc<SubProgram>) -> impl Future<Output = bool> + Send;
    fn iteration_count(&self) -> usize;
    fn total_number_of_programs(&self) -> usize;
    fn number_of_programs(&self, iteration: usize, output_type: &ValueType) -> impl Future<Output = usize> + Send;

    fn iter_programs<'a, 'b>(
        &'a self,
        iteration: usize,
        output_type: &'b ValueType,
    ) -> impl Future<Output = impl Iterator<Item = &'a Arc<SubProgram>> + Send + 'a> + Send;

    fn create_iteration_builder(&self) -> Self::IterationBuilderType;
    fn end_iteration(&mut self, iteration: Self::IterationBuilderType) -> impl Future<Output = ()> + Send;
}
