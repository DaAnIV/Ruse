use std::fmt;

use crate::{class_name, ClassName};

#[derive(Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum ObjectType {
    Array(Box<ValueType>),
    Set(Box<ValueType>),
    Map(Box<ValueType>, Box<ValueType>),
    Class(ClassName),
    DOM,
    DOMElement,
}
impl ObjectType {
    pub fn class_obj_type(class_name: &str) -> ObjectType {
        ObjectType::Class(class_name!(class_name))
    }

    pub fn array_obj_type(elem_type: &ValueType) -> ObjectType {
        ObjectType::Array(elem_type.clone().into())
    }

    pub fn set_obj_type(elem_type: &ValueType) -> ObjectType {
        ObjectType::Set(elem_type.clone().into())
    }

    pub fn map_obj_type(key_type: &ValueType, value_type: &ValueType) -> ObjectType {
        ObjectType::Map(key_type.clone().into(), value_type.clone().into())
    }

    pub fn is_array_obj_type(&self) -> bool {
        matches!(self, ObjectType::Array(_))
    }

    pub fn is_set_obj_type(&self) -> bool {
        matches!(self, ObjectType::Set(_))
    }

    pub fn is_map_obj_type(&self) -> bool {
        matches!(self, ObjectType::Map(_, _))
    }

    pub fn is_class_obj_type(&self, class_name: &ClassName) -> bool {
        match self {
            ObjectType::Class(obj_class_name) => obj_class_name == class_name,
            _ => false,
        }
    }

    pub fn class_name(&self) -> Option<&ClassName> {
        match self {
            ObjectType::Class(class_name) => Some(class_name),
            _ => None,
        }
    }

    pub fn obj_type_base_name(&self) -> &str {
        match self {
            ObjectType::Class(class_name) => class_name.as_str(),
            ObjectType::Array(_) => "Array",
            ObjectType::Set(_) => "Set",
            ObjectType::Map(_, _) => "Map",
            ObjectType::DOM => "DOM",
            ObjectType::DOMElement => "DOMElement",
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum ValueType {
    Number,
    Bool,
    String,
    Object(ObjectType),
    Null,
}

impl ValueType {
    pub fn class_value_type(class_name: ClassName) -> ValueType {
        ValueType::Object(ObjectType::Class(class_name))
    }

    pub fn array_value_type(elem_type: &ValueType) -> ValueType {
        ValueType::Object(ObjectType::array_obj_type(elem_type))
    }

    pub fn set_value_type(elem_type: &ValueType) -> ValueType {
        ValueType::Object(ObjectType::set_obj_type(elem_type))
    }

    pub fn map_value_type(key_type: &ValueType, value_type: &ValueType) -> ValueType {
        ValueType::Object(ObjectType::map_obj_type(key_type, value_type))
    }

    pub fn is_primitive(&self) -> bool {
        matches!(
            self,
            ValueType::Number | ValueType::Bool | ValueType::String
        )
    }

    pub fn obj_type(&self) -> Option<&ObjectType> {
        match self {
            ValueType::Object(obj_type) => Some(obj_type),
            _ => None,
        }
    }
}

impl fmt::Display for ObjectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ObjectType::Array(elem_type) => write!(f, "Array<{}>", elem_type),
            ObjectType::Set(elem_type) => write!(f, "Set<{}>", elem_type),
            ObjectType::Map(key_type, value_type) => write!(f, "Map<{},{}>", key_type, value_type),
            ObjectType::Class(class_name) => write!(f, "{}", class_name),
            ObjectType::DOM => write!(f, "DOM"),
            ObjectType::DOMElement => write!(f, "DOMElement"),
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
            ValueType::Object(o) => write!(f, "{}", o),
        }
    }
}
