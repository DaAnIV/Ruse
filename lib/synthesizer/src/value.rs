use core::fmt;
use ruse_object_graph::{CachedString, NodeIndex, Number, ObjectGraph, PrimitiveValue};
use std::sync::Arc;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ValueType {
    Number,
    Bool,
    String,
    Object(CachedString),
}

#[derive(PartialEq, Eq, Debug, Clone, Hash)]
pub struct ObjectValue {
    pub graph: Arc<ObjectGraph>,
    pub node: NodeIndex,
}

#[derive(PartialEq, Eq, Debug, Clone, Hash)]
pub enum Value {
    Primitive(PrimitiveValue),
    Object(ObjectValue),
}

#[derive(PartialEq, Eq, Debug, Clone, Hash)]
pub struct ObjectFieldLoc {
    pub var: CachedString,
    pub node: NodeIndex,
    pub field: CachedString,
}

#[derive(PartialEq, Eq, Debug, Clone, Hash)]
pub struct VarLoc {
    pub var: CachedString,
}

#[derive(PartialEq, Eq, Debug, Clone, Hash)]
pub enum Location {
    Temp,
    Var(VarLoc),
    ObjectField(ObjectFieldLoc),
}

#[derive(PartialEq, Eq, Debug, Clone, Hash)]
pub struct LocValue {
    pub(crate) loc: Location,
    pub(crate) val: Value,
}

impl ObjectValue {
    pub fn get_field_value(&self, field_name: &CachedString) -> Option<Value> {
        match self.graph.get_field(self.node, &field_name) {
            Some(val) => Some(Value::Primitive(val.clone())),
            None => match self.graph.get_neighbor(self.node, &field_name) {
                Some(neighbor) => Some(Value::Object(ObjectValue {
                    graph: self.graph.clone(),
                    node: neighbor,
                })),
                None => None,
            },
        }
    }

    pub fn obj_type(&self) -> CachedString {
        self.graph.node_weight(self.node).unwrap().obj_type.clone()
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
                PrimitiveValue::Null => todo!(),
            },
            Value::Object(o) => ValueType::Object(o.obj_type()),
        }
    }
}

impl Location {
    pub fn is_temp(&self) -> bool {
        match &self {
            Location::Temp => true,
            _ => false,
        }
    }

    pub fn is_var(&self) -> bool {
        match &self {
            Location::Var(_) => true,
            _ => false,
        }
    }

    pub fn is_object_field(&self) -> bool {
        match &self {
            Location::ObjectField(_) => true,
            _ => false,
        }
    }
}

impl LocValue {
    #[inline]
    pub fn val(&self) -> &Value {
        &self.val
    }
    #[inline]
    pub fn loc(&self) -> &Location {
        &self.loc
    }
}

impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueType::Number => f.write_str("Number"),
            ValueType::Bool => f.write_str("Bool"),
            ValueType::String => f.write_str("String"),
            ValueType::Object(o) => f.write_fmt(format_args!("Object({})", o.as_str())),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            Value::Primitive(p) => write!(f, "{}", p),
            Value::Object(o) => o.graph.fmt_node(f, o.node),
        }
    }
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
    ($g:expr,$r:expr) => {
        $crate::value::Value::Object($crate::value::ObjectValue {
            graph: $g,
            node: $r,
        })
    };
}
