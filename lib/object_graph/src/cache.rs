use std::{collections::HashSet, sync::Arc};

pub struct Cache {
    strings: HashSet<Arc<String>>,
}

impl Cache {
    pub fn new() -> Self {
        Cache {
            strings: Default::default(),
        }
    }

    pub fn get_or_insert_str(&mut self, str: &str) -> Arc<String> {
        self.get_or_insert_string(str.to_string())
    }

    pub fn get_or_insert_string(&mut self, str: String) -> Arc<String> {
        if !self.strings.contains(&str) {
            self.strings.insert(str.clone().into());
        }
        self.strings.get(&str).unwrap().clone()
    }

    pub fn clear_cache(&mut self) {
        self.strings.clear();
    }
}

#[macro_export]
macro_rules! str_cached {
    ($cache:expr; $x:expr) => {
        $cache.get_or_insert_str($x)
    };
}

#[macro_export]
macro_rules! scached {
    ($cache:expr; $x:expr) => {
        $cache.get_or_insert_string($x)
    };
}
