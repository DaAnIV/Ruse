
use std::fmt::{self, Display, Formatter};
use std::{fmt::Debug, sync::Arc};
use std::hash::{Hash, Hasher};
use std::collections::BTreeMap;
use petgraph::graph::{EdgeIndex, NodeIndex};
use bitcode;

#[derive(bitcode::Encode, Clone, Copy, Debug, PartialOrd)]
pub struct Number(pub f64);

impl PartialEq for Number {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0 && self.0.is_sign_positive() == other.0.is_sign_positive()
    }
}

impl Eq for Number {}

impl Hash for Number {
    fn hash<H: Hasher>(&self, state: &mut H) {
        fn integer_decode(val: f64) -> (u64, i16, i8) {
            let bits: u64 = val.to_bits();
            let sign: i8 = if bits >> 63 == 0 { 1 } else { -1 };
            let mut exponent: i16 = ((bits >> 52) & 0x7ff) as i16;
            let mantissa = if exponent == 0 {
                (bits & 0xfffffffffffff) << 1
            } else {
                (bits & 0xfffffffffffff) | 0x10000000000000
            };

            exponent -= 1023 + 52;
            (mantissa, exponent, sign)
        }

        integer_decode(self.0).hash(state);
    }
}

impl Display for Number {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.0.is_infinite() {
            if self.0.is_sign_positive() {
                Display::fmt("Infinity", f)
            } else {
                Display::fmt("-Infinity", f)
            }
        } else {
            Display::fmt(&self.0, f)
        }
    }
}

impl From<f64> for Number {
    #[inline]
    fn from(value: f64) -> Self {
        Number(value)
    }
}

impl From<usize> for Number {
    #[inline]
    fn from(value: usize) -> Self {
        Number(value as _)
    }
}

impl From<u32> for Number {
    #[inline]
    fn from(value: u32) -> Self {
        Number(value as _)
    }
}

impl From<i32> for Number {
    #[inline]
    fn from(value: i32) -> Self {
        Number(value as _)
    }
}

impl From<u64> for Number {
    #[inline]
    fn from(value: u64) -> Self {
        Number(value as _)
    }
}

impl From<i64> for Number {
    #[inline]
    fn from(value: i64) -> Self {
        Number(value as _)
    }
}

impl From<Number> for u64 {
    #[inline]
    fn from(value: Number) -> u64 {
        value.0 as _
    }
}

impl Default for Number {
    fn default() -> Self {
        Number::from(0u64)
    }
}

#[derive(Hash, Clone, PartialEq, Eq, Debug, bitcode::Encode)]
pub enum PrimitiveValue {
    Number(Number),
    Bool(bool),
    String(Arc<String>),
    Null,
}

impl PrimitiveValue {
    pub fn number(&self) -> Option<Number> {
        match self {
            PrimitiveValue::Number(n) => Some(n.clone()),
            _ => None
        }
    }

    pub fn bool(&self) -> Option<bool> {
        match self {
            PrimitiveValue::Bool(b) => Some(b.clone()),
            _ => None
        }
    }

    pub fn string(&self) -> Option<Arc<String>> {
        match self {
            PrimitiveValue::String(s) => Some(s.clone()),
            _ => None
        }
    }
}

pub type FieldsMap = BTreeMap<Arc<String>, PrimitiveValue>;
pub type PointersMap = BTreeMap<Arc<String>, (EdgeIndex, NodeIndex)>;

#[derive(Clone)]
pub struct ObjectData {
    pub obj_type: Arc<String>,
    pub fields: Arc<FieldsMap>,
    pub(super) pointers: Arc<PointersMap>,
}

impl ObjectData {
    pub fn new(obj_type: Arc<String>, fields: Arc<FieldsMap>) -> Self {
        ObjectData {
            obj_type: obj_type,
            fields: fields,
            pointers: Default::default()
        }
    }

    pub fn get_neighbor(&self, name: &Arc<String>) -> Option<NodeIndex> {
        Some(self.pointers.get(name)?.1)
    }
}

impl Hash for ObjectData {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.fields.hash(state);
        for key in self.pointers.keys() {
            key.hash(state);
        }
    }
}

impl Debug for ObjectData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectData").field("fields", &self.fields).finish()
    }
}

#[macro_export]
macro_rules! fields {
    () => (
        std::sync::Arc::new($crate::FieldsMap::default())
    );
    ($($x:expr),+ $(,)?) => (
        std::sync::Arc::new($crate::FieldsMap::from([$($x),+]))
    );
}
