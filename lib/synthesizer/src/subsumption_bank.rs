use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    sync::Arc,
};

use crate::{
    bank::{ProgBank, ProgOutput, ProgramsMap, TypeMap},
    bank_hasher::BankHasherBuilder,
    prog::SubProgram,
};

// The bank is hierarchical
// iteration -> out_type -> sub_prog

#[derive(Debug)]
pub struct SubsumptionProgramsMap {
    map: HashMap<ProgOutput, Vec<Arc<SubProgram>>, BankHasherBuilder>,
    set: HashSet<Arc<SubProgram>, BankHasherBuilder>,
}

enum MinmalProgResult<'a> {
    LargerProg(&'a mut Arc<SubProgram>),
    SmallerProg,
    NonComparable,
}

impl SubsumptionProgramsMap {
    fn find_minmal<'a>(
        p: &Arc<SubProgram>,
        existing_progs: &'a mut Vec<Arc<SubProgram>>,
    ) -> MinmalProgResult<'a> {
        for ep in existing_progs {
            if ep.pre_ctx() == p.pre_ctx() {
                if p.size() < ep.size() {
                    return MinmalProgResult::LargerProg(ep);
                } else {
                    return MinmalProgResult::SmallerProg;
                }
            } else if ep.pre_ctx().subset(p.pre_ctx()) {
                return MinmalProgResult::SmallerProg;
            }
        }
        return MinmalProgResult::NonComparable;
    }
}

impl ProgramsMap for SubsumptionProgramsMap {
    fn new_with_hasher(hash_builder: BankHasherBuilder) -> Self {
        Self {
            map: HashMap::with_hasher(hash_builder),
            set: HashSet::with_hasher(hash_builder),
        }
    }

    fn insert(&mut self, p: Arc<SubProgram>) -> bool {
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

    fn iter(&self) -> impl Iterator<Item = &Arc<SubProgram>> + Send {
        self.set.iter()
    }

    fn take_minimal_prog(&mut self, mut other: Self) {
        for (out, progs) in other.map {
            match self.map.entry(out) {
                Entry::Occupied(mut existing_progs) => {
                    for p in progs {
                        other.set.remove(&p);
                        match Self::find_minmal(&p, existing_progs.get_mut()) {
                            MinmalProgResult::LargerProg(larger_p) => {
                                self.set.remove(larger_p);
                                *larger_p = p.clone();
                                self.set.insert(p);
                            }
                            MinmalProgResult::SmallerProg => (),
                            MinmalProgResult::NonComparable => {
                                existing_progs.get_mut().push(p.clone());
                                self.set.insert(p);
                            }
                        }
                    }
                }
                Entry::Vacant(vacant_entry) => {
                    self.set.extend(progs.iter().cloned());
                    vacant_entry.insert(progs);
                }
            }
        }
    }
}

#[derive(Default)]
pub struct SubsumptionProgBank {
    hash_builder: BankHasherBuilder,
    iterations: Vec<TypeMap<SubsumptionProgramsMap>>,
}

impl ProgBank for SubsumptionProgBank {
    type T = SubsumptionProgramsMap;

    fn new_with_hasher(hash_builder: BankHasherBuilder) -> Self {
        Self {
            hash_builder: hash_builder,
            iterations: Default::default(),
        }
    }
    fn new_type_map(&self) -> TypeMap<Self::T> {
        TypeMap::new_with_hasher(self.hash_builder)
    }

    fn iterations(&self) -> &Vec<TypeMap<Self::T>> {
        &self.iterations
    }

    fn mut_iterations(&mut self) -> &mut Vec<TypeMap<Self::T>> {
        &mut self.iterations
    }
}
