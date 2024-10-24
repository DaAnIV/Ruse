use dashmap::DashMap;
use std::sync::{atomic, Arc};

pub type CachedString = Arc<String>;

pub struct Cache {
    strings: DashMap<String, CachedString>,
    output_root_name: CachedString,
}

static TEMP: atomic::AtomicU64 = atomic::AtomicU64::new(0);

impl Cache {
    pub fn new() -> Self {
        let strings = DashMap::default();
        let output_root_name = Arc::new("____output_root_name".to_string());

        strings.insert(output_root_name.to_string(), output_root_name.clone());

        let instance = Self {
            strings,
            output_root_name,
        };

        instance
    }

    pub fn temp_string(&self) -> CachedString {
        let val = TEMP.fetch_add(1, atomic::Ordering::Relaxed);
        let str = format!("____temp_str_{val}");
        self.get_or_insert_string(str)
    }

    pub fn output_root_name(&self) -> &CachedString {
        &self.output_root_name
    }

    pub fn get_or_insert_str(&self, str: &str) -> CachedString {
        self.get_or_insert_string(str.to_string())
    }

    pub fn get_or_insert_string(&self, str: String) -> CachedString {
        self.strings.entry(str.clone()).or_insert(str.into()).value().clone()
    }

    pub fn clear_cache(&self) {
        self.strings.clear();
    }
}

impl Default for Cache {
    fn default() -> Self {
        Self::new()
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
