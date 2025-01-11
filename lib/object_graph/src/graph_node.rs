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

#[derive(Debug, Clone)]
pub struct Attributes {
    pub readonly: bool
}

impl Default for Attributes {
    fn default() -> Self {
        Self { readonly: false }
    }
}

#[derive(Debug, Clone)]
pub struct PrimitiveField {
    pub value: PrimitiveValue,
    pub attributes: Attributes
}

impl From<PrimitiveValue> for PrimitiveField {
    fn from(value: PrimitiveValue) -> Self {
        Self {
            value,
            attributes: Attributes::default(),
        }
    }
}

impl Hash for PrimitiveField {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl Eq for PrimitiveField {}
impl PartialEq for PrimitiveField {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

pub type FieldsMap = BTreeMap<FieldName, PrimitiveField>;
pub type PointersMap = BTreeMap<FieldName, EdgeEndPoint>;

#[derive(Clone)]
pub struct ObjectGraphNode {
    obj_type: ObjectType,
    fields: FieldsMap,
    pointers: PointersMap,
    pub attributes: Attributes
}

impl ObjectGraphNode {
    pub fn new(obj_type: ObjectType, fields: FieldsMap, pointers: PointersMap) -> Self {
        Self {
            obj_type,
            fields,
            pointers: pointers,
            attributes: Attributes::default()
        }
    }
    pub fn new_with_attrs(obj_type: ObjectType, fields: FieldsMap, pointers: PointersMap, attributes: Attributes) -> Self {
        Self {
            obj_type,
            fields,
            pointers: pointers,
            attributes
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

    pub fn fields_iter(&self) -> impl std::iter::Iterator<Item = (&FieldName, &PrimitiveField)> {
        self.fields.iter()
    }

    pub fn get_field(&self, field_name: &FieldName) -> Option<&PrimitiveField> {
        self.fields.get(field_name)
    }

    pub fn insert_field(
        &mut self,
        field_name: FieldName,
        value: PrimitiveValue,
    ) -> Option<PrimitiveField> {
        self.insert_field_with_attributes(field_name, value, Attributes::default())
    }

    pub fn insert_field_with_attributes(
        &mut self,
        field_name: FieldName,
        value: PrimitiveValue,
        attributes: Attributes
    ) -> Option<PrimitiveField> {
        self.fields.insert(field_name, PrimitiveField { value, attributes })
    }

    pub fn remove_field(&mut self, field_name: &FieldName) -> Option<PrimitiveField> {
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
        for (field_name, field) in &self.fields {
            writeln!(f, "  {}: {}", field_name, field.value)?;
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
    ($(($key:expr, $value:expr)),+) => (
        $crate::FieldsMap::from([$(($key, $value.into())),+])
    );
    ($(($key:expr, $value:expr, $attrs:expr)),+ $(,)?) => (
        $crate::FieldsMap::from([$(($key, $crate::PrimitiveField { value: $value, attributes: $attrs })),+])
    );
}
