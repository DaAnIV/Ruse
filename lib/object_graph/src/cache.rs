use dashmap::DashMap;
use std::sync::Arc;

pub type CachedString = Arc<String>;

pub struct Cache {
    strings: DashMap<String, CachedString>,
    output_root_name: CachedString,
}

impl Cache {
    pub const OUTPUT_ROOT_NAME: &str = "____output_root_name";

    pub fn new() -> Self {
        let strings = DashMap::default();
        let output_root_name = Arc::new(Self::OUTPUT_ROOT_NAME.to_string());

        strings.insert(output_root_name.to_string(), output_root_name.clone());

        let instance = Self {
            strings,
            output_root_name,
        };

        instance
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
