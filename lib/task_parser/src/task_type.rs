use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use itertools::Itertools;
use ruse_object_graph::{
    fields, str_cached,
    value::{Value, ValueType},
    vbool, vnull, vnum, vobj, vstr, Cache, GraphIndex, GraphsMap, Number, PrimitiveValue,
};
use ruse_synthesizer::context::GraphIdGenerator;
use ruse_ts_interpreter::{
    dom::{self, DomLoader},
    js_object_wrapper::EngineContext,
    ts_class::{TsClass, TsClasses},
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::json;

use crate::{
    error::SnythesisTaskError,
    parse_err,
    task::{parse_json_values_array, parse_json_values_map},
    var_ref::{VarRef, REF_GRAPH_FIELD_NAME, REF_GRAPH_OBJ_TYPE},
    verify_err,
};

pub(crate) type JsonValuesMap = serde_json::Map<String, serde_json::Value>;

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

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum TaskType {
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
                graphs_map.ensure_graph(graph_id);
                let node = graphs_map.add_primitive_array_object(
                    graph_id,
                    id_gen.get_id_for_node(),
                    &ValueType::Number,
                    numbers,
                    cache,
                );

                Ok(vobj!(
                    ValueType::array_obj_cached_string(&ValueType::Number, cache),
                    graph_id,
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
                graphs_map.ensure_graph(graph_id);
                let node = graphs_map.add_primitive_array_object(
                    graph_id,
                    id_gen.get_id_for_node(),
                    &ValueType::Number,
                    numbers,
                    cache,
                );

                Ok(vobj!(
                    ValueType::array_obj_cached_string(&ValueType::Number, cache),
                    graph_id,
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
                graphs_map.ensure_graph(graph_id);
                let node = graphs_map.add_primitive_array_object(
                    graph_id,
                    id_gen.get_id_for_node(),
                    &ValueType::String,
                    strings?,
                    cache,
                );

                Ok(vobj!(
                    ValueType::array_obj_cached_string(&ValueType::String, cache),
                    graph_id,
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
                graphs_map.ensure_graph(graph_id);
                let node = graphs_map.add_primitive_set_object(
                    graph_id,
                    id_gen.get_id_for_node(),
                    &ValueType::Number,
                    numbers.unique(),
                    cache,
                );

                Ok(vobj!(
                    ValueType::set_obj_cached_string(&ValueType::Number, cache),
                    graph_id,
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
                graphs_map.ensure_graph(graph_id);
                let node = graphs_map.add_primitive_set_object(
                    graph_id,
                    id_gen.get_id_for_node(),
                    &ValueType::String,
                    strings?.into_iter().unique(),
                    cache,
                );

                Ok(vobj!(
                    ValueType::set_obj_cached_string(&ValueType::String, cache),
                    graph_id,
                    node
                ))
            }
            TaskType::Object(s) => {
                let class_name = str_cached!(cache; s);
                let class = classes
                    .get_class(&class_name)
                    .ok_or(verify_err!("object type {} is not defined", s))?;
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
                graphs_map.ensure_graph(graph_id);
                self.parse_dom(html, graph_id, graphs_map, id_gen, cache)
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

        graphs_map.ensure_graph(graph_id);
        let obj = class.generate_object(values, graphs_map, graph_id, id_gen);

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
                .ok_or(verify_err!(
                    "object {} has no method {}",
                    &class.description.class_name,
                    method_name
                ))?;
            if !desc.is_static {
                return Err(verify_err!(
                    "{}.{} is not static",
                    &class.description.class_name,
                    method_name
                ));
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
        graphs_map.ensure_graph(graph_id); // Make sure graph exists
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
        graph_id: GraphIndex,
        graphs_map: &mut GraphsMap,
        id_gen: &GraphIdGenerator,
        cache: &Cache,
    ) -> Result<Value, SnythesisTaskError> {
        match self {
            TaskType::Dom => {
                let dom_value =
                    match dom::DomLoader::load_dom(id_gen, graph_id, graphs_map, html, cache) {
                        Ok(v) => v,
                        Err(e) => return Err(parse_err!(html, e)),
                    };

                Ok(vobj!(
                    dom::DomLoader::document_obj_type(cache),
                    graph_id,
                    dom_value
                ))
            }
            TaskType::DOMElement => {
                let dom_value =
                    match dom::DomLoader::load_element(id_gen, graph_id, graphs_map, html, cache) {
                        Ok(v) => v,
                        Err(e) => return Err(parse_err!(html, e)),
                    };

                Ok(vobj!(
                    dom::DomLoader::element_obj_type(cache),
                    graph_id,
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

        graphs_map.ensure_graph(refs_graph_id);
        let obj_type = str_cached!(cache; REF_GRAPH_OBJ_TYPE);
        let ref_id = graphs_map.add_simple_object_from_fields_map(
            refs_graph_id,
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

    pub(crate) fn is_var_ref(&self) -> bool {
        match self {
            TaskType::VarRef(_) => true,
            _ => false,
        }
    }

    pub(crate) fn is_object(&self) -> bool {
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

    pub fn json_value_from_string(
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

    pub(crate) fn load_value(
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
