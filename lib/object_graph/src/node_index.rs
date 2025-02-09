use std::{
    fmt,
    ops::{Add, AddAssign},
};

pub type DefaultIx = usize;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct NodeIndex<Ix: Copy = DefaultIx>(pub Ix);

impl<Ix: Copy> NodeIndex<Ix> {
    pub fn index(&self) -> Ix {
        self.0
    }
}

impl<Ix> AddAssign<Ix> for NodeIndex<Ix>
where
    Ix: Copy,
    Ix: AddAssign,
{
    fn add_assign(&mut self, rhs: Ix) {
        self.0 += rhs
    }
}

impl<Ix: AddAssign> AddAssign for NodeIndex<Ix>
where
    Ix: Copy,
    Ix: AddAssign,
{
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0
    }
}

impl<Ix: Add<Output = Ix>> Add<Ix> for NodeIndex<Ix>
where
    Ix: Copy,
    Ix: Add<Output = Ix>,
{
    type Output = Self;

    fn add(self, rhs: Ix) -> Self {
        Self(self.0 + rhs)
    }
}

impl<Ix: Add<Output = Ix>> Add for NodeIndex<Ix>
where
    Ix: Copy,
    Ix: Add<Output = Ix>,
{
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl<Ix: Copy> From<Ix> for NodeIndex<Ix> {
    fn from(value: Ix) -> Self {
        Self(value)
    }
}

impl<Ix: fmt::Display> fmt::Display for NodeIndex<Ix>
where
    Ix: Copy,
    Ix: Add<Output = Ix>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
