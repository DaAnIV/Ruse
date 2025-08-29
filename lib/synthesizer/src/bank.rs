use ruse_object_graph::ValueType;
use std::{collections::HashMap, fmt, hash::Hash, sync::Arc};
use tracing::info;

use crate::{
    bank_hasher::BankHasherBuilder, context::ContextArray, prog::SubProgram,
    value_array::ValueArray,
};

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

pub trait ProgramsMap: Send + Sync + fmt::Debug {
    fn new_with_hasher(hash_builder: BankHasherBuilder) -> Self;

    fn insert(&mut self, p: Arc<SubProgram>) -> bool;
    fn contains(&self, p: &Arc<SubProgram>) -> bool;
    fn iter(&self) -> impl Iterator<Item = &Arc<SubProgram>> + Send;
    fn take_minimal_prog(&mut self, other: Self);
    fn len(&self) -> usize;
}

#[derive(Debug)]
pub struct TypeMap<T: ProgramsMap> {
    hash_builder: BankHasherBuilder,
    maps: HashMap<ValueType, T, BankHasherBuilder>,
}

impl<T: ProgramsMap> TypeMap<T> {
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
            .or_insert(T::new_with_hasher(self.hash_builder));
        programs_map.insert(p)
    }

    pub(crate) fn contains(&self, p: &Arc<SubProgram>) -> bool {
        match self.maps.get(p.out_type()) {
            None => false,
            Some(values) => values.contains(p),
        }
    }

    pub(crate) fn get(&self, value_type: &ValueType) -> Option<&T> {
        self.maps.get(value_type)
    }

    pub(crate) fn take_minimal_prog(&mut self, x: Self) {
        for (value_type, programs_map) in x.maps.into_iter() {
            if let Some(cur_map) = self.maps.get_mut(&value_type) {
                cur_map.take_minimal_prog(programs_map);
            } else {
                self.maps.insert(value_type, programs_map);
            }
        }
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&ValueType, &T)> {
        self.maps.iter()
    }
}

pub trait ProgBank: Default + Send + Sync {
    type T: ProgramsMap + 'static;

    fn new_with_hasher(hash_builder: BankHasherBuilder) -> Self;
    fn new_type_map(&self) -> TypeMap<Self::T>;

    fn iterations(&self) -> &Vec<TypeMap<Self::T>>;
    fn mut_iterations(&mut self) -> &mut Vec<TypeMap<Self::T>>;

    fn iteration(&self, i: usize) -> &TypeMap<Self::T> {
        &self.iterations()[i]
    }

    fn output_exists(&self, p: &Arc<SubProgram>) -> bool {
        self.iterations()
            .iter()
            .any(|type_map| type_map.contains(p))
    }

    #[inline]
    fn insert(&mut self, type_map: TypeMap<Self::T>) {
        self.mut_iterations().push(type_map);
    }

    #[inline]
    fn iteration_count(&self) -> usize {
        self.iterations().len()
    }

    fn print_all_programs(&self) {
        for (i, type_map) in self.iterations().iter().enumerate() {
            info!(target: "ruse::synthesizer", "Iteration {}", i);
            for (_, maps) in type_map.maps.iter() {
                for p in maps.iter() {
                    info!(target: "ruse::synthesizer", "");
                    info!(target: "ruse::synthesizer", "{}", *p);
                }
            }
        }
    }

    fn number_of_programs(&self, iteration: usize, output_type: &ValueType) -> usize {
        match self.iteration(iteration).get(output_type) {
            Some(map) => map.len(),
            None => 0,
        }
    }
}
