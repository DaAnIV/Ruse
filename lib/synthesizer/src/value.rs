use core::fmt;
use itertools::Itertools;
use ruse_object_graph::{
    scached, Cache, CachedString, FieldsMap, NodeIndex, Number, ObjectData, ObjectGraph,
    PrimitiveValue,
};
use std::{collections::HashMap, fmt::Debug, hash::Hash, sync::Arc};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ValueType {
    Number,
    Bool,
    String,
    Object(CachedString),
}

impl ValueType {
    pub fn is_array_obj_type(obj_type: &CachedString) -> bool {
        obj_type.starts_with("Array<")
    }

    pub fn array_obj_string(elem_type: &ValueType) -> String {
        format!("Array<{}>", elem_type)
    }

    pub fn array_obj_cached_string(elem_type: &ValueType, cache: &Cache) -> CachedString {
        scached!(cache; Self::array_obj_string(elem_type))
    }

    pub fn array_value_type(elem_type: &ValueType, cache: &Cache) -> ValueType {
        ValueType::Object(Self::array_obj_cached_string(elem_type, cache))
    }

    pub fn is_primitive(&self) -> bool {
        !matches!(self, ValueType::Object(_))
    }
}

#[derive(Debug, Clone)]
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
        self.get_primitive_field_value(field_name)
            .or(self.get_object_field_value(field_name))
    }

    pub fn get_primitive_field_value(&self, field_name: &CachedString) -> Option<Value> {
        Option::map(self.graph.get_field(self.node, field_name), |x| {
            Value::Primitive(x.clone())
        })
    }

    pub fn get_object_field_value(&self, field_name: &CachedString) -> Option<Value> {
        Option::map(self.graph.get_neighbor(self.node, field_name), |x| {
            Value::Object(ObjectValue {
                graph: self.graph.clone(),
                node: x,
            })
        })
    }

    pub fn obj_type(&self) -> CachedString {
        self.graph.node_weight(self.node).unwrap().obj_type.clone()
    }

    pub fn primitive_field_count(&self) -> usize {
        self.graph.node_weight(self.node).unwrap().fields.len()
    }

    pub fn pointers_field_count(&self) -> usize {
        self.graph.node_weight(self.node).unwrap().neighbors_count()
    }

    pub fn total_field_count(&self) -> usize {
        self.primitive_field_count() + self.pointers_field_count()
    }

    pub fn set_as_graph_root(&mut self, root: CachedString) {
        let graph = Arc::get_mut(&mut self.graph).unwrap();
        graph.set_as_root(root, self.node);
        graph.generate_serialized_data();
    }

    pub fn is_array(&self) -> bool {
        ValueType::is_array_obj_type(&self.obj_type())
    }

    pub fn fields(&self) -> impl Iterator<Item = (&Arc<String>, &PrimitiveValue)> {
        self.graph.fields(self.node)
    }

    pub fn neighbors(&self) -> impl Iterator<Item = (&Arc<String>, NodeIndex)> {
        self.graph
            .neighbors(self.node)
            .map(|(key, value)| (key, value.1))
    }
}

impl std::fmt::Display for ObjectValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_array() && self.primitive_field_count() > 0 {
            let values = self
                .graph
                .fields(self.node)
                .map(|(_, v)| v.to_string())
                .join(", ");
            return write!(f, "[{}]", values);
        }
        self.graph.fmt_node(f, self.node)
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

    pub fn generate_simple_object_from_map<I, T>(obj_type: CachedString, map: I) -> Value
    where
        I: IntoIterator<Item = (CachedString, T)>,
        T: Into<PrimitiveValue>,
    {
        let mut fields = FieldsMap::new();

        for (key, val) in map {
            fields.insert(key, val.into());
        }

        Self::create_simple_out_object(obj_type, fields)
    }

    pub fn generate_object_from_map<I>(obj_type: CachedString, map: I) -> Value
    where
        I: IntoIterator<Item = (CachedString, Value)>,
    {
        let mut fields = FieldsMap::new();
        let mut seen_graphs = HashMap::new();
        let mut obj_keys = vec![];

        for (key, val) in map {
            Self::visit_field(val, &mut fields, key, &mut seen_graphs, &mut obj_keys);
        }

        if seen_graphs.is_empty() {
            Self::create_simple_out_object(obj_type, fields)
        } else {
            Self::create_out_object(
                seen_graphs.into_values().collect(),
                obj_type,
                fields,
                &obj_keys,
            )
        }
    }

    fn visit_field(
        val: Value,
        fields: &mut FieldsMap,
        key: CachedString,
        seen_graphs: &mut HashMap<u64, Arc<ObjectGraph>>,
        obj_keys: &mut Vec<(CachedString, (u64, NodeIndex))>,
    ) {
        match val {
            Value::Primitive(p) => {
                fields.insert(key, p);
            }
            Value::Object(o) => {
                let ptr = Arc::as_ptr(&o.graph) as u64;
                seen_graphs.insert(ptr, o.graph);
                obj_keys.push((key, (ptr, o.node)));
            }
        };
    }

    fn create_out_object(
        graphs: Vec<Arc<ObjectGraph>>,
        obj_type: CachedString,
        fields: FieldsMap,
        obj_keys: &Vec<(CachedString, (u64, NodeIndex))>,
    ) -> Value {
        let (mut out, nodes_map) = ObjectGraph::union(&graphs);

        let node = out.add_node(ObjectData::new(obj_type, fields.into()));
        for (key, old_node) in obj_keys {
            out.add_edge(node, nodes_map[old_node], key);
        }

        out.generate_serialized_data();

        ObjectValue {
            graph: out.into(),
            node,
        }
        .into()
    }

    fn create_simple_out_object(obj_type: CachedString, fields: FieldsMap) -> Value {
        let mut graph = ObjectGraph::new();

        let node = graph.add_node(ObjectData::new(obj_type, fields.into()));

        graph.generate_serialized_data();

        ObjectValue {
            graph: graph.into(),
            node,
        }
        .into()
    }

    pub fn create_primitive_array_object<I>(
        elem_type: &ValueType,
        values: I,
        cache: &Cache,
    ) -> Value
    where
        I: IntoIterator,
        I::Item: Into<PrimitiveValue>,
    {
        let obj_type = ValueType::array_obj_cached_string(elem_type, cache);
        let map = values
            .into_iter()
            .enumerate()
            .map(|(i, v)| (scached!(cache; i.to_string()), v));
        Self::generate_simple_object_from_map(obj_type, map)
    }

    pub fn create_array_object<I>(elem_type: &ValueType, values: I, cache: &Cache) -> Value
    where
        I: IntoIterator<Item = Value>,
    {
        let obj_type = ValueType::array_obj_cached_string(elem_type, cache);
        let values_map = values
            .into_iter()
            .enumerate()
            .map(|(i, v)| (scached!(cache; i.to_string()), v));
        Self::generate_object_from_map(obj_type, values_map)
    }
}

impl Eq for ObjectValue {}

impl PartialEq for ObjectValue {
    fn eq(&self, other: &Self) -> bool {
        if self.graph == other.graph && self.node == other.node {
            return true;
        }
        ObjectGraph::slow_equal_roots((&self.graph, &self.node), (&other.graph, &other.node))
    }
}

impl Hash for ObjectValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.graph.hash(state);
        self.node.hash(state);
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

impl Location {
    pub fn is_temp(&self) -> bool {
        matches!(&self, Location::Temp)
    }

    pub fn is_var(&self) -> bool {
        matches!(&self, Location::Var(_))
    }

    pub fn is_object_field(&self) -> bool {
        matches!(&self, Location::ObjectField(_))
    }

    pub fn var(&self) -> Option<&'_ VarLoc> {
        match &self {
            Location::Var(l) => Some(l),
            _ => None,
        }
    }

    pub fn object_field(&self) -> Option<&'_ ObjectFieldLoc> {
        match &self {
            Location::ObjectField(l) => Some(l),
            _ => None,
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

    pub fn get_obj_field_loc_value(&self, field_name: &CachedString) -> Option<Self> {
        let obj = self.val().obj().unwrap();
        let field = obj.get_field_value(field_name)?;
        let loc = match &self.loc() {
            Location::Temp => Location::Temp,
            Location::Var(l) => Location::ObjectField(ObjectFieldLoc {
                var: l.var.clone(),
                node: obj.node,
                field: field_name.clone(),
            }),
            Location::ObjectField(l) => Location::ObjectField(ObjectFieldLoc {
                var: l.var.clone(),
                node: obj.node,
                field: field_name.clone(),
            }),
        };

        Some(Self { val: field, loc })
    }
}

impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueType::Number => f.write_str("Number"),
            ValueType::Bool => f.write_str("Bool"),
            ValueType::String => f.write_str("String"),
            ValueType::Object(o) => f.write_fmt(format_args!("{}", o.as_str())),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            Value::Primitive(p) => write!(f, "{}", p),
            Value::Object(o) => write!(f, "{}", o),
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
