use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt::{Debug, Display},
    fs,
    hash::{BuildHasherDefault, DefaultHasher},
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
    bank::{ProgBank, SubsumptionProgBank},
    context::{Context, ContextArray, GraphIdGenerator, SynthesizerContext, ValuesMap},
    opcode::{ExprOpcode, OpcodesList},
    prog::SubProgram,
    synthesizer::SynthesizerPredicate,
};
use ruse_ts_interpreter::{
    dom::{self, DomLoader},
    js_object_wrapper::EngineContext,
    js_value::value_to_js_value,
    ts_class::{TsClass, TsClasses, TsClassesBuilder},
};
use ruse_ts_synthesizer::*;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::json;
use wildmatch::WildMatch;

use crate::{config::BankType, results::BenchmarkResult};

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
        write!(f, "Verification error: {}", self.msg)
    }
}

#[derive(Debug)]
pub struct SkipError {
    pub reason: String,
}

impl From<&str> for SkipError {
    fn from(reason: &str) -> Self {
        SkipError {
            reason: reason.to_owned(),
        }
    }
}

impl std::error::Error for SkipError {}

impl std::fmt::Display for SkipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Skipped, reason: {}", self.reason)
    }
}

#[derive(Debug)]
pub enum SnythesisTaskError {
    IO(std::io::Error),
    Verify(VerifyError),
    Eval(boa_engine::JsError),
    Parse(ParseError),
    Skip(SkipError),
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

macro_rules! skip_err {
    ($reason:expr) => {
        $crate::task::SnythesisTaskError::Skip($crate::task::SkipError {
            reason: $reason.to_owned(),
        })
    };
}

impl std::fmt::Display for SnythesisTaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SnythesisTaskError::IO(e) => write!(f, "{}", e),
            SnythesisTaskError::Verify(e) => write!(f, "{}", e),
            SnythesisTaskError::Eval(e) => write!(f, "{}", e),
            SnythesisTaskError::Parse(e) => write!(f, "{}", e),
            SnythesisTaskError::Skip(e) => write!(f, "{}", e),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

impl From<&str> for VarRef {
    fn from(value: &str) -> Self {
        let mut iter = value.split(".");
        let var = iter.next().unwrap().to_string();
        let fields = iter.map(|x| FieldName::from(x.to_string())).collect();
        VarRef { var, fields }
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

#[derive(Debug, Clone, Copy)]
enum ExprPrefix {
    FieldRef,
    StaticRef,
    HtmlFile,
    StaticMethod,
    Null,
}

impl ExprPrefix {
    fn get<'a>(&self, expr: &'a str) -> Option<&'a str> {
        expr.strip_prefix(self.as_str())
    }

    fn as_str(&self) -> &str {
        match self {
            ExprPrefix::FieldRef => "*",
            ExprPrefix::StaticRef => "$",
            ExprPrefix::HtmlFile => "#",
            ExprPrefix::StaticMethod => "@",
            ExprPrefix::Null => "Null",
        }
    }
}

fn get_or_create_graph(graphs_map: &mut GraphsMap, graph_id: GraphIndex) -> &mut ObjectGraph {
    if !graphs_map.contains(graph_id) {
        graphs_map.insert_graph(ObjectGraph::new(graph_id).into());
    }
    Arc::make_mut(graphs_map.get_mut(&graph_id).unwrap())
}

#[derive(Debug, PartialEq, Eq)]
enum TaskType {
    Int,
    IntArray,
    IntSet,
    Double,
    DoubleArray,
    Bool,
    String,
    StringArray,
    StringSet,
    Dom,
    DOMElement,
    VarRef(VarRef),
    Object(String),
}

impl TaskType {
    pub fn create_value(
        &self,
        value: &serde_json::Value,
        classes: &TsClasses,
        graph_id: GraphIndex,
        graphs_map: &mut GraphsMap,
        id_gen: &Arc<GraphIdGenerator>,
        refs_graph_id: Option<GraphIndex>,
        cache: &Arc<Cache>,
    ) -> Result<Value, SnythesisTaskError> {
        if let Some(expr) = Self::get_expr_value(value) {
            self.parse_expr_value(expr, classes, graphs_map, refs_graph_id, id_gen, cache)
        } else {
            let out = self.create_regular_value(
                value,
                graph_id,
                classes,
                graphs_map,
                id_gen,
                refs_graph_id,
                cache,
            );

            out
        }
    }

    fn get_expr_value(value: &serde_json::Value) -> Option<&str> {
        if let Some(value_string) = value.as_str() {
            if !Self::is_string_value(value_string) {
                return Some(value_string);
            }
        }

        None
    }

    fn create_regular_value(
        &self,
        value: &serde_json::Value,
        graph_id: GraphIndex,
        classes: &TsClasses,
        graphs_map: &mut GraphsMap,
        id_gen: &Arc<GraphIdGenerator>,
        refs_graph_id: Option<GraphIndex>,
        cache: &Arc<Cache>,
    ) -> Result<Value, SnythesisTaskError> {
        match self {
            TaskType::Int => match value.as_i64() {
                Some(num) => Ok(vnum!(ruse_object_graph::Number::from(num))),
                None => Err(parse_err!(value, "Value is not a int")),
            },
            TaskType::IntArray => {
                let numbers = match value.as_array() {
                    Some(value_array) => {
                        if value_array.iter().any(|x| !x.is_i64()) {
                            return Err(parse_err!(
                                value,
                                "Value is an array with an invalid int value"
                            ));
                        }
                        value_array
                            .iter()
                            .map(|x| Number::from(x.as_i64().unwrap()))
                    }
                    None => return Err(parse_err!(value, "Value is not an array")),
                };
                let graph = get_or_create_graph(graphs_map, graph_id);
                let node = graph.add_primitive_array_object(
                    id_gen.get_id_for_node(),
                    &ValueType::Number,
                    numbers,
                    cache,
                );

                Ok(vobj!(
                    ValueType::array_obj_cached_string(&ValueType::Number, cache),
                    graph.id,
                    node
                ))
            }
            TaskType::Double => match value.as_f64() {
                Some(num) => Ok(vnum!(ruse_object_graph::Number::from(num))),
                None => Err(parse_err!(value, "Value is not a double")),
            },
            TaskType::DoubleArray => {
                let numbers = match value.as_array() {
                    Some(value_array) => {
                        if value_array.iter().any(|x| !x.is_f64()) {
                            return Err(parse_err!(
                                value,
                                "Value is an array with an invalid double value"
                            ));
                        }
                        value_array
                            .iter()
                            .map(|x| Number::from(x.as_f64().unwrap()))
                    }
                    None => return Err(parse_err!(value, "Value is not an array")),
                };
                let graph = get_or_create_graph(graphs_map, graph_id);
                let node = graph.add_primitive_array_object(
                    id_gen.get_id_for_node(),
                    &ValueType::Number,
                    numbers,
                    cache,
                );

                Ok(vobj!(
                    ValueType::array_obj_cached_string(&ValueType::Number, cache),
                    graph.id,
                    node
                ))
            }
            TaskType::Bool => match value.as_bool() {
                Some(b) => Ok(vbool!(b)),
                None => Err(parse_err!(value, "Value is not a boolean")),
            },
            TaskType::String => match value.as_str() {
                Some(s) => Ok(vstr!(cache; Self::strip_string(s)?)),
                None => Err(parse_err!(value, "Value is not a string")),
            },
            TaskType::StringArray => {
                let strings: Result<Vec<_>, _> = match value.as_array() {
                    Some(value_array) => {
                        if value_array.iter().any(|x| !x.is_string()) {
                            return Err(parse_err!(
                                value,
                                "Value is an array with an invalid string value"
                            ));
                        }

                        value_array
                            .iter()
                            .map(|x| {
                                Self::strip_string(x.as_str().unwrap())
                                    .map(|s| str_cached!(cache; s))
                            })
                            .collect()
                    }
                    None => return Err(parse_err!(value, "Value is not an array")),
                };
                let graph = get_or_create_graph(graphs_map, graph_id);
                let node = graph.add_primitive_array_object(
                    id_gen.get_id_for_node(),
                    &ValueType::String,
                    strings?,
                    cache,
                );

                Ok(vobj!(
                    ValueType::array_obj_cached_string(&ValueType::String, cache),
                    graph.id,
                    node
                ))
            }
            TaskType::IntSet => {
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
                let graph = get_or_create_graph(graphs_map, graph_id);
                let node = graph.add_primitive_set_object(
                    id_gen.get_id_for_node(),
                    &ValueType::Number,
                    numbers.unique(),
                    cache,
                );

                Ok(vobj!(
                    ValueType::set_obj_cached_string(&ValueType::Number, cache),
                    graph.id,
                    node
                ))
            }
            TaskType::StringSet => {
                let strings: Result<Vec<_>, _> = match value.as_array() {
                    Some(value_array) => {
                        if value_array.iter().any(|x| !x.is_string()) {
                            return Err(parse_err!(
                                value,
                                "Value is an array with an invalid string value"
                            ));
                        }
                        value_array
                            .iter()
                            .map(|x| {
                                Self::strip_string(x.as_str().unwrap())
                                    .map(|s| str_cached!(cache; s))
                            })
                            .collect()
                    }
                    None => return Err(parse_err!(value, "Value is not an array")),
                };
                let graph = get_or_create_graph(graphs_map, graph_id);
                let node = graph.add_primitive_set_object(
                    id_gen.get_id_for_node(),
                    &ValueType::String,
                    strings?.into_iter().unique(),
                    cache,
                );

                Ok(vobj!(
                    ValueType::set_obj_cached_string(&ValueType::String, cache),
                    graph.id,
                    node
                ))
            }
            TaskType::Object(s) => {
                let class_name = str_cached!(cache; s);
                let class = classes
                    .get_class(&class_name)
                    .ok_or(verify_err!(format!("object type {} is not defined", s)))?;
                let fields = value
                    .as_object()
                    .ok_or(parse_err!(value, "Value is not an object value"))?;

                if fields.contains_key(ExprPrefix::StaticMethod.as_str()) {
                    let method_name = fields[ExprPrefix::StaticMethod.as_str()]
                        .as_str()
                        .ok_or(parse_err!(value, "@ Static method value is not a string"))?;
                    let args_value = fields.get("args").ok_or(parse_err!(
                        value,
                        "@ Static method doesn't contain args field"
                    ))?;
                    let json_args = args_value.as_array().ok_or(parse_err!(
                        value,
                        "@ Static method args field is not an array"
                    ))?;
                    self.create_object_from_method(
                        class,
                        method_name,
                        json_args,
                        graph_id,
                        classes,
                        graphs_map,
                        id_gen,
                        cache,
                    )
                } else {
                    self.create_object_from_fields(
                        fields,
                        class,
                        graph_id,
                        classes,
                        graphs_map,
                        id_gen,
                        refs_graph_id,
                        cache,
                    )
                }
            }
            TaskType::Dom | TaskType::DOMElement => {
                let html = match value.as_str() {
                    Some(str) => Self::strip_string(str),
                    None => return Err(parse_err!(value, "Value for html is not an string value")),
                }?;
                let graph = get_or_create_graph(graphs_map, graph_id);
                self.parse_dom(html, graph, id_gen, cache)
            }
            TaskType::VarRef(_) => Err(parse_err!(value, "Var ref is a delayed type")),
        }
    }

    fn create_object_from_fields(
        &self,
        fields: &JsonValuesMap,
        class: &TsClass,
        graph_id: GraphIndex,
        classes: &TsClasses,
        graphs_map: &mut GraphsMap,
        id_gen: &Arc<GraphIdGenerator>,
        refs_graph_id: Option<GraphIndex>,
        cache: &Arc<Cache>,
    ) -> Result<Value, SnythesisTaskError> {
        let fields_types = HashMap::from_iter(
            class
                .description
                .fields
                .iter()
                .map(|(k, v)| (k.to_string(), TaskType::from(&v.value_type))),
        );

        let values = parse_json_values_map(
            fields,
            &fields_types,
            graph_id,
            graphs_map,
            id_gen,
            refs_graph_id,
            classes,
            cache,
        )?;

        let graph = get_or_create_graph(graphs_map, graph_id);
        let obj = class.generate_object(values, graph, id_gen);

        Ok(Value::Object(obj))
    }

    fn create_object_from_method(
        &self,
        class: &TsClass,
        method_name: &str,
        json_args: &Vec<serde_json::Value>,
        graph_id: GraphIndex,
        classes: &TsClasses,
        graphs_map: &mut GraphsMap,
        id_gen: &Arc<GraphIdGenerator>,
        cache: &Arc<Cache>,
    ) -> Result<Value, SnythesisTaskError> {
        let method_desc = if method_name == "constructor" {
            &class.description.constructor
        } else {
            let desc = class
                .description
                .methods
                .get(method_name)
                .ok_or(verify_err!(format!(
                    "object {} has no method {}",
                    &class.description.class_name, method_name
                )))?;
            if !desc.is_static {
                return Err(verify_err!(format!(
                    "{}.{} is not static",
                    &class.description.class_name, method_name
                )));
            } else {
                desc
            }
        };
        let arg_types = method_desc.params.iter().map(|x| Self::from(&x.value_type));
        let args = parse_json_values_array(
            json_args, arg_types, graph_id, graphs_map, id_gen, classes, None, cache,
        )?;

        let mut boa_ctx = EngineContext::new_boa_ctx();
        let mut engine_ctx = EngineContext::create_engine_ctx(&mut boa_ctx, classes);
        get_or_create_graph(graphs_map, graph_id); // Make sure graph exists
        engine_ctx.reset_with_graph(graph_id, graphs_map, classes, id_gen, cache);

        if method_name == "constructor" {
            let new_obj = class.call_constructor(&args, classes, &mut engine_ctx);
            Ok(Value::Object(new_obj))
        } else {
            class
                .call_static_method(method_name, &args, classes, cache, &mut engine_ctx)
                .map_err(|x| SnythesisTaskError::Eval(x))
        }
    }

    pub fn parse_dom(
        &self,
        html: &str,
        graph: &mut ObjectGraph,
        id_gen: &GraphIdGenerator,
        cache: &Cache,
    ) -> Result<Value, SnythesisTaskError> {
        match self {
            TaskType::Dom => {
                let dom_value = match dom::DomLoader::load_dom(id_gen, graph, html, cache) {
                    Ok(v) => v,
                    Err(e) => return Err(parse_err!(html, e)),
                };

                Ok(vobj!(
                    dom::DomLoader::document_obj_type(cache),
                    graph.id,
                    dom_value
                ))
            }
            TaskType::DOMElement => {
                let dom_value = match dom::DomLoader::load_element(id_gen, graph, html, cache) {
                    Ok(v) => v,
                    Err(e) => return Err(parse_err!(html, e)),
                };

                Ok(vobj!(
                    dom::DomLoader::element_obj_type(cache),
                    graph.id,
                    dom_value
                ))
            }
            _ => unreachable!("Not a dom type"),
        }
    }

    pub fn parse_expr_value(
        &self,
        value_string: &str,
        classes: &TsClasses,
        graphs_map: &mut GraphsMap,
        refs_graph_id_opt: Option<GraphIndex>,
        id_gen: &GraphIdGenerator,
        cache: &Cache,
    ) -> Result<Value, SnythesisTaskError> {
        if let Some(static_ref) = ExprPrefix::StaticRef.get(value_string) {
            self.parse_static_ref_expr_value(static_ref, value_string, classes, graphs_map, cache)
        } else if value_string == ExprPrefix::Null.as_str() {
            if self.is_object() {
                Ok(vnull!())
            } else {
                Err(parse_err!(
                    value_string,
                    format!("Only object types can be set to null")
                ))
            }
        } else if let Some(field_ref) = ExprPrefix::FieldRef.get(value_string) {
            let refs_graph_id = refs_graph_id_opt
                .ok_or(parse_err!(value_string, "Ref expr in an invalid location"))?;
            self.parse_field_ref_expr_value(
                field_ref,
                value_string,
                graphs_map,
                refs_graph_id,
                id_gen,
                cache,
            )
        } else {
            Err(parse_err!(
                value_string,
                format!("The expr {} has unknown prefix", value_string)
            ))
        }
    }

    fn parse_static_ref_expr_value(
        &self,
        static_ref: &str,
        value_string: &str,
        classes: &TsClasses,
        graphs_map: &mut GraphsMap,
        cache: &Cache,
    ) -> Result<Value, SnythesisTaskError> {
        let (class_name, field_name) = static_ref.split_once('.').ok_or(parse_err!(
            value_string,
            format!("The static ref expr contains no '.'")
        ))?;
        let class = classes
            .get_class(&str_cached!(cache; class_name))
            .ok_or(parse_err!(
                value_string,
                format!(
                    "The class {} is not defined, can't parse static field ref",
                    class_name
                )
            ))?;
        let field_desc = class.description.fields.get(field_name).ok_or(parse_err!(
            value_string,
            format!(
                "The field {} is not defined for class {}, can't parse static field ref",
                field_name, class_name
            )
        ))?;
        if !field_desc.is_static {
            return Err(parse_err!(
                value_string,
                format!(
                    "The field {} is not a static field for class {}, can't parse static field ref",
                    field_name, class_name
                )
            ));
        }
        let value = class
            .static_fields
            .get(&str_cached!(cache; field_name))
            .ok_or(parse_err!(
                value_string,
                format!(
                    "The field {} is not defined for class {}, can't parse static field ref",
                    field_name, class_name
                )
            ))?;
        if &field_desc.value_type != &self.value_type(cache) {
            return Err(parse_err!(
                value_string,
                format!(
                    "The static field {} is not of variable type {:?}",
                    static_ref, self
                )
            ));
        }
        graphs_map.insert_graph(class.static_graph.as_ref().unwrap().clone());
        Ok(value.val().clone())
    }

    fn parse_field_ref_expr_value(
        &self,
        field_ref: &str,
        value_string: &str,
        graphs_map: &mut GraphsMap,
        refs_graph_id: GraphIndex,
        id_gen: &GraphIdGenerator,
        cache: &Cache,
    ) -> Result<Value, SnythesisTaskError> {
        if !self.is_object() {
            return Err(parse_err!(
                value_string,
                format!("The field ref expression is only valid for object types")
            ));
        }

        let refs_graph = Arc::make_mut(graphs_map.get_mut(&refs_graph_id).unwrap());
        let obj_type = str_cached!(cache; REF_GRAPH_OBJ_TYPE);
        let ref_id = refs_graph.add_simple_object_from_fields_map(
            id_gen.get_id_for_node(),
            obj_type.clone(),
            fields!((
                str_cached!(cache; REF_GRAPH_FIELD_NAME),
                PrimitiveValue::String(str_cached!(cache; field_ref))
            )),
        );

        Ok(vobj!(obj_type, refs_graph_id, ref_id))
    }

    fn value_type(&self, cache: &Cache) -> ValueType {
        match self {
            TaskType::Int => ValueType::Number,
            TaskType::IntArray => ValueType::array_value_type(&ValueType::Number, cache),
            TaskType::IntSet => ValueType::set_value_type(&ValueType::Number, cache),
            TaskType::Double => ValueType::Number,
            TaskType::DoubleArray => ValueType::array_value_type(&ValueType::Number, cache),
            TaskType::Bool => ValueType::Bool,
            TaskType::String => ValueType::String,
            TaskType::StringArray => ValueType::array_value_type(&ValueType::String, cache),
            TaskType::StringSet => ValueType::set_value_type(&ValueType::String, cache),
            TaskType::Dom => ValueType::Object(DomLoader::document_obj_type(cache)),
            TaskType::DOMElement => ValueType::Object(DomLoader::element_obj_type(cache)),
            TaskType::VarRef(_var_ref) => todo!(),
            TaskType::Object(o) => ValueType::Object(str_cached!(cache; o)),
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

    fn is_string_value(value_string: &str) -> bool {
        value_string.starts_with('\'') && value_string.ends_with('\'')
    }

    fn parse_string_collection(
        value_string: &str,
        is_set: bool,
    ) -> Result<serde_json::Value, SnythesisTaskError> {
        let mut part = String::new();
        let mut collected = Vec::new();

        let mut char_iter = value_string.chars();

        let start = if is_set { '{' } else { '[' };
        let end = if is_set { '}' } else { ']' };

        if char_iter.next() != Some(start) {
            return Err(parse_err!(value_string, "Missing opening bracket"));
        }

        loop {
            match char_iter
                .next()
                .ok_or(parse_err!(value_string, "Missing closing bracket"))?
            {
                x if x == end => {
                    if !part.is_empty() {
                        collected.push(part.clone());
                    }
                    return Ok(json!(collected));
                }
                ',' => {
                    if !part.is_empty() {
                        collected.push(part.clone());
                        part = String::new();
                    }
                }
                x => part.push(x),
            }
        }
    }

    fn parse_int_set(value_string: &str) -> Result<serde_json::Value, SnythesisTaskError> {
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
            TaskType::Int => match value_string.parse::<i64>() {
                Ok(num) => Ok(json!(num)),
                Err(e) => Err(parse_err!(value_string, e)),
            },
            TaskType::IntArray => match serde_json::from_str::<Vec<i64>>(value_string) {
                Ok(numbers) => Ok(json!(numbers)),
                Err(e) => Err(parse_err!(value_string, e)),
            },
            TaskType::Double => match value_string.parse::<f64>() {
                Ok(num) => Ok(json!(num)),
                Err(e) => Err(parse_err!(value_string, e)),
            },
            TaskType::DoubleArray => match serde_json::from_str::<Vec<f64>>(value_string) {
                Ok(numbers) => Ok(json!(numbers)),
                Err(e) => Err(parse_err!(value_string, e)),
            },
            TaskType::Bool => match value_string.parse::<bool>() {
                Ok(b) => Ok(json!(b)),
                Err(e) => Err(parse_err!(value_string, e)),
            },
            TaskType::String => Ok(json!(value_string)),
            TaskType::StringArray => Self::parse_string_collection(value_string, false),
            TaskType::IntSet => Self::parse_int_set(value_string),
            TaskType::StringSet => Self::parse_string_collection(value_string, true),
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

    fn load_value(
        &self,
        dir: &Path,
        val: &mut serde_json::Value,
    ) -> Result<(), SnythesisTaskError> {
        if let Some(value_string) = val.as_str() {
            if let Some(html_path) = ExprPrefix::HtmlFile.get(value_string).map(PathBuf::from) {
                if html_path.is_relative() {
                    *val =
                        self.load_html_path_value(dir.join(html_path).as_path(), value_string)?;
                } else {
                    *val = self.load_html_path_value(html_path.as_path(), value_string)?;
                }
                return Ok(());
            }
        }

        Ok(())
    }

    fn load_html_path_value(
        &self,
        html_path: &Path,
        expr_value: &str,
    ) -> Result<serde_json::Value, SnythesisTaskError> {
        match self {
            TaskType::Dom | TaskType::DOMElement => {
                let data = fs::read(html_path).map_err(|_| {
                    parse_err!(
                        expr_value,
                        format!("{} is not a valid html file path", html_path.display())
                    )
                })?;
                let html = String::from_utf8(data).map_err(|_| {
                    parse_err!(
                        expr_value,
                        format!("Failed to parse {} text", html_path.display())
                    )
                })?;
                Ok(json!(format!("'{}'", html)))
            }
            _ => Err(parse_err!(
                expr_value,
                format!(
                    "The expr {} is only valid for DOM or DOMElement",
                    expr_value
                )
            )),
        }
    }
}

impl From<&ValueType> for TaskType {
    fn from(value: &ValueType) -> Self {
        match value {
            ValueType::Number => TaskType::Double,
            ValueType::Bool => TaskType::Bool,
            ValueType::String => TaskType::String,
            ValueType::Object(o) => match o.as_str() {
                "Array<Number>" => TaskType::DoubleArray,
                "Array<String>" => TaskType::StringArray,
                "Set<Number>" => TaskType::IntSet,
                "Set<String>" => TaskType::StringSet,
                dom::DomLoader::DOM_CLASS_STR => TaskType::Dom,
                dom::DomLoader::ELEMENT_CLASS_STR => TaskType::DOMElement,
                s => TaskType::Object(s.to_owned()),
            },
            ValueType::Null => unreachable!(),
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
            "Int" => Ok(TaskType::Int),
            "[Int]" => Ok(TaskType::IntArray),
            "{Int}" => Ok(TaskType::IntSet),
            "Double" => Ok(TaskType::Double),
            "[Double]" => Ok(TaskType::DoubleArray),
            "Bool" => Ok(TaskType::Bool),
            "String" => Ok(TaskType::String),
            "[String]" => Ok(TaskType::StringArray),
            "{String}" => Ok(TaskType::StringSet),
            "DOM" => Ok(TaskType::Dom),
            "DOMElement" => Ok(TaskType::DOMElement),
            _ => {
                if let Some(var_ref) = ExprPrefix::FieldRef.get(&val) {
                    Ok(TaskType::VarRef(VarRef::from(var_ref)))
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
            TaskType::Int => serializer.serialize_str("Int"),
            TaskType::IntArray => serializer.serialize_str("[Int]"),
            TaskType::Double => serializer.serialize_str("Double"),
            TaskType::DoubleArray => serializer.serialize_str("[Double]"),
            TaskType::Bool => serializer.serialize_str("Bool"),
            TaskType::String => serializer.serialize_str("String"),
            TaskType::StringArray => serializer.serialize_str("[String]"),
            TaskType::IntSet => serializer.serialize_str("{Int}"),
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

fn parse_json_values_map_roots<'a, M>(
    map: M,
    types: &HashMap<String, TaskType>,
    graphs_map: &mut GraphsMap,
    id_gen: &Arc<GraphIdGenerator>,
    refs_graph_id: Option<GraphIndex>,
    classes: &TsClasses,
    cache: &Arc<Cache>,
) -> Result<ValuesMap, SnythesisTaskError>
where
    M: IntoIterator<Item = (&'a String, &'a serde_json::Value)>,
{
    let mut values = ValuesMap::default();
    for (k, v) in map {
        let key = str_cached!(cache; k);
        let value_type = &match types.get(k) {
            Some(value_type) => value_type,
            None => return Err(verify_err!(format!("{} type is unknown", k))),
        };
        let mut value = value_type.create_value(
            v,
            classes,
            id_gen.get_id_for_graph(),
            graphs_map,
            id_gen,
            refs_graph_id,
            cache,
        )?;

        if let Some(obj) = value.mut_obj() {
            let graph = graphs_map.get_mut(&obj.graph_id).unwrap();
            Arc::make_mut(graph).set_as_root(key.clone(), obj.node);
        }
        values.insert(key, value);
    }

    Ok(values)
}

fn parse_json_values_map<'a, M>(
    map: M,
    types: &HashMap<String, TaskType>,
    graph_id: GraphIndex,
    graphs_map: &mut GraphsMap,
    id_gen: &Arc<GraphIdGenerator>,
    refs_graph_id: Option<GraphIndex>,
    classes: &TsClasses,
    cache: &Arc<Cache>,
) -> Result<ValuesMap, SnythesisTaskError>
where
    M: IntoIterator<Item = (&'a String, &'a serde_json::Value)>,
{
    let mut values = ValuesMap::default();
    for (k, v) in map {
        let key = str_cached!(cache; k);
        let value_type = &match types.get(k) {
            Some(value_type) => value_type,
            None => return Err(verify_err!(format!("{} type is unknown", k))),
        };
        let value = value_type.create_value(
            v,
            classes,
            graph_id,
            graphs_map,
            id_gen,
            refs_graph_id,
            cache,
        )?;
        values.insert(key, value);
    }

    Ok(values)
}

fn parse_json_values_array<'a, V, T>(
    arr: V,
    types: T,
    graph_id: GraphIndex,
    graphs_map: &mut GraphsMap,
    id_gen: &Arc<GraphIdGenerator>,
    classes: &TsClasses,
    refs_graph_id: Option<GraphIndex>,
    cache: &Arc<Cache>,
) -> Result<Vec<Value>, SnythesisTaskError>
where
    V: IntoIterator<Item = &'a serde_json::Value>,
    T: IntoIterator<Item = TaskType>,
{
    let mut values = vec![];
    for (value, task_type) in arr.into_iter().zip_eq(types) {
        let value = task_type.create_value(
            value,
            classes,
            graph_id,
            graphs_map,
            id_gen,
            refs_graph_id,
            cache,
        )?;
        values.push(value);
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
    #[serde(skip_serializing_if = "Option::is_none")]
    predicate_js: Option<String>,
}

const REF_GRAPH_ID: usize = usize::MAX;
const REF_GRAPH_FIELD_NAME: &str = "ref";
const REF_GRAPH_OBJ_TYPE: &str = "VARREF";

fn set_var_refs(
    variables: &HashMap<String, TaskType>,
    values: &mut ValuesMap,
    graphs_map: &mut GraphsMap,
    cache: &Cache,
) -> Result<(), SnythesisTaskError> {
    let mut refs = Vec::new();
    for (graph, node_id, node) in graph_walk::ObjectGraphWalker::from_graphs(
        graphs_map,
        graphs_map
            .graph_indices()
            .map(|g_id| *g_id)
            .filter(|g_id| *g_id != REF_GRAPH_ID),
    ) {
        for edge in node.pointers_iter() {
            if let EdgeEndPoint::Chain(edge_graph_id, edge_node_id) = edge.1 {
                if *edge_graph_id == REF_GRAPH_ID {
                    let ref_graph = graphs_map.get(&REF_GRAPH_ID).unwrap();
                    let var_ref = ref_graph
                        .get_primitive_field(
                            edge_node_id,
                            &str_cached!(cache; REF_GRAPH_FIELD_NAME),
                        )
                        .unwrap()
                        .value
                        .string()
                        .unwrap();

                    refs.push((
                        graph.id,
                        node_id,
                        edge.0.clone(),
                        graph.get_root_name(node_id),
                        VarRef::from(var_ref.as_str()),
                    ));
                }
            }
        }
    }
    for (graph_id, node_id, field_name, root_name_opt, var_ref) in refs {
        let actual_value = var_ref.create_value(values, graphs_map)?;
        let graph = Arc::make_mut(graphs_map.get_mut(&graph_id).unwrap());
        let actual_obj = actual_value.obj().unwrap();

        if actual_obj.graph_id == graph_id {
            graph.set_edge(&node_id, actual_obj.node, field_name);
        } else {
            graph.set_chain_edge(&node_id, actual_obj.graph_id, actual_obj.node, field_name);
        }
        if let Some(root_name) = root_name_opt {
            graph.set_as_root(root_name.clone(), actual_obj.node);
            if values.contains_key(&root_name) {
                values.insert(root_name, Value::Object(actual_obj.clone()));
            }
        }
    }

    let mut var_refs: VecDeque<_> = variables
        .iter()
        .filter_map(|(k, var_type)| {
            if let TaskType::VarRef(var_ref) = var_type {
                Some((str_cached!(cache; k), var_ref.clone()))
            } else {
                None
            }
        })
        .collect();

    for (key, value) in values.iter_mut() {
        if let Value::Object(obj_val) = value {
            if obj_val.graph_id == REF_GRAPH_ID {
                let ref_graph = graphs_map.get(&REF_GRAPH_ID).unwrap();
                let var_ref = VarRef::from(
                    ref_graph
                        .get_primitive_field(
                            &obj_val.node,
                            &str_cached!(cache; REF_GRAPH_FIELD_NAME),
                        )
                        .unwrap()
                        .value
                        .string()
                        .unwrap()
                        .as_str(),
                );
                var_refs.push_back((key.clone(), var_ref));
            }
        }
    }

    while let Some((key, var_ref)) = var_refs.pop_front() {
        if !values.contains_key(&var_ref.var) {
            var_refs.push_back((key, var_ref));
            continue;
        }
        let value = var_ref.create_value(&values, graphs_map)?;
        if let Value::Object(obj_val) = &value {
            let graph = Arc::make_mut(graphs_map.get_mut(&obj_val.graph_id).unwrap());
            graph.set_as_root(key.clone(), obj_val.node);
        }
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
        cache: &Arc<Cache>,
    ) -> Result<Box<Context>, SnythesisTaskError> {
        let id_gen = Arc::new(GraphIdGenerator::with_initial_values(
            classes.static_classes_gen_id.max_node_id(),
            classes.static_classes_gen_id.max_graph_id(),
        ));
        let mut graphs_map = GraphsMap::default();
        graphs_map.insert_graph(ObjectGraph::new(REF_GRAPH_ID).into());

        let mut values = parse_json_values_map_roots(
            &self.input,
            variables,
            &mut graphs_map,
            &id_gen,
            Some(REF_GRAPH_ID),
            classes,
            cache,
        )?;
        set_var_refs(variables, &mut values, &mut graphs_map, cache)?;
        graphs_map.remove(REF_GRAPH_ID);
        Ok(Context::with_values(values, graphs_map.into(), id_gen))
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
    #[serde(skip_serializing_if = "Option::is_none")]
    skip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    variables: Option<HashMap<String, TaskType>>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    immutable: Option<HashSet<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    examples: Option<Vec<SnythesisTaskExamples>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    solution: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    opcodes: Option<HashSet<String>>,
}

impl SnythesisTaskInner {
    fn verify(&self) -> Result<(), SnythesisTaskError> {
        if let Some(reason) = &self.skip {
            return Err(skip_err!(reason));
        }

        if self.variables.is_none() {
            return Err(verify_err!("Non skipped tasks must contain variables dict"));
        }

        if self.examples.is_none() {
            return Err(verify_err!("Non skipped tasks must contain examples array"));
        }

        let variables = self.variables.as_ref().unwrap();
        let examples = self.examples.as_ref().unwrap();

        if let Some(immutable) = &self.immutable {
            if !immutable.iter().all(|x| variables.contains_key(x)) {
                return Err(verify_err!(
                    "Immutable contains a key which is not a variable"
                ));
            }
        }

        if examples.is_empty() {
            return Err(verify_err!("No examples were given"));
        }

        if self.return_type.is_some() && examples.iter().any(|x| x.output.is_none()) {
            return Err(verify_err!(
                "All examples should have an output if the return type is given"
            ));
        }

        if examples.iter().any(|x| x.output.is_some()) && self.return_type.is_none() {
            return Err(verify_err!(
                "Can't give example outputs without a return type"
            ));
        }

        if examples
            .iter()
            .any(|x| x.output.is_none() && x.state.is_none() && x.predicate_js.is_none())
        {
            return Err(verify_err!(
                "All examples must have at least one form of predicate for success (output, state, predicate_js)"
            ));
        }

        if !(examples.iter().all(|x| x.state.is_some())
            || examples.iter().all(|x| x.state.is_none()))
        {
            return Err(verify_err!(
                "All examples should either have a state predicate or none of them"
            ));
        }

        if !(examples.iter().all(|x| x.predicate_js.is_some())
            || examples.iter().all(|x| x.predicate_js.is_none()))
        {
            return Err(verify_err!(
                "All examples should either have a JS predicate or none of them"
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
            if variables.iter().any(|var| var.1.is_object()) {
                return Err(verify_err!(
                    "Object type is only supported from .sy version 2"
                ));
            }
            if variables.iter().any(|var| var.1.is_var_ref()) {
                return Err(verify_err!(
                    "Var Ref type is only supported from .sy version 2"
                ));
            }
        }

        for (var, var_type) in variables {
            if var == "document" && var_type != &TaskType::Dom {
                return Err(verify_err!("document variable must be of type DOM"));
            }
            if var != "document" && var_type == &TaskType::Dom {
                return Err(verify_err!("Only the document variable can be of type DOM"));
            }

            if var_type.is_var_ref() {
                self.verify_no_var_ref_circle(var, variables)?;
            }
        }

        if !(examples.iter().all(|x| {
            variables
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
        for example in self.examples.as_mut().unwrap().iter_mut() {
            example.upgrade_from_version_1(&self.variables.as_ref().unwrap(), &self.return_type)?
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
    classes: Box<TsClasses>,
    class_names: Vec<ObjectType>,
    string_literals: HashSet<String, BuildHasherDefault<DefaultHasher>>,
    num_literals: HashSet<i64, BuildHasherDefault<DefaultHasher>>,
}

impl SnythesisTask {
    const DEFAULT_STRING_LITERALS: [&str; 2] = ["", " "];
    const DEFAULT_NUM_LITERALS: [i64; 2] = [0, 1];

    pub fn get_synthesizer(
        self,
        mut max_context_depth: usize,
        iteration_workers_count: usize,
        bank_type: BankType,
        cache: &Arc<Cache>,
    ) -> Result<TsSynthesizer<impl ProgBank>, SnythesisTaskError> {
        let variables = self.inner.variables.as_ref().unwrap();

        let opcodes = self.get_opcodes(cache);
        let context_array = self.get_context_array(cache)?;
        let predicate = self.get_predicate(cache)?;
        let valid = self.get_valid_predicate(cache)?;
        if let Some(immutable) = &self.inner.immutable {
            if immutable.len() == variables.len() {
                max_context_depth = 1;
            }
        }

        let bank = match bank_type {
            BankType::SubsumptionBank => SubsumptionProgBank::default(),
        };

        let immutable_opt = self.inner.immutable;
        let syn_ctx = SynthesizerContext::from_context_array_with_data(
            context_array,
            self.classes,
            cache.clone(),
        );
        let mut synthesizer = TsSynthesizer::new(
            bank,
            syn_ctx,
            opcodes,
            predicate,
            valid,
            max_context_depth,
            iteration_workers_count,
        );

        if let Some(immutable) = &immutable_opt {
            for var in immutable {
                synthesizer.set_immutable(&str_cached!(cache; var));
            }
        }

        Ok(synthesizer)
    }

    fn get_predicate(
        &self,
        cache: &Arc<Cache>,
    ) -> Result<SynthesizerPredicate, SnythesisTaskError> {
        let variables = self.inner.variables.as_ref().unwrap();
        let examples = self.inner.examples.as_ref().unwrap();

        let mut predicate_graphs_map = GraphsMap::default();
        let predicate_gen_id = Arc::new(GraphIdGenerator::default());
        let root_name = cache.output_root_name();
        let output_array = match &self.inner.return_type {
            Some(return_type) => {
                let mut array = Vec::with_capacity(examples.len());
                for example in examples {
                    let mut output = return_type.create_value(
                        example.output.as_ref().unwrap(),
                        &self.classes,
                        predicate_gen_id.get_id_for_graph(),
                        &mut predicate_graphs_map,
                        &predicate_gen_id,
                        None,
                        cache,
                    )?;
                    if let Some(obj) = output.mut_obj() {
                        let graph = predicate_graphs_map.get_mut(&obj.graph_id).unwrap();
                        Arc::make_mut(graph).set_as_root(root_name.clone(), obj.node);
                    }
                    array.push(output);
                }
                Some(array)
            }
            None => None,
        };
        let state_array = match examples[0].state {
            Some(_) => {
                let mut array = Vec::with_capacity(examples.len());
                for example in examples {
                    let state_map = parse_json_values_map_roots(
                        example.state.as_ref().unwrap(),
                        variables,
                        &mut predicate_graphs_map,
                        &predicate_gen_id,
                        None,
                        &self.classes,
                        cache,
                    )?;
                    array.push(state_map);
                }
                Some(array)
            }
            None => None,
        };

        let predicate_js: Option<Vec<String>> =
            examples.iter().map(|e| &e.predicate_js).cloned().collect();
        let predicate_cache = cache.clone();

        let predicate = Box::new(move |p: &SubProgram, syn_ctx: &SynthesizerContext| {
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
                for (actual, expected) in p.post_ctx().iter().zip_eq(array) {
                    for (var, value) in expected.iter() {
                        let actual_value = match actual.get_var_value(var) {
                            None => return false,
                            Some(v) => v,
                        };
                        if actual_value.wrap(&actual.graphs_map)
                            != value.wrap(&predicate_graphs_map)
                        {
                            return false;
                        }
                    }
                }
            }

            if let Some(js_vec) = &predicate_js {
                let classes = syn_ctx.data.downcast_ref::<TsClasses>().unwrap();
                let mut boa_ctx = EngineContext::new_boa_ctx();
                let mut engine_ctx = EngineContext::create_engine_ctx(&mut boa_ctx, classes);

                for (ctx, js) in p.post_ctx().iter().zip_eq(js_vec) {
                    engine_ctx.reset_with_context(ctx, classes, &predicate_cache);
                    let mut arg_names = Vec::with_capacity(ctx.variable_count());
                    let mut js_values = Vec::with_capacity(ctx.variable_count());
                    for (var, value) in ctx.variables() {
                        arg_names.push(var.as_str());
                        js_values.push(value_to_js_value(classes, value, &mut engine_ctx));
                    }
                    let code = format!(
                        "function func({}) {{return {}}}\nfunc",
                        arg_names.join(", "),
                        js
                    );
                    let js_func = engine_ctx
                        .eval(boa_engine::Source::from_bytes(&code))
                        .unwrap();
                    let func = js_func.as_callable().unwrap();
                    match func.call(&boa_engine::JsValue::null(), &js_values, &mut engine_ctx) {
                        Ok(val) => {
                            if let Some(b) = val.as_boolean() {
                                return b;
                            }
                            return false;
                        }
                        Err(_) => return false,
                    };
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
        Ok(Box::new(move |_p, _syn_ctx| true))
    }

    fn get_opcodes(&self, cache: &Cache) -> OpcodesList {
        let var_names: Vec<Arc<String>> = self
            .inner
            .variables
            .as_ref()
            .unwrap()
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
            Self::get_composite_opcodes(&self.classes, &self.class_names, true, &cache);

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
        add_seq: bool,
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

        add_set_opcodes(
            &mut composite_opcodes,
            &[ValueType::Number, ValueType::String],
            cache,
        );
        if add_seq {
            let mut value_types = vec![ValueType::Number, ValueType::String, ValueType::Null];
            value_types.extend(class_names.iter().map(|x| ValueType::Object(x.clone())));
            value_types.push(ValueType::array_value_type(&ValueType::Number, cache));
            value_types.push(ValueType::array_value_type(&ValueType::String, cache));
            value_types.push(ValueType::set_value_type(&ValueType::Number, cache));
            value_types.push(ValueType::set_value_type(&ValueType::String, cache));
            add_seq_opcodes(&mut composite_opcodes, 2, &value_types);
        }

        composite_opcodes.extend(Self::get_classes_opcodes(classes, class_names));

        composite_opcodes
    }

    pub fn get_classes_opcodes(classes: &TsClasses, class_names: &Vec<ObjectType>) -> OpcodesList {
        let mut composite_opcodes = OpcodesList::new();

        for class_name in class_names {
            let class = classes.get_class(class_name).unwrap();
            composite_opcodes.extend_from_slice(&class.member_opcodes);
            composite_opcodes.extend_from_slice(&class.method_opcodes);
        }

        composite_opcodes
    }

    fn get_context_array(&self, cache: &Arc<Cache>) -> Result<ContextArray, SnythesisTaskError> {
        let variables = self.inner.variables.as_ref().unwrap();
        let examples = self.inner.examples.as_ref().unwrap();

        let mut values = Vec::with_capacity(examples.len());
        for example in examples {
            values.push(example.create_context(variables, &self.classes, cache)?);
        }

        Ok(values.into())
    }

    pub fn from_json_file(
        path: &Path,
        cache: &Arc<Cache>,
    ) -> Result<SnythesisTask, SnythesisTaskError> {
        let reader = std::fs::File::open(path).map_err(|e| SnythesisTaskError::IO(e))?;
        let mut inner: SnythesisTaskInner = match serde_json::from_reader(reader) {
            Ok(val) => val,
            Err(e) => {
                return Err(parse_err!("json", e));
            }
        };
        inner.verify()?;

        let variables = inner.variables.as_ref().unwrap();

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

        let mut builder = TsClassesBuilder::new();
        let mut class_names = vec![];
        if let Some(classes_code) = &inner.classes {
            for code in classes_code {
                match builder.add_class(code, cache) {
                    Ok(class_name) => class_names.push(class_name),
                    Err(e) => {
                        return Err(parse_err!(code, e));
                    }
                }
            }
        }

        if let Some(ts_files) = &inner.ts_files {
            for ts_file in ts_files {
                let full_path = match ts_file.is_relative() {
                    true => path.parent().unwrap().join(ts_file),
                    false => ts_file.clone(),
                };
                match builder.add_ts_file(&full_path, cache) {
                    Ok(names) => class_names.extend(names),
                    Err(e) => {
                        return Err(parse_err!(String::from(full_path.to_string_lossy()), e));
                    }
                };
            }
        }

        let classes = builder.finalize(cache);

        for (var, var_type) in variables {
            if let TaskType::Object(obj_type) = var_type {
                if classes.get_class(&str_cached!(cache; obj_type)).is_none() {
                    return Err(verify_err!(format!(
                        "Variable {} has an unknown object type {}",
                        var, obj_type
                    )));
                }
            }
        }

        for example in inner.examples.as_mut().unwrap() {
            if let Some(return_type) = &inner.return_type {
                return_type.load_value(dir.as_path(), example.output.as_mut().unwrap())?;
            }
            for (var, var_type) in variables {
                var_type.load_value(dir.as_path(), example.input.get_mut(var).unwrap())?;
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
