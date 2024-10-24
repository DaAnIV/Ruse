use std::{ops::Index, sync::Arc};

use dashmap::{mapref::{entry::Entry, multiple::RefMulti}, DashMap};
use itertools::izip;
use ruse_object_graph::{value::ValueType, graph_map_value::GraphMapWrap};

use std::hash::Hash;

use crate::{context::ContextArray, location::LocValue, prog::SubProgram};

#[derive(Debug)]
pub struct ValueArray(Arc<Vec<LocValue>>);
impl ValueArray {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> impl std::iter::Iterator<Item = &LocValue> {
        self.0.iter()
    }
}

impl From<Vec<LocValue>> for ValueArray {
    fn from(value: Vec<LocValue>) -> Self {
        Self(Arc::new(value))
    }
}

impl Index<usize> for ValueArray {
    type Output = LocValue;

    fn index(&self, index: usize) -> &Self::Output {
        self.0.index(index)
    }
}

impl ValueArray {
    pub fn eq(
        &self,
        self_context_array: &ContextArray,
        other: &Self,
        other_context_array: &ContextArray,
    ) -> bool {
        debug_assert!(
            self.len() == other.len()
                && self.len() == self_context_array.len()
                && other.len() == other_context_array.len()
        );
        izip!(
            self.iter(),
            self_context_array.iter(),
            other.iter(),
            other_context_array.iter()
        )
        .all(|(self_val, self_ctx, other_val, other_ctx)| {
            self_val.wrap(&self_ctx.graphs_map) == other_val.wrap(&other_ctx.graphs_map)
        })
    }
}

impl ValueArray {
    pub fn calculate_hash<H: std::hash::Hasher>(
        &self,
        state: &mut H,
        self_context_array: &ContextArray,
    ) {
        debug_assert!(self.len() == self_context_array.len());
        for (val, ctx) in izip!(self.iter(), self_context_array.iter(),) {
            val.wrap(&ctx.graphs_map).hash(state);
        }
    }
}

// impl ValueArray {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>, self_context_array: &ContextArray) -> std::fmt::Result {
//         debug_assert!(self.len() == self_context_array.len());
//         for (val, ctx) in izip!(self.iter(), self_context_array.iter(),) {
//             write!(f, "{}", val.wrap(&ctx.graphs_map))?;
//         }
//         Ok(())
//     }
// }

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
        self.0.out_value().calculate_hash(state, self.post_ctx());
    }
}

// The bank is hierarchical
// iteration -> out_type -> sub_prog

#[derive(Debug, Default)]
pub(crate) struct ProgramsMap(pub DashMap<Output, Arc<SubProgram>>);

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

    pub fn iter(&self) -> dashmap::iter::Iter<Output, Arc<SubProgram>> {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a ProgramsMap {
    type Item = RefMulti<'a, Output, Arc<SubProgram>>;

    type IntoIter = dashmap::iter::Iter<'a, Output, Arc<SubProgram>>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[derive(Default, Debug)]
pub struct TypeMap(DashMap<ValueType, ProgramsMap>);

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
                    println!("");
                    println!("{}", p.value());
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
