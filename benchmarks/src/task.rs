use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt::{Debug, Display},
    hash::{BuildHasherDefault, DefaultHasher},
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};

use graph_map_value::GraphMapWrap;
use itertools::{izip, Itertools};
use ruse_object_graph::{
    value::{Value, ValueType},
    *,
};
use ruse_synthesizer::{
    context::{Context, ContextArray, GraphIdGenerator, ValuesMap},
    opcode::ExprOpcode,
    prog::SubProgram,
    synthesizer::{OpcodesList, SynthesizerPredicate},
};
use ruse_ts_interpreter::{dom, ts_class::TsClasses};
use ruse_ts_synthesizer::{
    add_array_opcodes, add_dom_opcodes, add_num_opcodes, add_set_opcodes, add_str_opcodes,
    construct_opcode_list, TsSynthesizer, ALL_BIN_NUM_OPCODES, ALL_BIN_STR_OPCODES,
    ALL_UNARY_NUM_OPCODES, ALL_UPDATE_NUM_OPCODES,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::json;
use wildmatch::WildMatch;

use crate::results::BenchmarkResult;

#[derive(Debug)]
pub struct TodoError {
    pub to_implement: &'static str,
}

impl TodoError {
    #[allow(dead_code)]
    pub fn new(to_implement: &'static str) -> Self {
        Self { to_implement }
    }
}

impl std::error::Error for TodoError {}

impl std::fmt::Display for TodoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} not yet implemented", self.to_implement)
    }
}

#[derive(Debug)]
pub struct ParseError {
    pub value: String,
    pub error: Box<dyn std::error::Error>,
}

impl std::error::Error for ParseError {}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to parse {}, Error: {}", self.value, self.error)
    }
}

#[derive(Debug)]
pub struct VerifyError {
    pub msg: String,
}

impl From<&str> for VerifyError {
    fn from(value: &str) -> Self {
        VerifyError {
            msg: value.to_owned(),
        }
    }
}

impl std::error::Error for VerifyError {}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

#[derive(Debug)]
pub enum SnythesisTaskError {
    IO(std::io::Error),
    Verify(VerifyError),
    Parse(ParseError),
}

impl std::error::Error for SnythesisTaskError {}

macro_rules! parse_err {
    ($val:expr, $e:expr) => {
        $crate::task::SnythesisTaskError::Parse($crate::task::ParseError {
            value: $val.to_string(),
            error: $e.into(),
        })
    };
}

macro_rules! verify_err {
    ($msg:expr) => {
        $crate::task::SnythesisTaskError::Verify($crate::task::VerifyError {
            msg: $msg.to_owned(),
        })
    };
}

impl std::fmt::Display for SnythesisTaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SnythesisTaskError::IO(e) => write!(f, "{}", e),
            SnythesisTaskError::Verify(e) => write!(f, "{}", e),
            SnythesisTaskError::Parse(e) => write!(f, "{}", e),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct VarRef {
    var: String,
    fields: Vec<FieldName>,
}

impl VarRef {
    fn create_value(
        &self,
        values: &ValuesMap,
        graphs_map: &GraphsMap,
    ) -> Result<Value, SnythesisTaskError> {
        let value = values.get(&self.var).ok_or(parse_err!(
            format!("{}", self),
            "Pointing to an uninitialized value"
        ))?;
        self.walk_fields(value, graphs_map)
    }

    fn walk_fields(
        &self,
        value: &Value,
        graphs_map: &GraphsMap,
    ) -> Result<Value, SnythesisTaskError> {
        if self.fields.is_empty() {
            Ok(value.clone())
        } else {
            let mut cur_value = value.clone();
            for field in &self.fields {
                if let Value::Object(obj) = cur_value {
                    cur_value = obj.get_field_value(field, graphs_map).ok_or(parse_err!(
                        format!("{}", self),
                        format!("Couldn't find field {}", field)
                    ))?;
                } else {
                    return Err(parse_err!(
                        format!("{}", self),
                        format!("Can't deref field {} on primitive value", field)
                    ));
                }
            }

            Ok(cur_value)
        }
    }
}

impl Display for VarRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.fields.is_empty() {
            write!(f, "{}", &self.var)
        } else {
            write!(f, "{}.{}", &self.var, self.fields.iter().join("."))
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum TaskType {
    Number,
    NumberArray,
    Bool,
    String,
    StringArray,
    NumberSet,
    StringSet,
    Dom,
    DOMElement,
    VarRef(VarRef),
    Object(String),
}

fn get_value_from_file_or_value(
    dir: &Path,
    value: &serde_json::Value,
) -> Result<serde_json::Value, SnythesisTaskError> {
    if let Some(filename) = value.as_str() {
        let full_path = dir.join(filename);
        if let Ok(mut html_file) = std::fs::File::open(full_path) {
            let mut buf = String::new();
            if let Err(e) = html_file.read_to_string(&mut buf) {
                return Err(parse_err!(value, e));
            }
            return Ok(serde_json::Value::String(buf));
        }
    }

    Ok(value.to_owned())
}

impl TaskType {
    pub fn create_value(
        &self,
        value: &serde_json::Value,
        graph: &mut ObjectGraph,
        classes: &TsClasses,
        graphs_map: &mut GraphsMap,
        id_gen: &GraphIdGenerator,
        cache: &Cache,
    ) -> Result<Value, SnythesisTaskError> {
        match self {
            TaskType::Number => match value.as_i64() {
                Some(num) => Ok(vnum!(ruse_object_graph::Number::from(num))),
                None => Err(parse_err!(value, "Value is not a number")),
            },
            TaskType::NumberArray => {
                let numbers = match value.as_array() {
                    Some(value_array) => {
                        if value_array.iter().any(|x| !x.is_i64()) {
                            return Err(parse_err!(
                                value,
                                "Value is an array with an invalid number value"
                            ));
                        }
                        value_array
                            .iter()
                            .map(|x| Number::from(x.as_i64().unwrap()))
                    }
                    None => return Err(parse_err!(value, "Value is not an array")),
                };
                let node = graph.add_primitive_array_object(
                    id_gen.get_id_for_node(),
                    &ValueType::Number,
                    numbers,
                    cache,
                );

                Ok(vobj!(graph.id, node))
            }
            TaskType::Bool => match value.as_bool() {
                Some(b) => Ok(vbool!(b)),
                None => Err(parse_err!(value, "Value is not a boolean")),
            },
            TaskType::String => match value.as_str() {
                Some(s) => Ok(vstr!(cache; s)),
                None => Err(parse_err!(value, "Value is not a string")),
            },
            TaskType::StringArray => {
                let strings = match value.as_array() {
                    Some(value_array) => {
                        if value_array.iter().any(|x| !x.is_string()) {
                            return Err(parse_err!(
                                value,
                                "Value is an array with an invalid string value"
                            ));
                        }
                        value_array
                            .iter()
                            .map(|x| str_cached!(cache; x.as_str().unwrap()))
                    }
                    None => return Err(parse_err!(value, "Value is not an array")),
                };
                let node = graph.add_primitive_array_object(
                    id_gen.get_id_for_node(),
                    &ValueType::String,
                    strings,
                    cache,
                );

                Ok(vobj!(graph.id, node))
            }
            TaskType::NumberSet => {
                let numbers = match value.as_array() {
                    Some(value_array) => {
                        if value_array.iter().any(|x| !x.is_i64()) {
                            return Err(parse_err!(
                                value,
                                "Value is an array with an invalid number value"
                            ));
                        }
                        value_array
                            .iter()
                            .map(|x| Number::from(x.as_i64().unwrap()))
                    }
                    None => return Err(parse_err!(value, "Value is not an array")),
                };
                let node = graph.add_primitive_set_object(
                    id_gen.get_id_for_node(),
                    &ValueType::Number,
                    numbers.unique(),
                    cache,
                );

                Ok(vobj!(graph.id, node))
            }
            TaskType::StringSet => {
                let strings = match value.as_array() {
                    Some(value_array) => {
                        if value_array.iter().any(|x| !x.is_string()) {
                            return Err(parse_err!(
                                value,
                                "Value is an array with an invalid string value"
                            ));
                        }
                        value_array
                            .iter()
                            .map(|x| str_cached!(cache; x.as_str().unwrap()))
                    }
                    None => return Err(parse_err!(value, "Value is not an array")),
                };
                let node = graph.add_primitive_set_object(
                    id_gen.get_id_for_node(),
                    &ValueType::String,
                    strings.unique(),
                    cache,
                );

                Ok(vobj!(graph.id, node))
            }
            TaskType::Object(s) => {
                let class_name = str_cached!(cache; s);
                let class = match classes.get_class(&class_name) {
                    Some(v) => v,
                    None => return Err(verify_err!(format!("object type {} is not defined", s))),
                };
                let fields = match value.as_object() {
                    Some(obj) => obj,
                    None => return Err(parse_err!(value, "Value is not an object value")),
                };
                let fields_types = HashMap::from_iter(
                    class
                        .fields
                        .iter()
                        .map(|(k, v)| (k.to_string(), TaskType::from(v))),
                );
                let values =
                    parse_json_values(&fields, &fields_types, graphs_map, id_gen, classes, cache)?;

                let obj = class.generate_object(values, graph, id_gen);

                Ok(Value::Object(obj))
            }
            TaskType::Dom => {
                let html = match value.as_str() {
                    Some(str) => str,
                    None => return Err(parse_err!(value, "Value for html is not an string value")),
                };
                let dom_value = match dom::DomLoader::load_dom(id_gen, graph, html, cache) {
                    Ok(v) => v,
                    Err(e) => return Err(parse_err!(value, e)),
                };

                Ok(vobj!(graph.id, dom_value))
            }
            TaskType::DOMElement => {
                let html = match value.as_str() {
                    Some(str) => str,
                    None => return Err(parse_err!(value, "Value for html is not an string value")),
                };
                let dom_value = match dom::DomLoader::load_element(id_gen, graph, html, cache) {
                    Ok(v) => v,
                    Err(e) => return Err(parse_err!(value, e)),
                };

                Ok(vobj!(graph.id, dom_value))
            }
            TaskType::VarRef(_) => Err(parse_err!(value, "Var ref is a delayed type")),
        }
    }

    fn is_var_ref(&self) -> bool {
        match self {
            TaskType::VarRef(_) => true,
            _ => false,
        }
    }

    fn is_object(&self) -> bool {
        match self {
            TaskType::Object(_) => true,
            _ => false,
        }
    }

    fn strip_string(value_string: &str) -> Result<&str, SnythesisTaskError> {
        let stripped = match value_string.trim().strip_prefix('\'') {
            Some(s1) => match s1.strip_suffix('\'') {
                Some(s2) => s2,
                None => return Err(parse_err!(value_string, "String suffix is missing")),
            },
            None => return Err(parse_err!(value_string, "String prefix is missing")),
        };
        Ok(stripped)
    }

    fn parse_string(value_string: &str) -> Result<serde_json::Value, SnythesisTaskError> {
        Ok(json!(Self::strip_string(value_string)?))
    }

    fn parse_string_array(value_string: &str) -> Result<serde_json::Value, SnythesisTaskError> {
        let mut part = String::new();
        let mut collected = Vec::new();

        let mut char_iter = value_string.chars();

        if char_iter.next() != Some('[') {
            return Err(parse_err!(value_string, "Missing opening bracket"));
        }

        loop {
            match char_iter
                .next()
                .ok_or(parse_err!(value_string, "Missing closing bracket"))?
            {
                ']' => {
                    if !part.is_empty() {
                        collected.push(Self::strip_string(&part)?.to_string());
                    }
                    return Ok(json!(collected));
                }
                ',' => {
                    if !part.is_empty() {
                        collected.push(Self::strip_string(&part)?.to_string());
                        part = String::new();
                    }
                }
                x => part.push(x),
            }
        }
    }

    fn parse_string_set(value_string: &str) -> Result<serde_json::Value, SnythesisTaskError> {
        let mut part = String::new();
        let mut collected = Vec::new();

        let mut char_iter = value_string.chars();

        if char_iter.next() != Some('{') {
            return Err(parse_err!(value_string, "Missing opening bracket"));
        }

        loop {
            match char_iter
                .next()
                .ok_or(parse_err!(value_string, "Missing closing bracket"))?
            {
                '}' => {
                    if !part.is_empty() {
                        collected.push(Self::strip_string(&part)?.to_string());
                    }
                    return Ok(json!(collected));
                }
                ',' => {
                    if !part.is_empty() {
                        collected.push(Self::strip_string(&part)?.to_string());
                        part = String::new();
                    }
                }
                x => part.push(x),
            }
        }
    }

    fn parse_number_set(value_string: &str) -> Result<serde_json::Value, SnythesisTaskError> {
        if !value_string.starts_with('{') {
            return Err(parse_err!(value_string, "Missing opening bracket"));
        }
        if !value_string.ends_with('}') {
            return Err(parse_err!(value_string, "Missing closing bracket"));
        }
        let array_string = value_string.replace('{', "[").replace('}', "]");
        match serde_json::from_str::<Vec<i64>>(array_string.as_str()) {
            Ok(numbers) => Ok(json!(numbers)),
            Err(e) => Err(parse_err!(value_string, e)),
        }
    }

    fn json_value_from_string(
        &self,
        value_string: &str,
    ) -> Result<serde_json::Value, SnythesisTaskError> {
        match self {
            TaskType::Number => match value_string.parse::<i64>() {
                Ok(num) => Ok(json!(num)),
                Err(e) => Err(parse_err!(value_string, e)),
            },
            TaskType::NumberArray => match serde_json::from_str::<Vec<i64>>(value_string) {
                Ok(numbers) => Ok(json!(numbers)),
                Err(e) => Err(parse_err!(value_string, e)),
            },
            TaskType::Bool => match value_string.parse::<bool>() {
                Ok(b) => Ok(json!(b)),
                Err(e) => Err(parse_err!(value_string, e)),
            },
            TaskType::String => Self::parse_string(value_string),
            TaskType::StringArray => Self::parse_string_array(value_string),
            TaskType::NumberSet => Self::parse_number_set(value_string),
            TaskType::StringSet => Self::parse_string_set(value_string),
            TaskType::Dom => Ok(json!(value_string)),
            TaskType::DOMElement => Ok(json!(value_string)),
            TaskType::VarRef(_) => Err(parse_err!(
                value_string,
                "Doesn't support converting from string to object value"
            )),
            TaskType::Object(_) => Err(parse_err!(
                value_string,
                "Doesn't support converting from string to object value"
            )),
        }
    }
}

impl From<&ValueType> for TaskType {
    fn from(value: &ValueType) -> Self {
        match value {
            ValueType::Number => TaskType::Number,
            ValueType::Bool => TaskType::Bool,
            ValueType::String => TaskType::String,
            ValueType::Object(o) => match o.as_str() {
                "Array<Number>" => TaskType::NumberArray,
                "Array<String>" => TaskType::StringArray,
                "Set<Number>" => TaskType::NumberSet,
                "Set<String>" => TaskType::StringSet,
                dom::DomLoader::DOM_CLASS_STR => TaskType::Dom,
                dom::DomLoader::ELEMENT_CLASS_STR => TaskType::DOMElement,
                s => TaskType::Object(s.to_owned()),
            },
        }
    }
}

impl<'de> Deserialize<'de> for TaskType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val = String::deserialize(deserializer)?;
        match val.as_str() {
            "Int" => Ok(TaskType::Number),
            "[Int]" => Ok(TaskType::NumberArray),
            "{Int}" => Ok(TaskType::NumberSet),
            "Bool" => Ok(TaskType::Bool),
            "String" => Ok(TaskType::String),
            "[String]" => Ok(TaskType::StringArray),
            "{String}" => Ok(TaskType::StringSet),
            "DOM" => Ok(TaskType::Dom),
            "DOMElement" => Ok(TaskType::DOMElement),
            _ => {
                if let Some(var_ref) = val.strip_prefix("VAR:") {
                    let mut iter = var_ref.split(".");
                    let var = iter.next().unwrap().to_string();
                    let fields = iter.map(|x| FieldName::from(x.to_string())).collect();
                    Ok(TaskType::VarRef(VarRef { var, fields }))
                } else {
                    Ok(TaskType::Object(val))
                }
            }
        }
    }
}

impl Serialize for TaskType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            TaskType::Number => serializer.serialize_str("Int"),
            TaskType::NumberArray => serializer.serialize_str("[Int]"),
            TaskType::Bool => serializer.serialize_str("Bool"),
            TaskType::String => serializer.serialize_str("String"),
            TaskType::StringArray => serializer.serialize_str("[String]"),
            TaskType::NumberSet => serializer.serialize_str("{Int}"),
            TaskType::StringSet => serializer.serialize_str("{String}"),
            TaskType::Object(o) => serializer.serialize_str(o),
            TaskType::VarRef(v) => serializer.serialize_str(&format!("{}", v)),
            TaskType::Dom => serializer.serialize_str("DOM"),
            TaskType::DOMElement => serializer.serialize_str("DOMElement"),
        }
    }
}

type JsonValuesMap = serde_json::Map<String, serde_json::Value>;

fn upgrade_values_map(
    map: &mut JsonValuesMap,
    types: &HashMap<String, TaskType>,
) -> Result<(), SnythesisTaskError> {
    for (k, v) in map.iter_mut() {
        let value_type = &match types.get(k) {
            Some(value_type) => value_type,
            None => return Err(verify_err!(format!("{} type is unknown", k))),
        };

        let value_str = v.as_str().ok_or(verify_err!(format!(
            "All values must be given as string in version 1"
        )))?;
        *v = value_type.json_value_from_string(value_str)?;
    }

    Ok(())
}

fn parse_json_values(
    map: &JsonValuesMap,
    types: &HashMap<String, TaskType>,
    graphs_map: &mut GraphsMap,
    id_gen: &GraphIdGenerator,
    classes: &TsClasses,
    cache: &Cache,
) -> Result<ValuesMap, SnythesisTaskError> {
    let mut values = ValuesMap::default();
    for (k, v) in map {
        let key = str_cached!(cache; k);
        let value_type = &match types.get(k) {
            Some(value_type) => value_type,
            None => return Err(verify_err!(format!("{} type is unknown", k))),
        };
        let mut graph = ObjectGraph::new(id_gen.get_id_for_graph());
        let mut value =
            value_type.create_value(v, &mut graph, classes, graphs_map, id_gen, cache)?;
        if let Some(obj) = value.mut_obj() {
            graph.set_as_root(key.clone(), obj.node);
        }
        graphs_map.insert_graph(graph.into());
        values.insert(key, value);
    }

    Ok(values)
}

#[derive(Deserialize, Serialize, Debug)]
struct SnythesisTaskExamples {
    input: JsonValuesMap,
    #[serde(skip_serializing_if = "Option::is_none")]
    output: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<JsonValuesMap>,
}

fn set_var_refs_variables(
    variables: &HashMap<String, TaskType>,
    values: &mut ValuesMap,
    graphs_map: &mut GraphsMap,
    cache: &Cache,
) -> Result<(), SnythesisTaskError> {
    let mut var_refs: VecDeque<_> = variables
        .iter()
        .filter_map(|(k, var_type)| {
            if let TaskType::VarRef(var_ref) = var_type {
                Some((str_cached!(cache; k), var_ref))
            } else {
                None
            }
        })
        .collect();

    while let Some((key, var_ref)) = var_refs.pop_front() {
        if !values.contains_key(&var_ref.var) {
            var_refs.push_back((key, var_ref));
            continue;
        }
        let value = var_ref.create_value(&values, graphs_map)?;
        values.insert(key, value);
    }

    Ok(())
}

impl SnythesisTaskExamples {
    fn upgrade_from_version_1(
        &mut self,
        variables: &HashMap<String, TaskType>,
        return_type: &Option<TaskType>,
    ) -> Result<(), SnythesisTaskError> {
        upgrade_values_map(&mut self.input, variables)?;
        if let Some(state) = &mut self.state {
            upgrade_values_map(state, variables)?;
        }
        if let Some(return_type) = return_type {
            let output = self.output.as_ref().unwrap();
            self.output = Some(return_type.json_value_from_string(output.as_str().unwrap())?)
        }

        Ok(())
    }

    fn create_context(
        &self,
        variables: &HashMap<String, TaskType>,
        classes: &TsClasses,
        cache: &Cache,
    ) -> Result<Context, SnythesisTaskError> {
        let id_gen = Arc::new(GraphIdGenerator::default());
        let mut graphs_map = GraphsMap::default();
        let mut values = parse_json_values(
            &self.input,
            variables,
            &mut graphs_map,
            &id_gen,
            classes,
            cache,
        )?;
        set_var_refs_variables(variables, &mut values, &mut graphs_map, cache)?;
        Ok(Context::with_values(values, graphs_map.into(), id_gen))
    }

    fn load_var_value(&mut self, dir: &Path, var: &str) -> Result<(), SnythesisTaskError> {
        self.input.insert(
            var.to_owned(),
            get_value_from_file_or_value(dir, &self.input[var])?,
        );
        if let Some(state) = &mut self.state {
            if state.contains_key(var) {
                state.insert(
                    var.to_owned(),
                    get_value_from_file_or_value(dir, &state[var])?,
                );
            }
        }

        Ok(())
    }

    fn load_output_value(&mut self, dir: &Path) -> Result<(), SnythesisTaskError> {
        self.output = Some(get_value_from_file_or_value(
            dir,
            self.output.as_ref().unwrap(),
        )?);

        Ok(())
    }
}

fn default_version() -> u32 {
    1
}

#[derive(Deserialize, Serialize, Debug)]
struct SnythesisTaskInner {
    #[serde(default = "default_version")]
    version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    variables: HashMap<String, TaskType>,
    #[serde(rename = "stringLiterals", skip_serializing_if = "Option::is_none")]
    string_literals: Option<Vec<String>>,
    #[serde(rename = "intLiterals", skip_serializing_if = "Option::is_none")]
    int_literals: Option<Vec<i64>>,
    #[serde(rename = "returnType", skip_serializing_if = "Option::is_none")]
    return_type: Option<TaskType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    classes: Option<Vec<String>>,
    #[serde(rename = "import", skip_serializing_if = "Option::is_none")]
    ts_files: Option<Vec<PathBuf>>,
    immutable: Option<HashSet<String>>,
    examples: Vec<SnythesisTaskExamples>,
    solution: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    opcodes: Option<HashSet<String>>,
}

impl SnythesisTaskInner {
    fn verify(&self) -> Result<(), SnythesisTaskError> {
        if let Some(immutable) = &self.immutable {
            if !immutable.iter().all(|x| self.variables.contains_key(x)) {
                return Err(verify_err!(
                    "Immutable contains a key which is not a variable"
                ));
            }
        }

        if self.examples.is_empty() {
            return Err(verify_err!("No examples were given"));
        }

        if self.return_type.is_some() && self.examples.iter().any(|x| x.output.is_none()) {
            return Err(verify_err!(
                "All examples should have an output if the return type is given"
            ));
        }

        if self.examples.iter().any(|x| x.output.is_some()) && self.return_type.is_none() {
            return Err(verify_err!(
                "Can't give example outputs without a return type"
            ));
        }

        if self.return_type.is_none() && self.examples.iter().any(|x| x.state.is_none()) {
            return Err(verify_err!(
                "All examples should have a state if the return type is not given"
            ));
        }

        if !(self.examples.iter().all(|x| x.state.is_some())
            || self.examples.iter().all(|x| x.state.is_none()))
        {
            return Err(verify_err!(
                "All examples should either have a state predicate or none of them"
            ));
        }

        if self.version == 1 {
            if self.classes.is_some() {
                return Err(verify_err!("classes is only supported from .sy version 2"));
            }
            if self.ts_files.is_some() {
                return Err(verify_err!("import is only supported from .sy version 2"));
            }

            if let Some(return_type) = &self.return_type {
                if return_type.is_object() {
                    return Err(verify_err!(
                        "Object type is only supported from .sy version 2"
                    ));
                }
                if return_type.is_var_ref() {
                    return Err(verify_err!(
                        "Var Ref type is only supported from .sy version 2"
                    ));
                }
            }
            if self.variables.iter().any(|var| var.1.is_object()) {
                return Err(verify_err!(
                    "Object type is only supported from .sy version 2"
                ));
            }
            if self.variables.iter().any(|var| var.1.is_var_ref()) {
                return Err(verify_err!(
                    "Var Ref type is only supported from .sy version 2"
                ));
            }
        }

        for (var, var_type) in &self.variables {
            if var == "document" && var_type != &TaskType::Dom {
                return Err(verify_err!("document variable must be of type DOM"));
            }
            if var != "document" && var_type == &TaskType::Dom {
                return Err(verify_err!("Only the document variable can be of type DOM"));
            }

            if var_type.is_var_ref() {
                self.verify_no_var_ref_circle(var, &self.variables)?;
            }
        }

        if !(self.examples.iter().all(|x| {
            self.variables
                .iter()
                .filter(|(_, t)| !t.is_var_ref())
                .all(|(k, _)| x.input.contains_key(k))
        })) {
            return Err(verify_err!(
                "All examples should contain values for all non-var-ref variables"
            ));
        }

        Ok(())
    }

    fn upgrade_from_version_1(&mut self) -> Result<(), SnythesisTaskError> {
        for example in self.examples.iter_mut() {
            example.upgrade_from_version_1(&self.variables, &self.return_type)?
        }

        Ok(())
    }

    fn verify_no_var_ref_circle(
        &self,
        var: &str,
        variables: &HashMap<String, TaskType>,
    ) -> Result<(), SnythesisTaskError> {
        let mut count = 0;
        let mut cur_var = var.to_string();
        while let TaskType::VarRef(var_ref) = &variables[&cur_var] {
            cur_var = var_ref.var.to_string();
            count += 1;
            if count > variables.len() {
                return Err(verify_err!("There is a variable reference loop"));
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct SnythesisTask {
    inner: SnythesisTaskInner,
    classes: TsClasses,
    class_names: Vec<ObjectType>,
    string_literals: HashSet<String, BuildHasherDefault<DefaultHasher>>,
    num_literals: HashSet<i64, BuildHasherDefault<DefaultHasher>>,
}

impl SnythesisTask {
    const DEFAULT_STRING_LITERALS: [&str; 2] = ["", " "];
    const DEFAULT_NUM_LITERALS: [i64; 2] = [0, 1];

    pub fn get_synthesizer(
        &self,
        mut max_context_depth: usize,
        iteration_workers_count: usize,
        cache: &Arc<Cache>,
    ) -> Result<TsSynthesizer, SnythesisTaskError> {
        let opcodes = self.get_opcodes(cache);
        let context_array = self.get_context_array(cache)?;
        let predicate = self.get_predicate(cache)?;
        let valid = self.get_valid_predicate(cache)?;
        if let Some(immutable) = &self.inner.immutable {
            if immutable.len() == self.inner.variables.len() {
                max_context_depth = 1;
            }
        }

        let mut synthesizer = TsSynthesizer::new(
            context_array,
            opcodes,
            predicate,
            valid,
            max_context_depth,
            iteration_workers_count,
            cache.clone(),
        );
        if let Some(immutable) = &self.inner.immutable {
            for var in immutable {
                synthesizer.set_immutable(&str_cached!(cache; var));
            }
        }

        Ok(synthesizer)
    }

    fn get_predicate(&self, cache: &Cache) -> Result<SynthesizerPredicate, SnythesisTaskError> {
        let mut predicate_graphs_map = GraphsMap::default();
        let predicate_gen_id = GraphIdGenerator::default();
        let root_name = cache.output_root_name();
        let output_array = match &self.inner.return_type {
            Some(return_type) => {
                let mut array = Vec::with_capacity(self.inner.examples.len());
                for example in &self.inner.examples {
                    let mut graph = ObjectGraph::new(predicate_gen_id.get_id_for_graph());
                    let mut output = return_type.create_value(
                        example.output.as_ref().unwrap(),
                        &mut graph,
                        &self.classes,
                        &mut predicate_graphs_map,
                        &predicate_gen_id,
                        cache,
                    )?;
                    if let Some(obj) = output.mut_obj() {
                        graph.set_as_root(root_name.clone(), obj.node);
                        predicate_graphs_map.insert_graph(graph.into());
                    }
                    array.push(output);
                }
                Some(array)
            }
            None => None,
        };
        let state_array = match self.inner.examples[0].state {
            Some(_) => {
                let mut array = Vec::with_capacity(self.inner.examples.len());
                for example in &self.inner.examples {
                    let state_map = parse_json_values(
                        example.state.as_ref().unwrap(),
                        &self.inner.variables,
                        &mut predicate_graphs_map,
                        &predicate_gen_id,
                        &self.classes,
                        cache,
                    )?;
                    array.push(state_map);
                }
                Some(array)
            }
            None => None,
        };

        let predicate = Box::new(move |p: &Arc<SubProgram>| {
            if let Some(array) = &output_array {
                for (actual, actual_ctx, expected) in
                    izip!(p.out_value().iter(), p.post_ctx().iter(), array)
                {
                    if actual.val().wrap(&actual_ctx.graphs_map)
                        != expected.wrap(&predicate_graphs_map)
                    {
                        return false;
                    }
                }
            }

            if let Some(array) = &state_array {
                for (actual, expected) in p.post_ctx().iter().zip(array) {
                    for (var, value) in expected.iter() {
                        let actual_value = match actual.get_var_loc_value(var) {
                            None => return false,
                            Some(v) => v,
                        };
                        if actual_value.val().wrap(&actual.graphs_map)
                            != value.wrap(&predicate_graphs_map)
                        {
                            return false;
                        }
                    }
                }
            }

            true
        });

        Ok(predicate)
    }

    fn get_valid_predicate(
        &self,
        _cache: &Cache,
    ) -> Result<SynthesizerPredicate, SnythesisTaskError> {
        Ok(Box::new(move |_p| true))
    }

    fn add_classes(
        classes: &TsClasses,
        classes_code: &Vec<String>,
        cache: &Cache,
    ) -> Result<Vec<CachedString>, SnythesisTaskError> {
        let mut names = Vec::with_capacity(classes_code.len());
        for code in classes_code {
            match classes.add_class(code, cache) {
                Ok(class_name) => names.push(class_name),
                Err(e) => {
                    return Err(parse_err!(code, e));
                }
            }
        }

        Ok(names)
    }

    pub fn add_classes_from_ts_files<I>(
        classes: &TsClasses,
        ts_file_paths: I,
        cache: &Cache,
    ) -> Result<Vec<CachedString>, SnythesisTaskError>
    where
        I: std::iter::IntoIterator<Item = PathBuf>,
    {
        let mut names: Vec<CachedString> = vec![];
        for full_path in ts_file_paths {
            match classes.add_ts_file(&full_path, cache) {
                Ok(class_names) => names.extend(class_names),
                Err(e) => {
                    return Err(parse_err!(String::from(full_path.to_string_lossy()), e));
                }
            };
        }

        Ok(names)
    }

    fn get_opcodes(&self, cache: &Cache) -> OpcodesList {
        let var_names: Vec<Arc<String>> = self
            .inner
            .variables
            .keys()
            .map(|x| str_cached!(cache; x))
            .collect();

        let string_literals = self
            .string_literals
            .iter()
            .map(|x| str_cached!(cache; x.as_str()))
            .collect_vec();

        let mut opcodes =
            construct_opcode_list(&var_names, &self.num_literals, &string_literals, false);

        let composite_opcodes =
            Self::get_composite_opcodes(&self.classes, &self.class_names, &cache);

        opcodes.extend(composite_opcodes.into_iter().filter(self.get_filter()));

        opcodes
    }

    fn get_filter(&self) -> Box<dyn Fn(&Arc<dyn ExprOpcode>) -> bool> {
        if let Some(filter) = &self.inner.opcodes {
            let wildcard_filter = filter
                .iter()
                .map(|f| WildMatch::new_case_insensitive(f))
                .collect_vec();
            Box::new(move |op| wildcard_filter.iter().any(|wf| wf.matches(op.op_name())))
        } else {
            Box::new(move |_op| true)
        }
    }

    pub fn get_composite_opcodes(
        classes: &TsClasses,
        class_names: &Vec<ObjectType>,
        cache: &Cache,
    ) -> OpcodesList {
        let mut composite_opcodes = OpcodesList::new();
        add_num_opcodes(
            &mut composite_opcodes,
            &ALL_BIN_NUM_OPCODES,
            &ALL_UNARY_NUM_OPCODES,
            &ALL_UPDATE_NUM_OPCODES,
        );
        add_str_opcodes(&mut composite_opcodes, &ALL_BIN_STR_OPCODES);
        add_array_opcodes(
            &mut composite_opcodes,
            &[ValueType::Number, ValueType::String],
            cache,
        );
        add_dom_opcodes(&mut composite_opcodes, cache);
        
        add_set_opcodes(&mut composite_opcodes, &[ValueType::Number, ValueType::String], cache);
        
        for class_name in class_names {
            let class = classes.get_class(class_name).unwrap();
            composite_opcodes.extend_from_slice(&class.member_opcodes);
            composite_opcodes.extend_from_slice(&class.method_opcodes);
        }
        composite_opcodes
    }

    fn get_context_array(&self, cache: &Cache) -> Result<ContextArray, SnythesisTaskError> {
        let mut values = Vec::with_capacity(self.inner.examples.len());
        for example in &self.inner.examples {
            values.push(example.create_context(&self.inner.variables, &self.classes, cache)?);
        }

        Ok(values.into())
    }

    pub fn from_json_file(path: &Path, cache: &Cache) -> Result<SnythesisTask, SnythesisTaskError> {
        let reader = std::fs::File::open(path).map_err(|e| SnythesisTaskError::IO(e))?;
        let mut inner: SnythesisTaskInner = match serde_json::from_reader(reader) {
            Ok(val) => val,
            Err(e) => {
                return Err(parse_err!("json", e));
            }
        };
        inner.verify()?;

        let mut dir = PathBuf::from(path);
        dir.pop();

        let mut string_literals = HashSet::<_, BuildHasherDefault<DefaultHasher>>::from_iter(
            Self::DEFAULT_STRING_LITERALS.map(|x| x.to_string()),
        );
        if let Some(user_lit) = &inner.string_literals {
            string_literals.extend(user_lit.clone());
        }

        let mut num_literals =
            HashSet::<_, BuildHasherDefault<DefaultHasher>>::from_iter(Self::DEFAULT_NUM_LITERALS);
        if let Some(user_lit) = &inner.int_literals {
            num_literals.extend(user_lit.clone());
        }

        let classes = TsClasses::new();
        let mut class_names = vec![];
        if let Some(classes_code) = &inner.classes {
            class_names.extend(Self::add_classes(&classes, classes_code, cache)?);
        }

        if let Some(ts_files) = &inner.ts_files {
            class_names.extend(Self::add_classes_from_ts_files(
                &classes,
                ts_files
                    .iter()
                    .map(|rel_path| PathBuf::from_iter(&[&dir, rel_path])),
                cache,
            )?);
        }

        for (var, var_type) in &inner.variables {
            if let TaskType::Object(obj_type) = var_type {
                if classes.get_class(&str_cached!(cache; obj_type)).is_none() {
                    return Err(verify_err!(format!(
                        "Variable {} has an unknown object type {}",
                        var, obj_type
                    )));
                }
            }
        }

        for (var, var_type) in &inner.variables {
            if var_type != &TaskType::Dom && var_type != &TaskType::DOMElement {
                continue;
            };
            for example in &mut inner.examples {
                example.load_var_value(dir.as_path(), var)?;
            }
        }

        if inner.return_type == Some(TaskType::Dom)
            || inner.return_type == Some(TaskType::DOMElement)
        {
            for example in &mut inner.examples {
                example.load_output_value(dir.as_path())?;
            }
        }

        if inner.version == 1 {
            inner.upgrade_from_version_1()?;
        }

        Ok(Self {
            string_literals,
            num_literals,
            classes,
            class_names,
            inner,
        })
    }

    pub fn populate_results(&self, results: &mut BenchmarkResult) {
        results.set_literals(
            Vec::from_iter(self.string_literals.iter().cloned()),
            Vec::from_iter(self.num_literals.iter().cloned()),
        );
    }
}
