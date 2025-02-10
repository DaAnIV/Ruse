use std::ops::Deref;

use dashmap::DashSet;

#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct CachedString(byteview::StrView);

impl CachedString {
    fn new(val: &str) -> Self {
        Self(byteview::StrView::new(val))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Clones the given range of the existing string without heap allocation.
    #[must_use]
    pub fn slice(&self, range: impl std::ops::RangeBounds<usize>) -> Self {
        Self(self.0.slice(range))
    }

    /// Returns `true` if the string is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the amount of bytes in the string.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if `needle` is a prefix of the string or equal to the string.
    #[must_use]
    pub fn starts_with(&self, needle: &str) -> bool {
        self.0.starts_with(needle)
    }
}

impl std::fmt::Display for CachedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl std::fmt::Debug for CachedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0, f)
    }
}

impl Deref for CachedString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::borrow::Borrow<str> for CachedString {
    fn borrow(&self) -> &str {
        self
    }
}

impl AsRef<str> for CachedString {
    fn as_ref(&self) -> &str {
        self
    }
}

pub struct Cache {
    strings: DashSet<CachedString>,
}

impl Cache {
    pub fn new() -> Self {
        let strings = DashSet::default();
        
        let instance = Self {
            strings,
        };

        instance
    }

    pub fn get_or_insert_string(&self, val: String) -> CachedString {
        self.get_or_insert_str(&val)
    }

    pub fn get_or_insert_str(&self, val: &str) -> CachedString {
        let new_key = CachedString::new(val);
        self.strings.insert(new_key.clone());
        unsafe { self.strings.get(&new_key).unwrap_unchecked() }.clone()
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
