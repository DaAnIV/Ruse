use std::{
    hash::{BuildHasher, DefaultHasher, Hash},
    sync::Arc,
};

use dashmap::{
    mapref::{entry::Entry, multiple::RefMulti},
    DashMap,
};
use ruse_object_graph::value::ValueType;
use tracing::info;

use crate::{context::ContextArray, prog::SubProgram, value_array::ValueArray};

#[derive(Clone)]
pub struct BankRandomState {}

impl BankRandomState {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for BankRandomState {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildHasher for BankRandomState {
    type Hasher = DefaultHasher;
    #[inline]
    fn build_hasher(&self) -> DefaultHasher {
        // This always produces the same hasher
        // We are not afraid of DDOS attacks
        DefaultHasher::default()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Output(Arc<SubProgram>);

impl Output {
    fn out_type(&self) -> ValueType {
        self.0.out_type()
    }
    fn out_value(&self) -> &ValueArray {
        self.0.out_value()
    }
    fn pre_ctx(&self) -> &ContextArray {
        self.0.pre_ctx()
    }
    fn post_ctx(&self) -> &ContextArray {
        self.0.post_ctx()
    }
}

impl From<Arc<SubProgram>> for Output {
    fn from(value: Arc<SubProgram>) -> Self {
        Self(value)
    }
}

impl Eq for Output {}

impl PartialEq for Output {
    fn eq(&self, other: &Self) -> bool {
        self.out_type() == other.out_type()
            && self
                .out_value()
                .eq(self.post_ctx(), other.out_value(), other.post_ctx())
            && self.pre_ctx().subset(other.pre_ctx())
            && self.post_ctx().subset(other.post_ctx())
    }
}

impl Hash for Output {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.out_type().hash(state);
        self.0.out_value().wrap(self.post_ctx()).hash(state);
    }
}

// The bank is hierarchical
// iteration -> out_type -> sub_prog

#[derive(Debug, Default)]
pub(crate) struct ProgramsMap(pub DashMap<Output, Arc<SubProgram>, BankRandomState>);

impl ProgramsMap {
    fn insert(&self, p: Arc<SubProgram>) -> bool {
        let output: Output = p.clone().into();
        match self.0.entry(output) {
            Entry::Occupied(_) => false,
            Entry::Vacant(vacant_entry) => {
                vacant_entry.insert(p);
                true
            }
        }
    }

    fn contains(&self, p: &Arc<SubProgram>) -> bool {
        let output: Output = p.clone().into();
        self.0.contains_key(&output)
    }

    fn get_inserted_prog(&self, p: &Arc<SubProgram>) -> Option<Arc<SubProgram>> {
        let output: Output = p.clone().into();
        self.0.get(&output).map(|p| p.clone())
    }

    pub fn iter(&self) -> dashmap::iter::Iter<Output, Arc<SubProgram>, BankRandomState> {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a ProgramsMap {
    type Item = RefMulti<'a, Output, Arc<SubProgram>>;

    type IntoIter = dashmap::iter::Iter<'a, Output, Arc<SubProgram>, BankRandomState>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[derive(Default, Debug)]
pub struct TypeMap(DashMap<ValueType, ProgramsMap, BankRandomState>);

impl TypeMap {
    pub(crate) fn insert_program(&self, p: Arc<SubProgram>) -> bool {
        let mut binding = self.0.entry(p.out_type().clone()).or_default();
        let programs_map = binding.value_mut();
        programs_map.insert(p)
    }

    pub(crate) fn contains(&self, p: &Arc<SubProgram>) -> bool {
        match self.0.get(&p.out_type()) {
            None => false,
            Some(values) => values.contains(p),
        }
    }

    pub(crate) fn get_inserted_prog(&self, p: &Arc<SubProgram>) -> Option<Arc<SubProgram>> {
        match self.0.get(&p.out_type()) {
            None => None,
            Some(values) => values.get_inserted_prog(p),
        }
    }

    pub(crate) fn get(
        &self,
        value_type: &ValueType,
    ) -> Option<dashmap::mapref::one::Ref<ValueType, ProgramsMap>> {
        self.0.get(value_type)
    }
}

#[derive(Default)]
pub struct ProgBank(Vec<Arc<TypeMap>>);

impl ProgBank {
    pub fn output_exists(&self, p: &Arc<SubProgram>) -> bool {
        self.0.iter().any(|type_map| type_map.contains(p))
    }

    #[inline]
    pub(crate) fn insert(&mut self, type_map: Arc<TypeMap>) {
        self.0.push(type_map);
    }

    #[inline]
    pub fn iteration_count(&self) -> usize {
        self.0.len()
    }

    pub fn print_all_programs(&self) {
        for (i, type_map) in self.0.iter().enumerate() {
            info!(target: "ruse::synthesizer", "Iteration {}", i);
            for values in type_map.0.iter() {
                for p in values.value().iter() {
                    info!(target: "ruse::synthesizer", "");
                    info!(target: "ruse::synthesizer", "{}", p.value());
                }
            }
        }
    }
}

impl std::ops::Index<usize> for ProgBank {
    type Output = Arc<TypeMap>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}
