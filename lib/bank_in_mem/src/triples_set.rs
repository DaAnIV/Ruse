use indexmap::IndexSet;
use ruse_synthesizer::prog::SubProgram;
use std::{hash::RandomState, sync::Arc};

#[derive(Debug)]
pub(crate) struct TripleSet {
    sets: Vec<IndexSet<Arc<SubProgram>>>,
    hash_builder: RandomState,
}

impl TripleSet {
    pub fn with_hasher(hash_builder: RandomState) -> Self {
        Self {
            sets: Vec::new(),
            hash_builder,
        }
    }

    pub fn len(&self) -> usize {
        self.sets.iter().map(|set| set.len()).sum()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<SubProgram>> + Send {
        self.sets.iter().flat_map(|set| set.iter())
    }

    pub fn into_iter(self) -> impl Iterator<Item = Arc<SubProgram>> {
        self.sets.into_iter().flat_map(|set| set.into_iter())
    }

    pub fn get_set_mut(&mut self, p: &Arc<SubProgram>) -> Option<&mut IndexSet<Arc<SubProgram>>> {
        self.sets.get_mut(p.size() as usize - 1)
    }

    pub fn get_or_create_set_for_prog(
        &mut self,
        p: &Arc<SubProgram>,
    ) -> &mut IndexSet<Arc<SubProgram>> {
        let size = p.size() as usize;
        self.sets.resize_with(self.sets.len().max(size), || {
            IndexSet::with_hasher(self.hash_builder.clone())
        });

        &mut self.sets[size - 1]
    }

    pub fn insert(&mut self, p: Arc<SubProgram>) -> bool {
        self.get_or_create_set_for_prog(&p).insert(p)
    }

    pub fn remove(&mut self, p: &Arc<SubProgram>) -> bool {
        if let Some(set) = self.get_set_mut(p) {
            set.remove(p)
        } else {
            false
        }
    }
}
