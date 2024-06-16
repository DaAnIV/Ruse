use dashmap::{DashMap, Map, SharedValue};
use std::sync::{atomic, Arc};

pub type CachedString = Arc<String>;

pub struct Cache {
    strings: DashMap<CachedString, CachedString>,
    output_root_name: CachedString,
}

static TEMP: atomic::AtomicU64 = atomic::AtomicU64::new(0);

fn get_or_insert_to_strings_set(
    strings: &DashMap<CachedString, CachedString>,
    string: CachedString,
) -> CachedString {
    let idx = strings.determine_map(&string);
    let mut shard = unsafe { strings._yield_write_shard(idx) };
    let kv = shard.get_key_value(&string);
    match kv {
        Some(v) => v.1.get().clone(),
        None => {
            shard.insert(string.clone(), SharedValue::new(string.clone()));
            shard.get_key_value(&string).unwrap().1.get().clone()
        }
    }
}

impl Cache {
    pub fn new() -> Self {
        let strings = Default::default();
        let output_root_name =
            get_or_insert_to_strings_set(&strings, "____output_root_name".to_string().into());

        Self {
            strings,
            output_root_name,
        }
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
        get_or_insert_to_strings_set(&self.strings, str.into())
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
