use crate::{
    dot::{Dot, DotConfig},
    graph_equality::equal_graphs_by_node,
    graph_map_value::*,
    graph_node::{EdgeEndPoint, FieldName, ObjectType},
    graph_walk::ObjectGraphWalker,
    scached, Attributes, Cache, CachedString, GraphIndex, GraphsMap, NodeIndex, Number,
    ObjectGraph, PrimitiveField, PrimitiveValue,
};
use core::fmt;
use std::{
    fmt::{Debug, Display},
    hash::Hash,
    sync::Arc,
};

#[derive(Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum ValueType {
    Number,
    Bool,
    String,
    Object(ObjectType),
    Null,
}

impl ValueType {
    pub fn is_array_obj_type(obj_type: &ObjectType) -> bool {
        obj_type.starts_with("Array<")
    }

    pub fn array_obj_string(elem_type: &ValueType) -> String {
        format!("Array<{}>", elem_type)
    }

    pub fn array_obj_cached_string(elem_type: &ValueType, cache: &Cache) -> ObjectType {
        scached!(cache; Self::array_obj_string(elem_type))
    }

    pub fn array_value_type(elem_type: &ValueType, cache: &Cache) -> ValueType {
        ValueType::Object(Self::array_obj_cached_string(elem_type, cache))
    }

    pub fn is_set_obj_type(obj_type: &ObjectType) -> bool {
        obj_type.starts_with("Set<")
    }

    pub fn set_obj_string(elem_type: &ValueType) -> String {
        format!("Set<{}>", elem_type)
    }

    pub fn set_obj_cached_string(elem_type: &ValueType, cache: &Cache) -> ObjectType {
        scached!(cache; Self::set_obj_string(elem_type))
    }

    pub fn set_value_type(elem_type: &ValueType, cache: &Cache) -> ValueType {
        ValueType::Object(Self::set_obj_cached_string(elem_type, cache))
    }

    pub fn is_primitive(&self) -> bool {
        !matches!(self, ValueType::Object(_))
    }

    pub fn obj_type(&self) -> Option<&ObjectType> {
        match self {
            ValueType::Object(obj_type) => Some(obj_type),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ObjectValue {
    pub obj_type: ObjectType,
    pub graph_id: GraphIndex,
    pub node: NodeIndex,
}

#[derive(Clone)]
pub enum Value {
    Primitive(PrimitiveValue),
    Object(ObjectValue),
    Null,
}

impl ObjectValue {
    pub fn graph(&self, graphs_map: &GraphsMap) -> Arc<ObjectGraph> {
        graphs_map[self.graph_id].clone()
    }

    pub fn graph_ref<'a>(&self, graphs_map: &'a GraphsMap) -> &'a ObjectGraph {
        &graphs_map[self.graph_id]
    }

    pub fn attrs(&self, graphs_map: &GraphsMap) -> Attributes {
        self.graph_ref(graphs_map)
            .node_attributes(self.node)
            .unwrap()
    }

    pub fn get_field_value(&self, field_name: &FieldName, graphs_map: &GraphsMap) -> Option<Value> {
        self.get_primitive_field_value(field_name, graphs_map)
            .or(self.get_object_field_value(field_name, graphs_map))
    }

    pub fn get_field_attrs(
        &self,
        field_name: &FieldName,
        graphs_map: &GraphsMap,
    ) -> Option<Attributes> {
        self.get_primitive_field_attrs(field_name, graphs_map)
            .or(self.get_object_field_attrs(field_name, graphs_map))
    }

    pub fn get_primitive_field_value(
        &self,
        field_name: &FieldName,
        graphs_map: &GraphsMap,
    ) -> Option<Value> {
        Option::map(self.get_primitive_field(field_name, graphs_map), |x| {
            Value::Primitive(x.value)
        })
    }

    pub fn get_primitive_field_attrs(
        &self,
        field_name: &FieldName,
        graphs_map: &GraphsMap,
    ) -> Option<Attributes> {
        Option::map(self.get_primitive_field(field_name, graphs_map), |x| {
            x.attributes
        })
    }

    fn get_primitive_field(
        &self,
        field_name: &FieldName,
        graphs_map: &GraphsMap,
    ) -> Option<PrimitiveField> {
        Option::map(
            self.graph(graphs_map)
                .get_primitive_field(&self.node, field_name),
            |x| x.clone(),
        )
    }

    pub fn get_object_field_value(
        &self,
        field_name: &FieldName,
        graphs_map: &GraphsMap,
    ) -> Option<Value> {
        Option::map(self.get_object_field(field_name, graphs_map), |x| {
            Value::Object(x)
        })
    }

    pub fn get_object_field_attrs(
        &self,
        field_name: &FieldName,
        graphs_map: &GraphsMap,
    ) -> Option<Attributes> {
        Option::map(self.get_object_field(field_name, graphs_map), |x| {
            x.attrs(graphs_map)
        })
    }

    pub fn get_object_field(
        &self,
        field_name: &FieldName,
        graphs_map: &GraphsMap,
    ) -> Option<ObjectValue> {
        Option::map(
            self.graph(graphs_map).get_neighbor(&self.node, field_name),
            |x| match x {
                EdgeEndPoint::Internal(field_node_index) => ObjectValue {
                    obj_type: self
                        .graph(graphs_map)
                        .obj_type(field_node_index)
                        .unwrap()
                        .clone(),
                    graph_id: self.graph_id,
                    node: *field_node_index,
                },
                EdgeEndPoint::Chain(field_graph_index, field_node_index) => ObjectValue {
                    obj_type: graphs_map[field_graph_index]
                        .obj_type(field_node_index)
                        .unwrap()
                        .clone(),
                    graph_id: *field_graph_index,
                    node: *field_node_index,
                },
            },
        )
    }

    pub fn primitive_field_count(&self, graphs_map: &GraphsMap) -> usize {
        self.graph(graphs_map).primitive_fields_count(&self.node)
    }

    pub fn pointers_field_count(&self, graphs_map: &GraphsMap) -> usize {
        self.graph(graphs_map).neighbors_count(&self.node)
    }

    pub fn total_field_count(&self, graphs_map: &GraphsMap) -> usize {
        self.primitive_field_count(graphs_map) + self.pointers_field_count(graphs_map)
    }

    pub fn is_array(&self) -> bool {
        ValueType::is_array_obj_type(&self.obj_type)
    }

    pub fn is_set(&self) -> bool {
        ValueType::is_set_obj_type(&self.obj_type)
    }

    pub fn fields<'a>(
        &self,
        graphs_map: &'a GraphsMap,
    ) -> impl Iterator<Item = (&'a FieldName, &'a PrimitiveField)> {
        graphs_map[&self.graph_id].primitive_fields(&self.node)
    }

    pub fn neighbors<'a>(
        &self,
        graphs_map: &'a GraphsMap,
    ) -> impl Iterator<Item = (&'a FieldName, &'a EdgeEndPoint)> {
        graphs_map[&self.graph_id].neighbors(&self.node)
    }

    pub fn dot_display<'b>(&self, graphs_map: &'b GraphsMap) -> ObjectValueDotDispaly<'_, 'b> {
        ObjectValueDotDispaly {
            value: self,
            graphs_map,
            config: DotConfig::default(),
        }
    }

    pub fn dot_display_with_config<'b>(
        &self,
        graphs_map: &'b GraphsMap,
        config: DotConfig,
    ) -> ObjectValueDotDispaly<'_, 'b> {
        ObjectValueDotDispaly {
            value: self,
            graphs_map,
            config,
        }
    }
}

impl GraphMapDisplay for ObjectValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>, graphs_map: &GraphsMap) -> fmt::Result {
        let graph = self.graph(graphs_map);
        if self.is_array() && self.pointers_field_count(graphs_map) == 0 {
            write!(f, "[")?;
            let mut iter = graph.primitive_fields(&self.node);
            match iter.next() {
                None => (),
                Some((_, first_field)) => {
                    write!(f, "{}", first_field.value)?;
                    iter.for_each(|(_, field)| {
                        write!(f, ", {}", field.value).unwrap();
                    });
                }
            }
            write!(f, "]")?;
            fmt::Result::Ok(())
        } else if self.is_set() && self.pointers_field_count(graphs_map) == 0 {
            write!(f, "{{")?;
            let mut iter = graph.primitive_fields(&self.node);
            match iter.next() {
                None => (),
                Some((_, first_field)) => {
                    write!(f, "{}", first_field.value)?;
                    iter.for_each(|(_, field)| {
                        write!(f, ", {}", field.value).unwrap();
                    });
                }
            }
            write!(f, "}}")?;
            fmt::Result::Ok(())
        } else {
            self.graph(graphs_map).fmt_node(f, graphs_map, &self.node)
        }
    }
}

pub struct ObjectValueDotDispaly<'a, 'b> {
    value: &'a ObjectValue,
    graphs_map: &'b GraphsMap,
    config: DotConfig,
}

impl<'a, 'b> Display for ObjectValueDotDispaly<'a, 'b> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            Dot::from_nodes_with_config(
                self.graphs_map,
                vec![(self.value.graph_id, self.value.node)],
                self.config.clone()
            )
        )
    }
}

impl Value {
    pub fn is_obj(&self) -> bool {
        matches!(*self, Value::Object(_))
    }

    pub fn is_primitive(&self) -> bool {
        matches!(*self, Value::Primitive(_))
    }

    pub fn obj(&self) -> Option<&ObjectValue> {
        match self {
            Value::Object(v) => Some(v),
            _ => None,
        }
    }

    pub fn mut_obj(&mut self) -> Option<&mut ObjectValue> {
        match self {
            Value::Object(v) => Some(v),
            _ => None,
        }
    }

    pub fn primitive(&self) -> Option<&PrimitiveValue> {
        match self {
            Value::Primitive(p) => Some(p),
            _ => None,
        }
    }

    pub fn number_value(&self) -> Option<Number> {
        match self {
            Value::Primitive(p) => p.number(),
            _ => None,
        }
    }

    pub fn bool_value(&self) -> Option<bool> {
        match self {
            Value::Primitive(p) => p.bool(),
            _ => None,
        }
    }

    pub fn string_value(&self) -> Option<CachedString> {
        match self {
            Value::Primitive(p) => p.string(),
            _ => None,
        }
    }

    pub fn val_type(&self) -> ValueType {
        match &self {
            Value::Primitive(p) => match p {
                PrimitiveValue::Number(_) => ValueType::Number,
                PrimitiveValue::Bool(_) => ValueType::Bool,
                PrimitiveValue::String(_) => ValueType::String,
            },
            Value::Object(o) => ValueType::Object(o.obj_type.clone()),
            Value::Null => ValueType::Null,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }
}

impl GraphMapWrap<Self> for ObjectValue {
    fn wrap<'a>(&'a self, graphs_map: &'a GraphsMap) -> GraphMapValue<'a, Self> {
        GraphMapValue::from(&self, graphs_map)
    }
}

impl GraphMapEq for ObjectValue {
    fn eq(&self, self_graphs_map: &GraphsMap, other: &Self, other_graphs_map: &GraphsMap) -> bool {
        equal_graphs_by_node(
            self_graphs_map,
            other_graphs_map,
            self.graph_id,
            other.graph_id,
            self.node,
            other.node,
        )
    }
}

impl GraphMapHash for ObjectValue {
    fn calculate_hash<H: std::hash::Hasher>(&self, state: &mut H, graphs_map: &GraphsMap) {
        for (_, _, node) in ObjectGraphWalker::from_node(graphs_map, self.graph_id, self.node) {
            node.hash(state);
        }
    }
}

impl From<ObjectValue> for Value {
    fn from(value: ObjectValue) -> Self {
        Value::Object(value)
    }
}

impl From<PrimitiveValue> for Value {
    fn from(value: PrimitiveValue) -> Self {
        Value::Primitive(value)
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Primitive(primitive_value) => fmt::Debug::fmt(primitive_value, f),
            Value::Object(object_value) => fmt::Debug::fmt(object_value, f),
            Value::Null => write!(f, "Null"),
        }
    }
}

impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueType::Number => write!(f, "Number"),
            ValueType::Bool => write!(f, "Bool"),
            ValueType::String => write!(f, "String"),
            ValueType::Null => write!(f, "Null"),
            ValueType::Object(o) => write!(f, "{}", o.as_str()),
        }
    }
}

impl GraphMapWrap<Self> for Value {
    fn wrap<'a>(&'a self, graphs_map: &'a GraphsMap) -> GraphMapValue<'a, Self> {
        GraphMapValue::from(&self, graphs_map)
    }
}

impl GraphMapEq for Value {
    fn eq(&self, self_graphs_map: &GraphsMap, other: &Self, other_graphs_map: &GraphsMap) -> bool {
        match (self, other) {
            (Value::Primitive(self_primitive_value), Value::Primitive(other_primitive_value)) => {
                self_primitive_value == other_primitive_value
            }
            (Value::Object(self_object_value), Value::Object(other_object_value)) => {
                self_object_value.eq(self_graphs_map, other_object_value, other_graphs_map)
            }
            (_, _) => false,
        }
    }
}

impl GraphMapHash for Value {
    fn calculate_hash<H: std::hash::Hasher>(&self, state: &mut H, graphs_map: &GraphsMap) {
        match self {
            Value::Primitive(primitive_value) => primitive_value.hash(state),
            Value::Object(object_value) => object_value.calculate_hash(state, graphs_map),
            Value::Null => 0.hash(state),
        }
    }
}

impl GraphMapDisplay for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>, graphs_map: &GraphsMap) -> fmt::Result {
        match &self {
            Value::Primitive(p) => write!(f, "{}", p),
            Value::Object(o) => GraphMapDisplay::fmt(o, f, graphs_map),
            Value::Null => write!(f, "Null"),
        }
    }
}

pub struct ValueDotDispaly<'a, 'b> {
    value: &'a Value,
    graphs_map: &'b GraphsMap,
    config: DotConfig,
}

impl<'a, 'b> Display for ValueDotDispaly<'a, 'b> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.value {
            Value::Primitive(p) => {
                Dot::write_header_with_config(f, &self.config)?;
                Dot::write_node(
                    f,
                    "p",
                    &format!("{}", p),
                    self.config.subgraph.as_ref().map(|s| s.name.to_string()),
                )?;
                Dot::write_footer_with_config(f, &self.config)?;

                Ok(())
            }
            Value::Object(o) => write!(
                f,
                "{}",
                o.dot_display_with_config(self.graphs_map, self.config.clone())
            ),
            Value::Null => {
                Dot::write_header_with_config(f, &self.config)?;
                Dot::write_node(
                    f,
                    "Null",
                    "Null",
                    self.config.subgraph.as_ref().map(|s| s.name.to_string()),
                )?;
                Dot::write_footer_with_config(f, &self.config)?;

                Ok(())
            }
        }
    }
}

impl Value {
    pub fn dot_display<'b>(&self, graphs_map: &'b GraphsMap) -> ValueDotDispaly<'_, 'b> {
        ValueDotDispaly {
            value: self,
            graphs_map,
            config: DotConfig::default(),
        }
    }

    pub fn dot_display_with_config<'b>(
        &self,
        graphs_map: &'b GraphsMap,
        config: DotConfig,
    ) -> ValueDotDispaly<'_, 'b> {
        ValueDotDispaly {
            value: self,
            graphs_map,
            config,
        }
    }
}

#[macro_export]
macro_rules! vnull {
    () => {
        $crate::value::Value::Null
    };
}

#[macro_export]
macro_rules! vbool {
    ($e:expr) => {
        $crate::value::Value::Primitive(ruse_object_graph::PrimitiveValue::Bool($e))
    };
}

#[macro_export]
macro_rules! vnum {
    ($e:expr) => {
        $crate::value::Value::Primitive(ruse_object_graph::PrimitiveValue::Number($e))
    };
}

#[macro_export]
macro_rules! vstring {
    ($cache:expr; $e:expr) => { $crate::value::Value::Primitive(ruse_object_graph::PrimitiveValue::String(ruse_object_graph::scached!($cache; $e))) }
}

#[macro_export]
macro_rules! vstr {
    ($cache:expr; $e:expr) => { $crate::value::Value::Primitive(ruse_object_graph::PrimitiveValue::String(ruse_object_graph::str_cached!($cache; $e))) }
}

#[macro_export]
macro_rules! vcstring {
    ($e:expr) => {
        $crate::value::Value::Primitive(ruse_object_graph::PrimitiveValue::String($e))
    };
}

#[macro_export]
macro_rules! vobj {
    ($t:expr,$g:expr,$r:expr) => {
        $crate::value::Value::Object($crate::value::ObjectValue {
            obj_type: $t,
            graph_id: $g,
            node: $r,
        })
    };
}
