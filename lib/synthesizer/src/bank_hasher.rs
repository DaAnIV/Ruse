use siphasher::sip::SipHasher13;
use std::cell::Cell;
use std::fmt;
use std::hash::{BuildHasher, Hasher};
use std::str::FromStr;

fn hashmap_random_keys() -> (u64, u64) {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).expect("Failed to get random buffer");
    let k1 = u64::from_ne_bytes(bytes[..8].try_into().unwrap());
    let k2 = u64::from_ne_bytes(bytes[8..].try_into().unwrap());
    (k1, k2)
}

#[derive(Clone, Copy, Debug)]
pub struct BankKeys(u64, u64);

impl FromStr for BankKeys {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split(',');

        let first = parts.next().unwrap().to_owned();
        let second = parts.next().map(|x| x.to_owned());
        if parts.next().is_some() {
            return Err(anyhow::Error::msg("Value contains more then two ','"));
        }

        let k0: u64 = first.parse()?;
        let k1: u64 = if let Some(next) = second {
            next.parse()?
        } else {
            0
        };

        Ok(Self(k0, k1))
    }
}

pub struct BankHasher(SipHasher13);

impl BankHasher {
    pub fn new_with_keys(key0: u64, key1: u64) -> BankHasher {
        BankHasher(SipHasher13::new_with_keys(key0, key1))
    }
}

impl Hasher for BankHasher {
    // The underlying `SipHasher13` doesn't override the other
    // `write_*` methods, so it's ok not to forward them here.

    #[inline]
    fn write(&mut self, msg: &[u8]) {
        self.0.write(msg)
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.0.finish()
    }
}

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Deserialize,
    serde::Serialize,
)]
pub struct BankHasherBuilder {
    k0: u64,
    k1: u64,
}

impl BankHasherBuilder {
    pub const fn new() -> Self {
        Self { k0: 0, k1: 0 }
    }

    pub fn new_with_keys(keys: BankKeys) -> Self {
        Self { k0: keys.0, k1: keys.1 }
    }

    pub fn new_with_random_keys() -> Self {
        // Copied from std::hash::RandomState
        thread_local!(static KEYS: Cell<(u64, u64)> = {
            Cell::new(hashmap_random_keys())
        });

        KEYS.with(|keys| {
            let (k0, k1) = keys.get();
            keys.set((k0.wrapping_add(1), k1));
            Self { k0, k1 }
        })
    }
}

impl BuildHasher for BankHasherBuilder {
    type Hasher = BankHasher;

    fn build_hasher(&self) -> BankHasher {
        BankHasher::new_with_keys(self.k0, self.k1)
    }
}

impl fmt::Display for BankHasherBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "k0: {}, k1: {}", self.k0, self.k1)
    }
}
