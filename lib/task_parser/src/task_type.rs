use std::{
    collections::HashMap,
    fmt, fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use ruse_object_graph::{
    class_name, field_name, fields, str_cached, value::Value, vbool, vnull, vnum, vobj, vstr,
    GraphIndex, GraphsMap, ObjectType, PrimitiveValue, ValueType,
};
use ruse_synthesizer::context::GraphIdGenerator;
use ruse_ts_interpreter::{
    dom::{self, DomLoader},
    engine_context::EngineContext,
    ts_classes::TsClasses,
    ts_user_class::TsUserClass,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::json;

use crate::{
    error::SnythesisTaskError,
    parse_err,
    task::parse_json_values_array,
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
    Double,
    Bool,
    String,
    Array(Box<TaskType>),
    Set(Box<TaskType>),
    Map(Box<TaskType>, Box<TaskType>),
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
    ) -> Result<Value, SnythesisTaskError> {
        if let Some(expr) = Self::get_expr_value(value) {
            self.parse_expr_value(expr, classes, graphs_map, refs_graph_id, id_gen)
        } else {
            let out = self.create_regular_value(
                value,
                graph_id,
                classes,
                graphs_map,
                id_gen,
                refs_graph_id,
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
    ) -> Result<Value, SnythesisTaskError> {
        match self {
            TaskType::Int => match value.as_i64() {
                Some(num) => Ok(vnum!(ruse_object_graph::Number::from(num))),
                None => Err(parse_err!(value, "Value is not a int")),
            },
            TaskType::Double => match value.as_number() {
                Some(num) => Ok(vnum!(ruse_object_graph::Number::from(
                    num.as_f64().unwrap()
                ))),
                None => Err(parse_err!(value, "Value is not a double")),
            },
            TaskType::Bool => match value.as_bool() {
                Some(b) => Ok(vbool!(b)),
                None => Err(parse_err!(value, "Value is not a boolean")),
            },
            TaskType::String => match value.as_str() {
                Some(s) => Ok(vstr!(Self::strip_string(s)?)),
                None => Err(parse_err!(value, "Value is not a string")),
            },
            TaskType::Array(inner_type) => {
                if inner_type.is_var_ref() || inner_type.is_dom() {
                    return Err(parse_err!(
                        value,
                        format!("Array of {} is not supported", inner_type)
                    ));
                }
                let elements = match value.as_array() {
                    Some(value_array) => {
                        let result: Result<Vec<_>, _> = value_array
                            .iter()
                            .map(|inner_value| {
                                inner_type.create_regular_value(
                                    inner_value,
                                    graph_id,
                                    classes,
                                    graphs_map,
                                    id_gen,
                                    refs_graph_id,
                                )
                            })
                            .collect();

                        result?
                    }
                    None => return Err(parse_err!(value, "Value is not an array")),
                };
                graphs_map.ensure_graph(graph_id);
                let node = graphs_map.add_array_object(
                    graph_id,
                    id_gen.get_id_for_node(),
                    &inner_type.value_type(),
                    elements,
                );

                Ok(vobj!(
                    ObjectType::array_obj_type(&inner_type.value_type()),
                    graph_id,
                    node
                ))
            }
            TaskType::Set(inner_type) => {
                if inner_type.is_var_ref() || inner_type.is_dom() || inner_type.is_object() {
                    return Err(parse_err!(
                        value,
                        format!("Set of {} is not supported", inner_type)
                    ));
                }
                let elements = match value.as_array() {
                    Some(value_array) => {
                        let result: Result<Vec<_>, _> = value_array
                            .iter()
                            .map(|inner_value| {
                                let elem = inner_type.create_regular_value(
                                    inner_value,
                                    graph_id,
                                    classes,
                                    graphs_map,
                                    id_gen,
                                    refs_graph_id,
                                )?;

                                Ok(elem.into_primitive().unwrap())
                            })
                            .collect();

                        result?
                    }
                    None => return Err(parse_err!(value, "Value is not a set")),
                };
                graphs_map.ensure_graph(graph_id);
                let node = graphs_map.add_primitive_set_object(
                    graph_id,
                    id_gen.get_id_for_node(),
                    &inner_type.value_type(),
                    elements,
                );

                Ok(vobj!(
                    ObjectType::set_obj_type(&inner_type.value_type()),
                    graph_id,
                    node
                ))
            }
            TaskType::Map(key_type, value_type) => {
                if !key_type.is_primitive() {
                    return Err(parse_err!(
                        value,
                        format!("Only map with primitive keys is supported")
                    ));
                }
                let values = value
                    .as_object()
                    .ok_or(parse_err!(value, "Value is not a map value"))?;
                let mut parsed_values = HashMap::new();
                for (k, v) in values {
                    key_type.json_value_from_string(k)?;

                    let key = field_name!(k.as_str());
                    let value = value_type.create_value(
                        v,
                        classes,
                        graph_id,
                        graphs_map,
                        id_gen,
                        refs_graph_id,
                    )?;
                    parsed_values.insert(key, value);
                }

                graphs_map.ensure_graph(graph_id);
                let node = graphs_map.add_object_from_map(
                    graph_id,
                    id_gen.get_id_for_node(),
                    ObjectType::map_obj_type(&key_type.value_type(), &value_type.value_type()),
                    parsed_values,
                );

                Ok(vobj!(
                    ObjectType::map_obj_type(&key_type.value_type(), &value_type.value_type(),),
                    graph_id,
                    node
                ))
            }
            TaskType::Object(s) => {
                let class_name = class_name!(s.as_str());
                let class = classes
                    .get_user_class(&class_name)
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
                    )
                }
            }
            TaskType::Dom | TaskType::DOMElement => {
                let html = match value.as_str() {
                    Some(str) => Self::strip_string(str),
                    None => return Err(parse_err!(value, "Value for html is not an string value")),
                }?;
                graphs_map.ensure_graph(graph_id);
                self.parse_dom(html, graph_id, graphs_map, id_gen)
            }
            TaskType::VarRef(_) => Err(parse_err!(value, "Var ref is a delayed type")),
        }
    }

    fn create_object_from_fields(
        &self,
        fields: &JsonValuesMap,
        class: &TsUserClass,
        graph_id: GraphIndex,
        classes: &TsClasses,
        graphs_map: &mut GraphsMap,
        id_gen: &Arc<GraphIdGenerator>,
        refs_graph_id: Option<GraphIndex>,
    ) -> Result<Value, SnythesisTaskError> {
        let fields_types: HashMap<&str, TaskType> = HashMap::from_iter(
            class
                .description
                .fields
                .iter()
                .map(|(k, v)| (k.as_str(), TaskType::from(&v.value_type))),
        );

        let mut values = HashMap::new();
        for (k, v) in fields {
            let key = field_name!(k.as_str());
            let value_type = match fields_types.get(&k.as_str()) {
                Some(value_type) => value_type,
                None => return Err(verify_err!("{} type is unknown", k)),
            };
            let value =
                value_type.create_value(v, classes, graph_id, graphs_map, id_gen, refs_graph_id)?;
            values.insert(key, value);
        }

        graphs_map.ensure_graph(graph_id);
        let obj = class.generate_object(values, graphs_map, graph_id, id_gen);

        Ok(Value::Object(obj))
    }

    fn create_object_from_method(
        &self,
        class: &TsUserClass,
        method_name: &str,
        json_args: &Vec<serde_json::Value>,
        graph_id: GraphIndex,
        classes: &TsClasses,
        graphs_map: &mut GraphsMap,
        id_gen: &Arc<GraphIdGenerator>,
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
            json_args, arg_types, graph_id, graphs_map, id_gen, classes, None,
        )?;

        let mut boa_ctx = EngineContext::new_boa_ctx();
        let mut engine_ctx = EngineContext::create_engine_ctx(&mut boa_ctx, classes);
        graphs_map.ensure_graph(graph_id); // Make sure graph exists
        engine_ctx.reset_with_graph(graph_id, graphs_map, classes, id_gen);

        if method_name == "constructor" {
            let new_obj = class
                .call_constructor(&args, &mut engine_ctx)
                .map_err(|x| SnythesisTaskError::Eval(x))?;
            Ok(Value::Object(new_obj))
        } else {
            class
                .call_static_method(method_name, &args, &mut engine_ctx)
                .map_err(|x| SnythesisTaskError::Eval(x))
        }
    }

    pub fn parse_dom(
        &self,
        html: &str,
        graph_id: GraphIndex,
        graphs_map: &mut GraphsMap,
        id_gen: &GraphIdGenerator,
    ) -> Result<Value, SnythesisTaskError> {
        match self {
            TaskType::Dom => {
                let dom_value = match dom::DomLoader::load_dom(id_gen, graph_id, graphs_map, html) {
                    Ok(v) => v,
                    Err(e) => return Err(parse_err!(html, e)),
                };

                Ok(vobj!(
                    dom::DomLoader::document_obj_type(),
                    graph_id,
                    dom_value
                ))
            }
            TaskType::DOMElement => {
                let dom_value =
                    match dom::DomLoader::load_element(id_gen, graph_id, graphs_map, html) {
                        Ok(v) => v,
                        Err(e) => return Err(parse_err!(html, e)),
                    };

                Ok(vobj!(
                    dom::DomLoader::element_obj_type(),
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
    ) -> Result<Value, SnythesisTaskError> {
        if let Some(static_ref) = ExprPrefix::StaticRef.get(value_string) {
            self.parse_static_ref_expr_value(static_ref, value_string, classes, graphs_map)
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
    ) -> Result<Value, SnythesisTaskError> {
        let (class_name, field_name) = static_ref.split_once('.').ok_or(parse_err!(
            value_string,
            format!("The static ref expr contains no '.'")
        ))?;
        let class = classes
            .get_user_class(&class_name!(class_name))
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
            .get(&field_name!(field_name))
            .ok_or(parse_err!(
                value_string,
                format!(
                    "The field {} is not defined for class {}, can't parse static field ref",
                    field_name, class_name
                )
            ))?;
        if &field_desc.value_type != &self.value_type() {
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
    ) -> Result<Value, SnythesisTaskError> {
        if !self.is_object() {
            return Err(parse_err!(
                value_string,
                format!("The field ref expression is only valid for object types")
            ));
        }

        graphs_map.ensure_graph(refs_graph_id);
        let obj_type = ObjectType::class_obj_type(&REF_GRAPH_OBJ_TYPE);
        let ref_id = graphs_map.add_simple_object_from_fields_map(
            refs_graph_id,
            id_gen.get_id_for_node(),
            obj_type.clone(),
            fields!((
                field_name!(REF_GRAPH_FIELD_NAME),
                PrimitiveValue::String(str_cached!(field_ref))
            )),
        );

        Ok(vobj!(obj_type, refs_graph_id, ref_id))
    }

    pub fn value_type(&self) -> ValueType {
        match self {
            TaskType::Int => ValueType::Number,
            TaskType::Double => ValueType::Number,
            TaskType::Bool => ValueType::Bool,
            TaskType::String => ValueType::String,
            TaskType::Array(inner_type) => ValueType::array_value_type(&inner_type.value_type()),
            TaskType::Set(inner_type) => ValueType::set_value_type(&inner_type.value_type()),
            TaskType::Map(key_type, val_type) => {
                ValueType::map_value_type(&key_type.value_type(), &val_type.value_type())
            }
            TaskType::Dom => ValueType::Object(DomLoader::document_obj_type()),
            TaskType::DOMElement => ValueType::Object(DomLoader::element_obj_type()),
            TaskType::VarRef(_var_ref) => todo!(),
            TaskType::Object(o) => ValueType::Object(ObjectType::class_obj_type(&o)),
        }
    }

    pub(crate) fn is_var_ref(&self) -> bool {
        match self {
            TaskType::VarRef(_) => true,
            _ => false,
        }
    }

    pub(crate) fn is_dom(&self) -> bool {
        match self {
            TaskType::DOMElement => true,
            TaskType::Dom => true,
            _ => false,
        }
    }

    pub(crate) fn is_object(&self) -> bool {
        match self {
            TaskType::Object(_) => true,
            _ => false,
        }
    }

    pub(crate) fn is_primitive(&self) -> bool {
        match self {
            TaskType::Bool => true,
            TaskType::Double => true,
            TaskType::Int => true,
            TaskType::String => true,
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

    fn parse_double_set(value_string: &str) -> Result<serde_json::Value, SnythesisTaskError> {
        if !value_string.starts_with('{') {
            return Err(parse_err!(value_string, "Missing opening bracket"));
        }
        if !value_string.ends_with('}') {
            return Err(parse_err!(value_string, "Missing closing bracket"));
        }
        let array_string = value_string.replace('{', "[").replace('}', "]");
        match serde_json::from_str::<Vec<f64>>(array_string.as_str()) {
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
            TaskType::Double => match value_string.parse::<f64>() {
                Ok(num) => Ok(json!(num)),
                Err(e) => Err(parse_err!(value_string, e)),
            },
            TaskType::Bool => match value_string.parse::<bool>() {
                Ok(b) => Ok(json!(b)),
                Err(e) => Err(parse_err!(value_string, e)),
            },
            TaskType::String => Ok(json!(value_string)),
            TaskType::Array(inner_element) => match inner_element.as_ref() {
                TaskType::Int => match serde_json::from_str::<Vec<i64>>(value_string) {
                    Ok(numbers) => Ok(json!(numbers)),
                    Err(e) => Err(parse_err!(value_string, e)),
                },
                TaskType::Double => match serde_json::from_str::<Vec<f64>>(value_string) {
                    Ok(numbers) => Ok(json!(numbers)),
                    Err(e) => Err(parse_err!(value_string, e)),
                },
                TaskType::String => Self::parse_string_collection(value_string, false),
                _ => {
                    return Err(parse_err!(
                        value_string,
                        format!("Doesn't support converting from string to {}", self)
                    ))
                }
            },
            TaskType::Set(inner_element) => match inner_element.as_ref() {
                TaskType::Int => Self::parse_int_set(value_string),
                TaskType::Double => Self::parse_double_set(value_string),
                TaskType::String => Self::parse_string_collection(value_string, true),
                _ => {
                    return Err(parse_err!(
                        value_string,
                        format!("Doesn't support converting from string to {}", self)
                    ))
                }
            },
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
            TaskType::Map(_, _) => Err(parse_err!(
                value_string,
                "Doesn't support converting from string to map value"
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

impl fmt::Display for TaskType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskType::Int => write!(f, "int"),
            TaskType::Double => write!(f, "double"),
            TaskType::Bool => write!(f, "bool"),
            TaskType::String => write!(f, "string"),
            TaskType::Array(inner_type) => write!(f, "Array<{}>", inner_type),
            TaskType::Set(inner_type) => write!(f, "Set<{}>", inner_type),
            TaskType::Map(key, val) => write!(f, "Map<{},{}>", key, val),
            TaskType::Dom => write!(f, "dom"),
            TaskType::DOMElement => write!(f, "dom_element"),
            TaskType::VarRef(var_ref) => write!(f, "{}", var_ref),
            TaskType::Object(o) => write!(f, "{}", o),
        }
    }
}

impl From<&ValueType> for TaskType {
    fn from(value_type: &ValueType) -> Self {
        match value_type {
            ValueType::Number => TaskType::Double,
            ValueType::Bool => TaskType::Bool,
            ValueType::String => TaskType::String,
            ValueType::Object(ObjectType::Class(class_name)) => {
                TaskType::Object(class_name.to_string())
            }
            ValueType::Object(ObjectType::Array(inner_type)) => {
                TaskType::Array(TaskType::from(inner_type.as_ref()).into())
            }
            ValueType::Object(ObjectType::Set(inner_type)) => {
                TaskType::Set(TaskType::from(inner_type.as_ref()).into())
            }
            ValueType::Object(ObjectType::Map(key_type, value_type)) => TaskType::Map(
                TaskType::from(key_type.as_ref()).into(),
                TaskType::from(value_type.as_ref()).into(),
            ),
            _ => unreachable!("Unsupported type"),
        }
    }
}

impl Into<ValueType> for TaskType {
    fn into(self) -> ValueType {
        self.value_type()
    }
}

fn strip_string_wrap(value_str: &str, prefix: char, suffix: char) -> Option<&str> {
    value_str.strip_prefix(prefix)?.strip_suffix(suffix)
}

fn strip_template_class<'a>(value_str: &'a str, class: &str) -> Option<&'a str> {
    let prefix = format!("{}<", class);
    value_str.strip_prefix(&prefix)?.strip_suffix('>')
}

fn array_elem_type<'a>(value_str: &'a str) -> Option<TaskType> {
    let elem_type =
        strip_string_wrap(value_str, '[', ']').or(strip_template_class(value_str, "Array"))?;
    Some(TaskType::from(elem_type))
}

fn set_elem_type<'a>(value_str: &'a str) -> Option<TaskType> {
    let elem_type =
        strip_string_wrap(value_str, '{', '}').or(strip_template_class(value_str, "Set"))?;
    Some(TaskType::from(elem_type))
}

fn map_types<'a>(value_str: &'a str) -> Option<(TaskType, TaskType)> {
    let types_str = strip_template_class(value_str, "Map")?;
    let elements: Vec<&str> = types_str.split(',').collect();
    assert!(elements.len() == 2, "Map type should have two types");
    let key_type = TaskType::from(elements[0].trim());
    let val_type = TaskType::from(elements[1].trim());
    Some((key_type, val_type))
}

impl From<&str> for TaskType {
    fn from(val: &str) -> Self {
        match val {
            "Int" => TaskType::Int,
            "Double" => TaskType::Double,
            "Bool" => TaskType::Bool,
            "String" => TaskType::String,
            "DOM" => TaskType::Dom,
            "DOMElement" => TaskType::DOMElement,
            _ => {
                if let Some(elem_type) = array_elem_type(val) {
                    TaskType::Array(elem_type.into())
                } else if let Some(elem_type) = set_elem_type(val) {
                    TaskType::Set(elem_type.into())
                } else if let Some((key_type, val_type)) = map_types(val) {
                    TaskType::Map(key_type.into(), val_type.into())
                } else if let Some(var_ref) = ExprPrefix::FieldRef.get(&val) {
                    TaskType::VarRef(VarRef::from(var_ref))
                } else {
                    TaskType::Object(val.to_string())
                }
            }
        }
    }
}

impl<'de> Deserialize<'de> for TaskType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val = String::deserialize(deserializer)?;
        Ok(TaskType::from(val.as_str()))
    }
}

impl Serialize for TaskType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(format!("{}", self).as_str())
    }
}
