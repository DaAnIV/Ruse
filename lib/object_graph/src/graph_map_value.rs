use crate::GraphsMap;
use core::fmt;
use std::{
    fmt::{Debug, Display},
    hash::Hash,
    sync::Arc,
};

pub trait GraphMapEq {
    fn eq(&self, self_graphs_map: &GraphsMap, other: &Self, other_graphs_map: &GraphsMap) -> bool;
}

pub trait GraphMapHash {
    fn calculate_hash<H: std::hash::Hasher>(&self, state: &mut H, graphs_map: &GraphsMap);
}

pub trait GraphMapDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>, graphs_map: &GraphsMap) -> fmt::Result;
}

pub trait GraphMapDebug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>, graphs_map: &GraphsMap) -> fmt::Result;
}

pub struct GraphMapValue<'a, T> {
    value: &'a T,
    map: &'a GraphsMap,
}

impl<'a, T> GraphMapValue<'a, T> {
    pub fn from(value: &'a T, map: &'a GraphsMap) -> GraphMapValue<'a, T> {
        Self { value, map }
    }
}

impl<T: GraphMapEq> Eq for GraphMapValue<'_, T> {}

impl<T: GraphMapEq> PartialEq for GraphMapValue<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        self.value.eq(self.map, other.value, other.map)
    }
}

impl<T: GraphMapHash> Hash for GraphMapValue<'_, T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.value.calculate_hash(state, self.map);
    }
}

impl<T: GraphMapDisplay> Display for GraphMapValue<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        GraphMapDisplay::fmt(self.value, f, self.map)
    }
}

impl<T: Debug> Debug for GraphMapValue<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.value)
    }
}

pub trait GraphMapWrap<T> {
    fn wrap<'a>(&'a self, graphs_map: &'a GraphsMap) -> GraphMapValue<'a, T>;
}

impl<T: GraphMapWrap<T>> GraphMapWrap<T> for Arc<T> {
    #[inline]
    fn wrap<'a>(&'a self, graphs_map: &'a GraphsMap) -> GraphMapValue<'a, T>
    where
        Self: Sized,
    {
        GraphMapValue::from(self.as_ref(), graphs_map)
    }
}
