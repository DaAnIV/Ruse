use core::fmt;
use std::collections::BTreeMap;

use crate::{node_index::NodeIndex, CachedString, GraphIndex, PrimitiveValue};

use std::hash::{Hash, Hasher};

pub type ObjectType = CachedString;
pub type FieldName = CachedString;

#[derive(Clone, Copy, Debug)]
pub enum EdgeEndPoint {
    Internal(NodeIndex),
    Chain(GraphIndex, NodeIndex),
}

impl EdgeEndPoint {
    pub fn index(&self) -> &NodeIndex {
        match self {
            EdgeEndPoint::Internal(index) => index,
            EdgeEndPoint::Chain(_, index) => index,
        }
    }
}

pub type FieldsMap = BTreeMap<FieldName, PrimitiveValue>;
pub type PointersMap = BTreeMap<FieldName, EdgeEndPoint>;

#[derive(Clone)]
pub struct ObjectGraphNode {
    pub id: NodeIndex,
    pub obj_type: ObjectType,
    pub fields: FieldsMap,
    pub pointers: PointersMap,
}

impl ObjectGraphNode {
    pub fn insert_internal_edge(&mut self, field_name: FieldName, neig: NodeIndex) {
        self.pointers
            .insert(field_name, EdgeEndPoint::Internal(neig));
    }

    pub fn insert_chain_edge(
        &mut self,
        field_name: FieldName,
        neig_graph: GraphIndex,
        neig_node: NodeIndex,
    ) {
        self.pointers
            .insert(field_name, EdgeEndPoint::Chain(neig_graph, neig_node));
    }
}

impl Hash for ObjectGraphNode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.obj_type.hash(state);
        self.fields.hash(state);
        for (key, _) in &self.pointers {
            key.hash(state);
        }
    }
}

impl fmt::Display for ObjectGraphNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}: {{", self.obj_type)?;
        for (field, value) in &self.fields {
            writeln!(f, "  {}: {}", field, value)?;
        }
        writeln!(f, "}}")?;

        Ok(())
    }
}

impl fmt::Debug for ObjectGraphNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectGraphNode")
            .field("obj_type", &self.obj_type)
            .field("fields", &self.fields)
            .finish()
    }
}

#[macro_export]
macro_rules! fields {
    () => (
        $crate::FieldsMap::default()
    );
    ($($x:expr),+ $(,)?) => (
        $crate::FieldsMap::from([$($x),+])
    );
}
