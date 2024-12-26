use std::{
    hash::{BuildHasherDefault, DefaultHasher, Hash},
    sync::Arc,
};

use dashmap::{
    mapref::{entry::Entry, multiple::RefMulti},
    DashMap,
};
use ruse_object_graph::value::ValueType;
use tracing::info;

use crate::{context::ContextArray, prog::SubProgram, value_array::ValueArray};

pub type BankHasherBuilder = BuildHasherDefault<DefaultHasher>;

#[derive(Debug, Clone)]
pub(crate) struct ProgOutput(Arc<SubProgram>);

impl ProgOutput {
    fn out_type(&self) -> &ValueType {
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
                .eq(self.post_ctx(), other.out_value(), other.post_ctx())
            && self.pre_ctx().subset(other.pre_ctx())
            && self.post_ctx().subset(other.post_ctx())
    }
}

impl Hash for ProgOutput {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.out_type().hash(state);
        self.0.out_value().wrap(self.post_ctx()).hash(state);
    }
}

// The bank is hierarchical
// iteration -> out_type -> sub_prog

#[derive(Debug, Default)]
pub(crate) struct ProgramsMap(pub DashMap<ProgOutput, Arc<SubProgram>, BankHasherBuilder>);

impl ProgramsMap {
    fn insert(&self, p: Arc<SubProgram>) -> bool {
        let output: ProgOutput = p.clone().into();
        match self.0.entry(output) {
            Entry::Occupied(_) => false,
            Entry::Vacant(vacant_entry) => {
                vacant_entry.insert(p);
                true
            }
        }
    }

    fn contains(&self, p: &Arc<SubProgram>) -> bool {
        let output: ProgOutput = p.clone().into();
        self.0.contains_key(&output)
    }

    fn get_inserted_prog(&self, p: &Arc<SubProgram>) -> Option<Arc<SubProgram>> {
        let output: ProgOutput = p.clone().into();
        self.0.get(&output).map(|p| p.clone())
    }

    pub fn iter(&self) -> dashmap::iter::Iter<ProgOutput, Arc<SubProgram>, BankHasherBuilder> {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a ProgramsMap {
    type Item = RefMulti<'a, ProgOutput, Arc<SubProgram>>;

    type IntoIter = dashmap::iter::Iter<'a, ProgOutput, Arc<SubProgram>, BankHasherBuilder>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[derive(Default, Debug)]
pub struct TypeMap(DashMap<ValueType, ProgramsMap, BankHasherBuilder>);

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
    
    pub(crate) fn extend(&self, x: TypeMap) {
        for (value_type, programs_map) in x.0.into_iter() {
            if let Some(mut cur_map) = self.0.get_mut(&value_type) {
                cur_map.0.extend(programs_map.0.into_iter());
            } else {
                self.0.insert(value_type, programs_map);
            }
        }
    }

    pub(crate) fn iter(&self) -> dashmap::iter::Iter<ValueType, ProgramsMap, BankHasherBuilder> {
        self.0.iter()
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
