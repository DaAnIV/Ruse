use std::ops::Deref;

#[derive(Clone, Hash, PartialEq, Eq)]
pub struct ClassName(hstr::Atom);

#[derive(Clone, Hash, PartialEq, Eq)]
pub struct FieldName(hstr::Atom);

#[derive(Clone, Hash, PartialEq, Eq)]
pub struct StringValue(hstr::Atom);

#[derive(Clone, Hash, PartialEq, Eq)]
pub struct RootName(hstr::Atom);

// Common trait implementations for all string types
macro_rules! impl_string_type {
    ($type:ident) => {
        impl $type {
            pub fn new<S>(val: S) -> Self
            where
                Self: From<S>,
            {
                Self::from(val)
            }

            pub fn as_str(&self) -> &str {
                self.0.as_ref()
            }

            pub fn is_empty(&self) -> bool {
                self.0.is_empty()
            }

            pub fn len(&self) -> usize {
                self.0.len()
            }

            pub fn starts_with(&self, needle: &str) -> bool {
                self.0.starts_with(needle)
            }
        }

        impl std::fmt::Display for $type {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Display::fmt(self.as_str(), f)
            }
        }

        impl std::fmt::Debug for $type {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Debug::fmt(self.as_str(), f)
            }
        }

        impl std::cmp::PartialOrd for $type {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                self.as_str().partial_cmp(other.as_str())
            }
        }

        impl std::cmp::Ord for $type {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.as_str().cmp(other.as_str())
            }
        }

        impl Deref for $type {
            type Target = str;

            fn deref(&self) -> &Self::Target {
                self.as_str()
            }
        }

        impl std::borrow::Borrow<str> for $type {
            fn borrow(&self) -> &str {
                self.as_str()
            }
        }

        impl AsRef<str> for $type {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl<S> From<S> for $type
        where
            hstr::Atom: From<S>,
        {
            fn from(value: S) -> Self {
                $type(hstr::Atom::from(value))
            }
        }

        impl serde::Serialize for $type {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> serde::Deserialize<'de> for $type {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let s = String::deserialize(deserializer)?;
                Ok($type::new(s))
            }
        }
    };
}

impl_string_type!(ClassName);
impl_string_type!(FieldName);
impl_string_type!(StringValue);
impl_string_type!(RootName);

#[macro_export]
macro_rules! str_cached {
    ($x:expr) => {
        $crate::StringValue::new($x)
    };
}

#[macro_export]
macro_rules! class_name {
    ($x:expr) => {
        $crate::ClassName::new($x)
    };
}

#[macro_export]
macro_rules! field_name {
    ($x:expr) => {
        $crate::FieldName::new($x)
    };
}

#[macro_export]
macro_rules! root_name {
    ($x:expr) => {
        $crate::RootName::new($x)
    };
}
