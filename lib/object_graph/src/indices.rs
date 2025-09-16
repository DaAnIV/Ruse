use std::{
    fmt,
    ops::{Add, AddAssign},
};

pub type DefaultIx = usize;

macro_rules! impl_index_type {
    ($name:ident) => {
        #[derive(
            Clone,
            Copy,
            PartialEq,
            Eq,
            Hash,
            Debug,
            PartialOrd,
            Ord,
            serde::Serialize,
            serde::Deserialize,
        )]
        pub struct $name<Ix: Copy = DefaultIx>(pub Ix);

        impl<Ix: Copy> $name<Ix> {
            pub fn index(&self) -> Ix {
                self.0
            }
        }

        impl<Ix> AddAssign<Ix> for $name<Ix>
        where
            Ix: Copy,
            Ix: AddAssign,
        {
            fn add_assign(&mut self, rhs: Ix) {
                self.0 += rhs
            }
        }

        impl<Ix: AddAssign> AddAssign for $name<Ix>
        where
            Ix: Copy,
            Ix: AddAssign,
        {
            fn add_assign(&mut self, rhs: Self) {
                self.0 += rhs.0
            }
        }

        impl<Ix: Add<Output = Ix>> Add<Ix> for $name<Ix>
        where
            Ix: Copy,
            Ix: Add<Output = Ix>,
        {
            type Output = Self;

            fn add(self, rhs: Ix) -> Self {
                Self(self.0 + rhs)
            }
        }

        impl<Ix: Add<Output = Ix>> Add for $name<Ix>
        where
            Ix: Copy,
            Ix: Add<Output = Ix>,
        {
            type Output = Self;

            fn add(self, rhs: Self) -> Self {
                Self(self.0 + rhs.0)
            }
        }

        impl<Ix: Copy> From<Ix> for $name<Ix> {
            fn from(value: Ix) -> Self {
                Self(value)
            }
        }

        impl<Ix: fmt::Display> fmt::Display for $name<Ix>
        where
            Ix: Copy,
            Ix: Add<Output = Ix>,
        {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

impl_index_type!(NodeIndex);
impl_index_type!(GraphIndex);
