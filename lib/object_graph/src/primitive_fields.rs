use std::fmt::Debug;
use std::fmt::{self, Display, Formatter};
use std::hash::{Hash, Hasher};

use crate::{CachedString, ValueType};

#[derive(Clone, Copy, Debug, PartialOrd)]
pub struct Number(pub f64);

const NUMBER_TOLL: f64 = 0.000_1;

impl PartialEq for Number {
    fn eq(&self, other: &Self) -> bool {
        (self.0 == other.0 || (self.0 - other.0).abs() < NUMBER_TOLL)
            && self.0.is_sign_positive() == other.0.is_sign_positive()
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

impl num_traits::FromPrimitive for Number {
    fn from_i64(n: i64) -> Option<Self> {
        Some(Number(n as f64))
    }

    fn from_u64(n: u64) -> Option<Self> {
        Some(Number(n as f64))
    }

    fn from_f64(n: f64) -> Option<Self> {
        Some(Number(n))
    }
}

impl num_traits::ToPrimitive for Number {
    fn to_i64(&self) -> Option<i64> {
        num_traits::cast(self.0)
    }

    fn to_u64(&self) -> Option<u64> {
        num_traits::cast(self.0)
    }

    fn to_f64(&self) -> Option<f64> {
        Some(self.0)
    }
}

impl Default for Number {
    fn default() -> Self {
        Number::from(0u64)
    }
}

#[derive(Hash, Clone, PartialEq, Eq, Debug)]
pub enum PrimitiveValue {
    Number(Number),
    Bool(bool),
    String(CachedString),
}

impl PrimitiveValue {
    pub fn number(&self) -> Option<Number> {
        match self {
            PrimitiveValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn bool(&self) -> Option<bool> {
        match self {
            PrimitiveValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn string(&self) -> Option<CachedString> {
        match self {
            PrimitiveValue::String(s) => Some(s.clone()),
            _ => None,
        }
    }

    pub fn val_type(&self) -> ValueType {
        match self {
            PrimitiveValue::Number(_) => ValueType::Number,
            PrimitiveValue::Bool(_) => ValueType::Bool,
            PrimitiveValue::String(_) => ValueType::String,
        }
    }
}

impl From<CachedString> for PrimitiveValue {
    fn from(value: CachedString) -> Self {
        PrimitiveValue::String(value)
    }
}

impl From<Number> for PrimitiveValue {
    fn from(value: Number) -> Self {
        PrimitiveValue::Number(value)
    }
}

impl From<u64> for PrimitiveValue {
    fn from(value: u64) -> Self {
        PrimitiveValue::Number(Number::from(value))
    }
}

impl From<bool> for PrimitiveValue {
    fn from(value: bool) -> Self {
        PrimitiveValue::Bool(value)
    }
}

impl Display for PrimitiveValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            PrimitiveValue::Number(n) => write!(f, "{}", n),
            PrimitiveValue::Bool(b) => write!(f, "{}", b),
            PrimitiveValue::String(s) => write!(f, "\"{}\"", s),
        }
    }
}
