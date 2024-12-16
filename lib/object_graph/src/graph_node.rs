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
    obj_type: ObjectType,
    fields: FieldsMap,
    pointers: PointersMap,
}

impl ObjectGraphNode {
    pub fn new(obj_type: ObjectType, fields: FieldsMap, pointers: PointersMap) -> Self {
        Self {
            obj_type,
            fields,
            pointers: pointers,
        }
    }

    pub fn clone_without_pointers(&self) -> Self {
        Self::new(
            self.obj_type.clone(),
            self.fields.clone(),
            Default::default(),
        )
    }

    pub fn obj_type(&self) -> &ObjectType {
        &self.obj_type
    }

    pub fn pointers_len(&self) -> usize {
        self.pointers.len()
    }

    pub fn pointers_get(&self, field_name: &FieldName) -> Option<&EdgeEndPoint> {
        self.pointers.get(field_name)
    }

    pub fn pointers_iter(&self) -> impl std::iter::Iterator<Item = (&FieldName, &EdgeEndPoint)> {
        self.pointers.iter()
    }

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

    pub fn pointers_remove(&mut self, field_name: &FieldName) -> Option<EdgeEndPoint> {
        self.pointers.remove(field_name)
    }

    pub fn fields_len(&self) -> usize {
        self.fields.len()
    }

    pub fn fields_iter(&self) -> impl std::iter::Iterator<Item = (&FieldName, &PrimitiveValue)> {
        self.fields.iter()
    }

    pub fn get_field(&self, field_name: &FieldName) -> Option<&PrimitiveValue> {
        self.fields.get(field_name)
    }

    pub fn insert_field(
        &mut self,
        field_name: FieldName,
        value: PrimitiveValue,
    ) -> Option<PrimitiveValue> {
        self.fields.insert(field_name, value)
    }

    pub fn remove_field(&mut self, field_name: &FieldName) -> Option<PrimitiveValue> {
        self.fields.remove(field_name)
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

impl Eq for ObjectGraphNode {}
impl PartialEq for ObjectGraphNode {
    fn eq(&self, other: &Self) -> bool {
        self.obj_type == other.obj_type
            && self.fields == other.fields
            && self.pointers.keys().eq(other.pointers.keys())
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
