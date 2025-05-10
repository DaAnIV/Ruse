use crate::{
    dot::{Dot, DotConfig},
    graph_equality::equal_graphs_by_node,
    graph_map_value::*,
    graph_node::EdgeEndPoint,
    graph_walk::ObjectGraphWalker,
    Attributes, ClassName, FieldName, GraphIndex, GraphsMap, NodeIndex, Number, ObjectGraph,
    ObjectType, PrimitiveField, PrimitiveValue, StringValue, ValueType,
};
use core::fmt;
use std::{
    fmt::{Debug, Display},
    hash::Hash,
    sync::Arc,
};

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
        self.graph(graphs_map)
            .get_neighbor(&self.node, field_name)
            .map(|x| x.attrs)
    }

    pub fn get_object_field(
        &self,
        field_name: &FieldName,
        graphs_map: &GraphsMap,
    ) -> Option<ObjectValue> {
        Option::map(
            self.graph(graphs_map).get_neighbor(&self.node, field_name),
            |x| {
                let field_graph_index = x.graph.unwrap_or(self.graph_id);
                ObjectValue {
                    obj_type: graphs_map[field_graph_index]
                        .obj_type(&x.node)
                        .unwrap()
                        .clone(),
                    graph_id: field_graph_index,
                    node: x.node,
                }
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
        self.obj_type.is_array_obj_type()
    }

    pub fn is_set(&self) -> bool {
        self.obj_type.is_set_obj_type()
    }

    pub fn is_class(&self, class_name: &ClassName) -> bool {
        self.obj_type.is_class_obj_type(class_name)
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

    pub fn val_type(&self) -> ValueType {
        ValueType::Object(self.obj_type.clone())
    }
}

impl ObjectValue {
    pub fn field_names_iterator<'a>(
        &self,
        graphs_map: &'a GraphsMap,
    ) -> impl Iterator<Item = &'a FieldName> {
        self.fields(graphs_map)
            .map(|(name, _)| name)
            .chain(self.neighbors(graphs_map).map(|(name, _)| name))
    }

    pub fn fields_values_iterator<'a>(
        &self,
        graphs_map: &'a GraphsMap,
    ) -> impl Iterator<Item = Value> + 'a {
        let self_graph_id = self.graph_id;
        self.fields(graphs_map)
            .map(|(_, p)| Value::Primitive(p.value.clone()))
            .chain(self.neighbors(graphs_map).map(move |(_, edge)| {
                let graph_id = edge.graph.unwrap_or(self_graph_id);
                let graph = graphs_map.get(&graph_id).unwrap();
                let obj_type = graph.obj_type(&edge.node).unwrap().clone();

                Value::Object(ObjectValue {
                    obj_type,
                    graph_id,
                    node: edge.node,
                })
            }))
    }

    pub fn fields_names_values_iterator<'a>(
        &self,
        graphs_map: &'a GraphsMap,
    ) -> impl Iterator<Item = (&'a FieldName, Value)> + 'a {
        let self_graph_id = self.graph_id;
        self.fields(graphs_map)
            .map(|(name, p)| (name, Value::Primitive(p.value.clone())))
            .chain(self.neighbors(graphs_map).map(move |(name, edge)| {
                let graph_id = edge.graph.unwrap_or(self_graph_id);
                let graph = graphs_map.get(&graph_id).unwrap();
                let obj_type = graph.obj_type(&edge.node).unwrap().clone();
                (
                    name,
                    Value::Object(ObjectValue {
                        obj_type,
                        graph_id,
                        node: edge.node,
                    }),
                )
            }))
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

    pub fn into_obj(self) -> Option<ObjectValue> {
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

    pub fn into_primitive(self) -> Option<PrimitiveValue> {
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

    pub fn string_value(&self) -> Option<StringValue> {
        match self {
            Value::Primitive(p) => p.string(),
            _ => None,
        }
    }

    pub fn val_type(&self) -> ValueType {
        match &self {
            Value::Primitive(p) => p.val_type(),
            Value::Object(o) => o.val_type(),
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
            (Value::Null, Value::Null) => true,
            _ => false,
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
macro_rules! vstr {
    ($x:expr) => {
        $crate::value::Value::Primitive(ruse_object_graph::PrimitiveValue::String(
            ruse_object_graph::StringValue::from($x),
        ))
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
