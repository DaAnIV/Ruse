use dashmap::DashSet;
use std::sync::{atomic, Arc};

pub type CachedString = Arc<String>;

pub struct Cache {
    strings: DashSet<CachedString>,
}

static TEMP: atomic::AtomicU64 = atomic::AtomicU64::new(0);

impl Cache {
    pub fn new() -> Self {
        Cache {
            strings: Default::default(),
        }
    }

    pub fn temp_string(&self) -> CachedString {
        let val = TEMP.fetch_add(1, atomic::Ordering::Relaxed);
        let str = format!("____temp_str_{val}");
        self.get_or_insert_string(str)
    }

    pub fn get_or_insert_str(&self, str: &str) -> CachedString {
        self.get_or_insert_string(str.to_string())
    }

    pub fn get_or_insert_string(&self, str: String) -> CachedString {
        if !self.strings.contains(&str) {
            self.strings.insert(str.clone().into());
        }
        self.strings.get(&str).unwrap().clone()
    }

    pub fn clear_cache(&self) {
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
