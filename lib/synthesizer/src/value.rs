use ruse_object_graph::{NodeIndex, ObjectGraph, PrimitiveValue};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ValueType {
    Number = 0,
    Bool,
    String,
    Object,
}

impl ValueType {
    pub fn range() -> usize {
        return ValueType::Object as usize;
    }
}

impl TryFrom<usize> for ValueType {
    type Error = ();

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(ValueType::Number),
            1 => Ok(ValueType::Bool),
            2 => Ok(ValueType::String),
            3 => Ok(ValueType::Number),
            _ => Err(()),
        }
    }
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
    pub var: Arc<String>,
    pub node: NodeIndex,
    pub field: Arc<String>,
}

#[derive(PartialEq, Eq, Debug, Clone, Hash)]
pub struct VarLoc {
    pub var: Arc<String>,
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
    pub fn get_field_value(&self, field_name: &Arc<String>) -> Option<Value> {
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

    pub fn val_type(&self) -> ValueType {
        match &self {
            Value::Primitive(p) => match p {
                PrimitiveValue::Number(_) => ValueType::Number,
                PrimitiveValue::Bool(_) => ValueType::Bool,
                PrimitiveValue::String(_) => ValueType::String,
                PrimitiveValue::Null => todo!(),
            },
            Value::Object(_) => ValueType::Object,
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
