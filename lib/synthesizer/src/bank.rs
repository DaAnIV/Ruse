use std::{
    hash::{BuildHasherDefault, DefaultHasher, Hash},
    sync::Arc,
};

use dashmap::{iter_set, DashMap, DashSet, Entry};
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

pub(crate) type ProgramsMapIter<'a> = iter_set::Iter<
    'a,
    Arc<SubProgram>,
    BankHasherBuilder,
    DashMap<Arc<SubProgram>, (), BankHasherBuilder>,
>;

// The bank is hierarchical
// iteration -> out_type -> sub_prog

#[derive(Debug, Default)]
pub(crate) struct ProgramsMap {
    map: DashMap<ProgOutput, Vec<Arc<SubProgram>>, BankHasherBuilder>,
    set: DashSet<Arc<SubProgram>, BankHasherBuilder>,
}

impl ProgramsMap {
    fn insert(&self, p: Arc<SubProgram>) -> bool {
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

    fn iter(&self) -> ProgramsMapIter<'_> {
        self.set.iter()
    }

    fn extend(&mut self, other: Self) {
        self.map.extend(other.map.into_iter());
        self.set.extend(other.set.into_iter());
    }
}

impl<'a> IntoIterator for &'a ProgramsMap {
    type Item = <ProgramsMapIter<'a> as Iterator>::Item;

    type IntoIter = ProgramsMapIter<'a>;

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

    pub(crate) fn get(
        &self,
        value_type: &ValueType,
    ) -> Option<dashmap::mapref::one::Ref<ValueType, ProgramsMap>> {
        self.0.get(value_type)
    }

    pub(crate) fn extend(&self, x: Self) {
        for (value_type, programs_map) in x.0.into_iter() {
            if let Some(mut cur_map) = self.0.get_mut(&value_type) {
                cur_map.extend(programs_map);
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

    pub fn number_of_programs(&self, iteration: usize, output_type: &ValueType) -> usize {
        match self[iteration].get(output_type) {
            Some(map) => map.len(),
            None => 0,
        }

    }
}

impl std::ops::Index<usize> for ProgBank {
    type Output = Arc<TypeMap>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}
