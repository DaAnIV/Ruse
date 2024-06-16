use std::sync::Arc;

use dashmap::{DashMap, Map, SharedValue};

use std::hash::Hash;

use crate::{
    prog::SubProgram,
    value::{LocValue, ValueType},
};

pub type ValueArray = Arc<Vec<LocValue>>;

#[derive(Debug, Clone)]
pub(crate) struct Output(Arc<SubProgram>);

impl From<Arc<SubProgram>> for Output {
    fn from(value: Arc<SubProgram>) -> Self {
        Self(value)
    }
}

impl Eq for Output {}

impl PartialEq for Output {
    fn eq(&self, other: &Self) -> bool {
        self.0.out_type() == other.0.out_type()
            && self.0.out_value() == other.0.out_value()
            && self.0.pre_ctx().contained(other.0.pre_ctx())
            && self.0.post_ctx().contained(other.0.post_ctx())
    }
}

impl Hash for Output {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.out_type().hash(state);
        self.0.out_value().hash(state);
    }
}

// The bank is hierarchical
// iteration -> out_type -> sub_prog

#[derive(Debug)]
pub(crate) struct ProgramsMap(DashMap<Output, Arc<SubProgram>>);

impl ProgramsMap {
    fn new() -> Self {
        Self(DashMap::<Output, Arc<SubProgram>>::new())
    }

    fn insert(&self, p: Arc<SubProgram>) -> bool {
        let output: Output = p.clone().into();
        let idx = self.0.determine_map(&output);
        let mut shard = unsafe { self.0._yield_write_shard(idx) };
        if shard.get_key_value(&output).is_some() {
            false
        } else {
            shard.insert(output, SharedValue::new(p));
            true
        }
    }

    fn contains(&self, p: &Arc<SubProgram>) -> bool {
        let output: Output = p.clone().into();
        self.0.contains_key(&output)
    }

    pub fn iter(&self) -> dashmap::iter::Iter<Output, Arc<SubProgram>> {
        self.0.iter()
    }
}

#[derive(Default, Debug)]
pub struct TypeMap(DashMap<ValueType, Arc<ProgramsMap>>);

impl TypeMap {
    pub(crate) fn insert_program(&self, p: Arc<SubProgram>) -> bool {
        let programs_map = self.get_or_insert_programs_map(&p.out_type());
        programs_map.insert(p)
    }

    fn get_or_insert_programs_map(&self, value_type: &ValueType) -> Arc<ProgramsMap> {
        let idx = self.0.determine_map(value_type);
        let mut shard = unsafe { self.0._yield_write_shard(idx) };
        if let Some((_, vptr)) = shard.get_key_value(value_type) {
            vptr.get().clone()
        } else {
            let m = Arc::new(ProgramsMap::new());
            shard.insert(value_type.clone(), SharedValue::new(m.clone()));
            m
        }
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
    ) -> Option<dashmap::mapref::one::Ref<ValueType, Arc<ProgramsMap>>> {
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
            println!("Iteration {}", i);
            for values in type_map.0.iter() {
                for p in values.value().iter() {
                    println!("{}", p.value())
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
