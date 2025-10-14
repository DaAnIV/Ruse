use std::{hash::RandomState, sync::Arc};

use indexmap::{map::Entry, IndexMap};

use ruse_synthesizer::{context::ContextSubsetResult, prog::SubProgram};

use crate::{
    subprogram_key::{subsumption_partial_cmp, SubProgramKey},
    triples_set::TripleSet,
};

#[derive(Debug)]
pub struct SubsumptionProgramsMap {
    map: IndexMap<SubProgramKey, Vec<Arc<SubProgram>>>,
    set: TripleSet,
}

impl SubsumptionProgramsMap {
    pub fn with_hasher(hash_builder: RandomState) -> Self {
        Self {
            map: IndexMap::with_hasher(hash_builder.clone()),
            set: TripleSet::with_hasher(hash_builder.clone()),
        }
    }

    pub(crate) fn insert(&mut self, p: Arc<SubProgram>) -> bool {
        let key: SubProgramKey = p.clone().into();
        let mut grater_ep = None;
        match self.map.entry(key) {
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
            self.set.remove(&grater_ep);
        }
        self.set.insert(p);
        true
    }

    pub(crate) fn extend(&mut self, other: Self) {
        for prog in other.into_iter() {
            self.insert(prog);
        }
    }

    pub(crate) fn contains(&self, p: &Arc<SubProgram>) -> bool {
        let output: SubProgramKey = p.clone().into();
        if let Some(progs) = self.map.get(&output) {
            progs
                .iter()
                .any(|other| other.pre_ctx().subset(p.pre_ctx()) != ContextSubsetResult::NotSubset)
        } else {
            false
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.set.len()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &Arc<SubProgram>> + Send {
        self.set.iter()
    }

    pub(crate) fn into_iter(self) -> impl Iterator<Item = Arc<SubProgram>> {
        self.set.into_iter()
    }
}
