use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};

use ruse_object_graph::{scached, str_cached, Cache, CachedString};
use ruse_synthesizer::{
    context::{Context, ContextArray},
    prog::SubProgram,
    synthesizer::{OpcodesList, SynthesizerPredicate},
    value::{Value, ValueType},
    vbool, vcstring, vnum,
};
use ruse_ts_interpreter::{dom, ts_class::TsClasses};
use ruse_ts_synthesizer::{
    add_array_opcodes, add_dom_opcodes, add_num_opcodes, add_str_opcodes, construct_opcode_list,
    TsSynthesizer, ALL_BIN_NUM_OPCODES, ALL_BIN_STR_OPCODES, ALL_UNARY_NUM_OPCODES,
    ALL_UPDATE_NUM_OPCODES,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug)]
pub struct TodoError {
    pub to_implement: &'static str,
}

impl TodoError {
    pub fn new(to_implement: &'static str) -> Self {
        Self {
            to_implement: to_implement,
        }
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
    Verify(VerifyError),
    Parse(ParseError),
}

impl std::error::Error for SnythesisTaskError {}

macro_rules! parse_err {
    ($val:expr, $e:expr) => {
        $crate::task::SnythesisTaskError::Parse($crate::task::ParseError {
            value: $val.to_owned(),
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
            SnythesisTaskError::Verify(e) => write!(f, "{}", e),
            SnythesisTaskError::Parse(e) => write!(f, "{}", e),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum TaskType {
    Number,
    NumberArray,
    Bool,
    String,
    StringArray,
    NumberSet,
    StringSet,
    DOM,
    DOMElement,
    Object(String),
}

fn get_string_from_file_or_value(dir: &Path, value: &str) -> Result<String, SnythesisTaskError> {
    let mut full_path = PathBuf::from(dir);
    full_path.push(value);
    if let Ok(mut html_file) = std::fs::File::open(full_path) {
        let mut buf = String::new();
        if let Err(e) = html_file.read_to_string(&mut buf) {
            return Err(parse_err!(value, e));
        }
        Ok(buf)
    } else {
        Ok(value.to_owned())
    }
}

fn parse_string(value: &str, cache: &Cache) -> Result<CachedString, SnythesisTaskError> {
    let stripped = match value.strip_prefix("'") {
        Some(s1) => match s1.strip_suffix("'") {
            Some(s2) => s2,
            None => return Err(parse_err!(value, "String suffix is missing")),
        },
        None => return Err(parse_err!(value, "String prefix is missing")),
    };
    Ok(str_cached!(cache; stripped))
}

impl TaskType {
    pub fn create_value(
        &self,
        value: &str,
        classes: &TsClasses,
        cache: &Cache,
    ) -> Result<Value, SnythesisTaskError> {
        match self {
            TaskType::Number => match value.parse::<u64>() {
                Ok(num) => Ok(vnum!(ruse_object_graph::Number::from(num))),
                Err(e) => Err(parse_err!(value, e)),
            },
            TaskType::NumberArray => {
                let numbers: Vec<u64> = match serde_json::from_str(value) {
                    Ok(v) => v,
                    Err(e) => {
                        return Err(parse_err!(value, e));
                    }
                };
                Ok(Value::create_primitive_array_object(
                    &ValueType::Number,
                    numbers,
                    cache,
                ))
            }
            TaskType::Bool => match value.parse::<bool>() {
                Ok(b) => Ok(vbool!(b)),
                Err(e) => Err(parse_err!(value, e)),
            },
            TaskType::String => Ok(vcstring!(parse_string(value, cache)?)),
            TaskType::StringArray => {
                let strings: Vec<String> = match serde_json::from_str(value) {
                    Ok(v) => v,
                    Err(e) => return Err(parse_err!(value, e)),
                };
                let mut values = Vec::with_capacity(strings.len());
                for string in strings {
                    values.push(parse_string(&string, cache)?)
                }

                Ok(Value::create_primitive_array_object(
                    &ValueType::String,
                    values,
                    cache,
                ))
            }
            TaskType::NumberSet => Err(parse_err!(value, TodoError::new("Number set"))),
            TaskType::StringSet => Err(parse_err!(value, TodoError::new("String set"))),
            TaskType::Object(s) => {
                let class_name = str_cached!(cache; s);
                let class = match classes.get_class(&class_name) {
                    Some(v) => v,
                    None => return Err(verify_err!(format!("object type {} is not defined", s))),
                };
                let strings: HashMap<String, String> = match serde_json::from_str(value) {
                    Ok(v) => v,
                    Err(e) => {
                        return Err(parse_err!(value, e));
                    }
                };
                let mut values = HashMap::<CachedString, Value>::with_capacity(strings.capacity());
                for (field, str_value) in strings {
                    let cached_field_name = scached!(cache; field);
                    let val = match class.fields.get(&cached_field_name) {
                        Some(v) => v,
                        None => {
                            return Err(verify_err!(format!(
                                "object type {} has no field {}",
                                s, &cached_field_name
                            )));
                        }
                    };
                    let field_value =
                        TaskType::from(val).create_value(str_value.as_str(), classes, cache)?;
                    values.insert(cached_field_name, field_value);
                }

                Ok(class.generate_object(values))
            }
            TaskType::DOM => {
                let dom_value = match dom::DomLoader::load_dom(value, cache) {
                    Ok(v) => v,
                    Err(e) => return Err(parse_err!(value, e)),
                };

                Ok(Value::Object(dom_value))
            }
            TaskType::DOMElement => {
                let dom_value = match dom::DomLoader::load_element(value, cache) {
                    Ok(v) => v,
                    Err(e) => return Err(parse_err!(value, e)),
                };

                Ok(Value::Object(dom_value))
            }
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
                dom::DomLoader::DOM_CLASS_STR => TaskType::DOM,
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
            "DOM" => Ok(TaskType::DOM),
            "DOMElement" => Ok(TaskType::DOMElement),
            _ => Ok(TaskType::Object(val)),
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
            TaskType::Object(o) => serializer.serialize_str(&format!("{}", o)),
            TaskType::DOM => serializer.serialize_str(&format!("{}", "DOM")),
            TaskType::DOMElement => serializer.serialize_str(&format!("{}", "DOMElement")),
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct SnythesisTaskExamples {
    input: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<HashMap<String, String>>,
}

fn string_map_to_value_map(
    map: &HashMap<String, String>,
    variables: &HashMap<String, TaskType>,
    classes: &TsClasses,
    cache: &Cache,
) -> Result<HashMap<CachedString, Value>, SnythesisTaskError> {
    let mut values = HashMap::with_capacity(map.len());
    for (k, v) in map {
        let key = str_cached!(cache; k);
        let value_type = variables.get(k).unwrap();
        let mut value = value_type.create_value(v, classes, cache)?;
        if let Some(obj) = value.mut_obj() {
            obj.set_as_graph_root(key.clone());
        }
        values.insert(key, value);
    }

    Ok(values)
}

impl SnythesisTaskExamples {
    fn create_context(
        &self,
        variables: &HashMap<String, TaskType>,
        classes: &TsClasses,
        cache: &Cache,
    ) -> Result<Context, SnythesisTaskError> {
        let values = string_map_to_value_map(&self.input, variables, classes, cache)?;
        Ok(Context::with_values(values))
    }

    fn load_var_value(&mut self, dir: &Path, var: &str) -> Result<(), SnythesisTaskError> {
        self.input.insert(
            var.to_owned(),
            get_string_from_file_or_value(dir, &self.input[var])?,
        );
        if let Some(state) = &mut self.state {
            if state.contains_key(var) {
                state.insert(
                    var.to_owned(),
                    get_string_from_file_or_value(dir, &state[var])?,
                );
            }
        }

        Ok(())
    }

    fn load_output_value(&mut self, dir: &Path) -> Result<(), SnythesisTaskError> {
        self.output = Some(get_string_from_file_or_value(
            dir,
            self.output.as_ref().unwrap(),
        )?);

        Ok(())
    }
}

#[derive(Deserialize, Serialize, Debug)]
struct SnythesisTaskInner {
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

        for (var, var_type) in &self.variables {
            if var == "document" && var_type != &TaskType::DOM {
                return Err(verify_err!("document variable must be of type DOM"));
            }
            if var != "document" && var_type == &TaskType::DOM {
                return Err(verify_err!("Only the document variable can be of type DOM"));
            }
        }

        return Ok(());
    }
}

#[derive(Debug)]
pub struct SnythesisTask {
    inner: SnythesisTaskInner,
    classes: TsClasses,
    class_names: Vec<CachedString>,
}

impl SnythesisTask {
    pub fn get_synthesizer(&self, cache: &Arc<Cache>) -> Result<TsSynthesizer, SnythesisTaskError> {
        let opcodes = self.get_opcodes(cache);
        let context_array = self.get_context_array(cache)?;
        let predicate = self.get_predicate(cache)?;
        let valid = self.get_valid_predicate(cache)?;
        let mut max_context_depth = 4;
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
            cache.clone()
        );
        if let Some(immutable) = &self.inner.immutable {
            for var in immutable {
                synthesizer.set_immutable(&str_cached!(cache; var));
            }
        }

        Ok(synthesizer)
    }

    fn get_predicate(&self, cache: &Cache) -> Result<SynthesizerPredicate, SnythesisTaskError> {
        let root_name = cache.temp_string();
        let output_array = match &self.inner.return_type {
            Some(return_type) => {
                let mut array = Vec::with_capacity(self.inner.examples.len());
                for example in &self.inner.examples {
                    let mut output = return_type.create_value(
                        example.output.as_ref().unwrap(),
                        &self.classes,
                        cache,
                    )?;
                    if let Some(obj) = output.mut_obj() {
                        obj.set_as_graph_root(root_name.clone());
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
                    let state_map = string_map_to_value_map(
                        example.state.as_ref().unwrap(),
                        &self.inner.variables,
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
                for (actual, expected) in p.out_value().iter().zip(array) {
                    if actual.val() != expected {
                        return false;
                    }
                }
            }

            if let Some(array) = &state_array {
                for (actual, expected) in p.post_ctx().iter().zip(array) {
                    for (var, value) in expected {
                        if actual.get_var_loc_value(var).val() != value {
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
        Ok(Box::new(move |_p| {
            // println!("{}", p);
            true
        }))
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

    fn add_classes_from_ts_files(
        classes: &TsClasses,
        root_path: &Path,
        ts_file_paths: &Vec<PathBuf>,
        cache: &Cache,
    ) -> Result<Vec<CachedString>, SnythesisTaskError> {
        let mut names: Vec<CachedString> = vec![];
        for ts_file_path in ts_file_paths {
            let mut full_path = PathBuf::from(root_path);
            full_path.push(ts_file_path);
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
        let num_literals = match &self.inner.int_literals {
            Some(literals) => literals.clone(),
            None => vec![0, 1],
        };
        let string_literals = match &self.inner.string_literals {
            Some(literals) => literals.iter().map(|x| str_cached!(cache; x)).collect(),
            None => vec![str_cached!(cache; " ")],
        };
        let mut opcodes = construct_opcode_list(&var_names, &num_literals, &string_literals, false);
        add_num_opcodes(
            &mut opcodes,
            &ALL_BIN_NUM_OPCODES,
            &ALL_UNARY_NUM_OPCODES,
            &ALL_UPDATE_NUM_OPCODES,
        );
        add_str_opcodes(&mut opcodes, &ALL_BIN_STR_OPCODES);
        add_array_opcodes(&mut opcodes, &[ValueType::Number, ValueType::String], cache);
        add_dom_opcodes(&mut opcodes, cache);

        for class_name in &self.class_names {
            let class = self.classes.get_class(class_name).unwrap();
            opcodes.extend_from_slice(&class.member_opcodes);
            opcodes.extend_from_slice(&class.method_opcodes);
        }

        opcodes
    }

    fn get_context_array(&self, cache: &Cache) -> Result<ContextArray, SnythesisTaskError> {
        let mut values = Vec::with_capacity(self.inner.examples.len());
        for example in &self.inner.examples {
            values.push(example.create_context(
                &self.inner.variables,
                &self.classes,
                cache,
            )?);
        }
        
        Ok(values.into())
    }

    pub fn from_json_file(path: &Path, cache: &Cache) -> Result<SnythesisTask, SnythesisTaskError> {
        let reader = std::fs::File::open(path).unwrap();
        let mut inner: SnythesisTaskInner = match serde_json::from_reader(reader) {
            Ok(val) => val,
            Err(e) => {
                return Err(parse_err!("json", e));
            }
        };
        inner.verify()?;

        let mut dir = PathBuf::from(path);
        dir.pop();

        let classes = TsClasses::new();
        let mut classes_names = vec![];
        if let Some(classes_code) = &inner.classes {
            classes_names.extend(Self::add_classes(&classes, classes_code, cache)?);
        }

        if let Some(ts_files) = &inner.ts_files {
            classes_names.extend(Self::add_classes_from_ts_files(
                &classes,
                dir.as_path(),
                ts_files,
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
            if var_type != &TaskType::DOM && var_type != &TaskType::DOMElement {
                continue;
            };
            for example in &mut inner.examples {
                example.load_var_value(dir.as_path(), var)?;
            }
        }

        if inner.return_type == Some(TaskType::DOM)
            || inner.return_type == Some(TaskType::DOMElement)
        {
            for example in &mut inner.examples {
                example.load_output_value(dir.as_path())?;
            }
        }

        Ok(Self {
            classes: classes,
            class_names: classes_names,
            inner: inner,
        })
    }
}
