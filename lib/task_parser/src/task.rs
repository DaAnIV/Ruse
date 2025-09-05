use std::{
    collections::{HashMap, HashSet},
    fmt::{self, Debug, Display},
    hash::{BuildHasherDefault, DefaultHasher},
    path::{Path, PathBuf},
    sync::Arc,
};

use itertools::Itertools;
use ruse_object_graph::{
    *,
    {value::Value, ValueType},
};
use ruse_synthesizer::{
    bank::ProgBank,
    context::{
        Context, ContextArray, GraphIdGenerator, SynthesizerContext, ValuesMap, VariableName,
    },
    opcode::{ExprOpcode, OpcodesList},
    synthesizer::SynthesizerPredicate,
};
use ruse_ts_interpreter::{
    engine_context::EngineContext,
    ts_classes::{TsClasses, TsClassesBuilder},
};
use ruse_ts_synthesizer::*;

use serde::{Deserialize, Serialize};
use tracing::debug;
use wildmatch::WildMatch;

use crate::{
    error::SnythesisTaskError,
    io_err, parse_err,
    predicate_builder::{PredicateBuilder, ValidPredicateBuilder},
    skip_err,
    task_type::{JsonValuesMap, TaskType},
    var_ref::{set_var_refs, verify_no_var_ref_circle, REF_GRAPH_ID},
    verify_err,
};

fn upgrade_values_map(
    map: &mut JsonValuesMap,
    types: &HashMap<String, TaskType>,
) -> Result<(), SnythesisTaskError> {
    for (k, v) in map.iter_mut() {
        let value_type = &match types.get(k) {
            Some(value_type) => value_type,
            None => return Err(verify_err!("{} type is unknown", k)),
        };

        let value_str = v.as_str().ok_or(verify_err!(
            "All values must be given as string in version 1"
        ))?;
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
    engine_ctx: &mut EngineContext,
) -> Result<ValuesMap, SnythesisTaskError>
where
    M: IntoIterator<Item = (&'a String, &'a serde_json::Value)>,
{
    let mut values = ValuesMap::default();
    for (k, v) in map {
        let key = root_name!(k.as_str());
        let value_type = &match types.get(k) {
            Some(value_type) => value_type,
            None => return Err(verify_err!("{} type is unknown", k)),
        };
        let mut value = value_type.create_value(
            v,
            classes,
            id_gen.get_id_for_graph(),
            graphs_map,
            id_gen,
            refs_graph_id,
            engine_ctx,
        )?;

        if let Some(obj) = value.mut_obj() {
            graphs_map.set_as_root(key.clone(), obj.graph_id, obj.node);
        }
        values.insert(key, value);
    }

    Ok(values)
}

pub(crate) fn parse_json_values_array<'a, V, T>(
    arr: V,
    types: T,
    graph_id: GraphIndex,
    graphs_map: &mut GraphsMap,
    id_gen: &Arc<GraphIdGenerator>,
    classes: &TsClasses,
    refs_graph_id: Option<GraphIndex>,
    engine_ctx: &mut EngineContext,
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
            engine_ctx,
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
    predicate_js: Option<serde_json::Value>,
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
        engine_ctx: &mut EngineContext,
    ) -> Result<Box<Context>, SnythesisTaskError> {
        let id_gen = Arc::new(GraphIdGenerator::with_initial_values(
            classes.static_classes_gen_id.max_node_id(),
            classes.static_classes_gen_id.max_graph_id(),
        ));
        let mut graphs_map = GraphsMap::default();
        graphs_map.ensure_graph(REF_GRAPH_ID);

        let mut values = parse_json_values_map_roots(
            &self.input,
            variables,
            &mut graphs_map,
            &id_gen,
            Some(REF_GRAPH_ID),
            classes,
            engine_ctx,
        )?;
        set_var_refs(variables, &mut values, &mut graphs_map)?;
        graphs_map.remove(REF_GRAPH_ID);
        Ok(Context::with_values(values, graphs_map.into(), id_gen))
    }

    fn extend(&mut self, other: &Self) {
        if other.output.is_some() {
            debug_assert!(self.output.is_none());
            self.output = other.output.clone()
        }
        if other.state.is_some() {
            debug_assert!(self.state.is_none());
            self.state = other.state.clone()
        }
        if other.predicate_js.is_some() {
            debug_assert!(self.predicate_js.is_none());
            self.predicate_js = other.predicate_js.clone()
        }
        for (name, value) in &other.input {
            debug_assert!(!self.input.contains_key(name));
            self.input.insert(name.clone(), value.clone());
        }
    }
}

fn default_strings() -> bool {
    true
}

fn default_pure() -> bool {
    false
}

fn default_max_seq_size() -> Option<usize> {
    Some(2)
}

fn default_version() -> u32 {
    1
}

#[derive(Deserialize, Serialize, Debug)]
struct SnythesisTaskOptions {
    #[serde(default = "default_strings")]
    strings: bool,
    #[serde(default = "default_pure")]
    pure: bool,
    #[serde(default = "default_max_seq_size")]
    max_seq_size: Option<usize>,
}

impl Default for SnythesisTaskOptions {
    fn default() -> Self {
        Self {
            strings: default_strings(),
            pure: default_pure(),
            max_seq_size: default_max_seq_size(),
        }
    }
}

#[derive(Debug, Copy, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SynthesisOOPCategory {
    Primitive,
    PrimitiveObjects,
    FullOOP,
}

impl Display for SynthesisOOPCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SynthesisOOPCategory::Primitive => write!(f, "Primitive"),
            SynthesisOOPCategory::PrimitiveObjects => write!(f, "Primitive Objects"),
            SynthesisOOPCategory::FullOOP => write!(f, "Full OOP"),
        }
    }
}

#[derive(Debug, Copy, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SnythesisTaskSideEffects {
    Yes,
    No,
    Maybe,
}

impl Display for SnythesisTaskSideEffects {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SnythesisTaskSideEffects::Yes => write!(f, "With Side Effects"),
            SnythesisTaskSideEffects::No => write!(f, "Without Side Effects"),
            SnythesisTaskSideEffects::Maybe => write!(f, "Possibly With Side Effects"),
        }
    }
}

#[derive(Debug, Copy, Clone, Deserialize, Serialize)]
pub struct SnythesisTaskCategory {
    pub oop_category: SynthesisOOPCategory,
    pub side_effects: SnythesisTaskSideEffects,
}

impl Display for SnythesisTaskCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.oop_category, self.side_effects)
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct SnythesisTaskSolution {
    has_side_effects: SnythesisTaskSideEffects,
    expected: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct SnythesisTaskInner {
    #[serde(default = "default_version")]
    version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    #[serde(default)]
    options: SnythesisTaskOptions,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_string_size: Option<usize>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    immutable: Option<HashSet<String>>,
    examples: Vec<SnythesisTaskExamples>,
    solution: SnythesisTaskSolution,
    #[serde(skip_serializing_if = "Option::is_none")]
    opcodes: Option<HashSet<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    common: Option<SnythesisTaskExamples>,
}

impl SnythesisTaskInner {
    fn extend_examples(&mut self) {
        if let Some(common) = &self.common {
            for example in &mut self.examples {
                example.extend(common);
            }
        }
    }

    fn verify(&self) -> Result<(), SnythesisTaskError> {
        let variables = &self.variables;
        let examples = &self.examples;

        let has_common_output = self.common.as_ref().is_some_and(|x| x.output.is_some());
        let has_common_state = self.common.as_ref().is_some_and(|x| x.state.is_some());
        let has_common_js = self
            .common
            .as_ref()
            .is_some_and(|x| x.predicate_js.is_some());
        let has_common_predicate = has_common_output | has_common_state | has_common_js;

        if let Some(immutable_set) = &self.immutable {
            for imm in immutable_set {
                if !variables.contains_key(imm) {
                    return Err(verify_err!(
                        "Immutable contains a key {} which is not a variable",
                        imm
                    ));
                }
            }
        }

        if examples.is_empty() {
            return Err(verify_err!("No examples were given"));
        }

        if examples.iter().any(|x| x.input.is_empty()) {
            return Err(verify_err!("All examples must define at least one input"));
        }

        if has_common_output && examples.iter().any(|x| x.output.is_some()) {
            return Err(verify_err!(
                "Can either have common output or output per example, not both"
            ));
        }

        if has_common_state && examples.iter().any(|x| x.state.is_some()) {
            return Err(verify_err!(
                "Can either have common state predicate or state predicate per example, not both"
            ));
        }

        if has_common_js && examples.iter().any(|x| x.predicate_js.is_some()) {
            return Err(verify_err!(
                "Can either have common js predicate or js predicate per example, not both"
            ));
        }

        if (has_common_output || examples.iter().any(|x| x.output.is_some()))
            && self.return_type.is_none()
        {
            return Err(verify_err!(
                "Can't give example outputs without a return type"
            ));
        }

        if !has_common_predicate
            && examples
                .iter()
                .any(|x| x.output.is_none() && x.state.is_none() && x.predicate_js.is_none())
        {
            return Err(verify_err!(
                "There must be at least one form of predicate for success (output, state, predicate_js), either common or per example"
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
                verify_no_var_ref_circle(var, variables)?;
            }
        }

        for (var_name, var_type) in variables {
            let var_is_common = self
                .common
                .as_ref()
                .is_some_and(|x| x.input.contains_key(var_name));

            if var_type.is_var_ref() {
                if var_is_common || examples.iter().any(|x| x.input.contains_key(var_name)) {
                    return Err(verify_err!(
                        "Var-ref variable {} value should not be defined",
                        var_name
                    ));
                }
            } else {
                if !var_is_common && examples.iter().any(|x| !x.input.contains_key(var_name)) {
                    return Err(verify_err!(
                        "Variable {} value is not defined everywhere",
                        var_name
                    ));
                }
            }
        }

        Ok(())
    }

    fn upgrade_from_version_1(&mut self) -> Result<(), SnythesisTaskError> {
        for example in &mut self.examples {
            example.upgrade_from_version_1(&self.variables, &self.return_type)?
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct SnythesisTask {
    pub path: PathBuf,
    pub name: String,
    inner: SnythesisTaskInner,
    classes: Box<TsClasses>,
    pub string_literals: HashSet<String, BuildHasherDefault<DefaultHasher>>,
    pub num_literals: HashSet<i64, BuildHasherDefault<DefaultHasher>>,
    oop_category: SynthesisOOPCategory,
}

impl SnythesisTask {
    const DEFAULT_STRING_LITERALS: [&str; 2] = ["", " "];
    const DEFAULT_NUM_LITERALS: [i64; 2] = [0, 1];

    pub fn get_synthesizer<P: ProgBank + 'static>(
        self,
        mut max_context_depth: usize,
        max_seq_size: usize,
        iteration_workers_count: usize,
        bank: P,
    ) -> Result<TsSynthesizer<P>, SnythesisTaskError> {
        let variables = &self.inner.variables;

        let mut engine_ctx = EngineContext::create_engine_ctx(&self.classes);

        let opcodes = self.get_opcodes(max_seq_size);
        let context_array = self.get_context_array(&mut engine_ctx)?;
        let predicate = self.get_predicate(&mut engine_ctx)?;
        let valid = self.get_valid_predicate()?;

        if tracing::enabled!(tracing::Level::DEBUG) {
            let opcodes_json = serde_json::to_string_pretty(
                &opcodes
                    .iter()
                    .map(|x| {
                        if x.arg_types().len() == 0 {
                            format!("{}", x.op_name())
                        } else {
                            format!(
                                "{}[{}]",
                                x.op_name(),
                                x.arg_types().iter().map(|x| x.to_string()).join(", ")
                            )
                        }
                    })
                    .collect_vec(),
            )
            .unwrap();
            debug!(target: "ruse::task_parser", { 
                opcodes.json = %opcodes_json,
                task_name = %self.name,
                opcodes_len = %opcodes.len()
            }, "Task {} Has {} opcodes", &self.name, opcodes.len());
        }

        if self.inner.options.pure {
            max_context_depth = 0;
        }
        if let Some(immutable) = &self.inner.immutable {
            if immutable.len() == variables.len() {
                max_context_depth = 0;
            }
        }

        let immutable_opt = self.inner.immutable;
        let syn_ctx = SynthesizerContext::from_context_array_with_data(context_array, self.classes);
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
                synthesizer.set_immutable(&root_name!(var.as_str()));
            }
        }

        Ok(synthesizer)
    }

    fn get_predicate(
        &self,
        engine_ctx: &mut EngineContext,
    ) -> Result<SynthesizerPredicate, SnythesisTaskError> {
        let variables = &self.inner.variables;
        let examples = &self.inner.examples;

        let mut predicate_graphs_map = GraphsMap::default();
        let predicate_gen_id = Arc::new(GraphIdGenerator::default());
        let output_array = match &self.inner.return_type {
            Some(return_type) => {
                let mut array = Vec::with_capacity(examples.len());
                for example in examples {
                    if let Some(output) = example.output.as_ref() {
                        let output = return_type.create_value(
                            output,
                            &self.classes,
                            predicate_gen_id.get_id_for_graph(),
                            &mut predicate_graphs_map,
                            &predicate_gen_id,
                            None,
                            engine_ctx,
                        )?;
                        array.push(output);
                    }
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
                        engine_ctx,
                    )?;
                    array.push(state_map);
                }
                Some(array)
            }
            None => None,
        };

        let predicate_js: Option<Vec<String>> = examples
            .iter()
            .map(|e| {
                e.predicate_js.as_ref().map(|predicate| {
                    if let Some(code_line) = predicate.as_str() {
                        return code_line.to_string();
                    }
                    let code_lines: Vec<String> =
                        serde_json::from_value(predicate.clone()).unwrap();
                    code_lines.join("")
                })
            })
            .collect();

        let output_type = self.inner.return_type.as_ref().map(|x| x.value_type());

        let builder = PredicateBuilder {
            output_type,
            output_array,
            state_array,
            predicate_js,
            graphs_map: predicate_graphs_map,
        };

        Ok(builder.finalize())
    }

    fn get_valid_predicate(&self) -> Result<SynthesizerPredicate, SnythesisTaskError> {
        let builder = ValidPredicateBuilder {
            max_string_size: self.inner.max_string_size.unwrap_or(30),
        };
        Ok(builder.finalize())
    }

    fn get_opcodes(&self, max_seq_size: usize) -> OpcodesList {
        let max_seq_size = self.inner.options.max_seq_size.unwrap_or(max_seq_size);

        let var_names: Vec<VariableName> = self
            .inner
            .variables
            .keys()
            .map(|x| root_name!(x.as_str()))
            .collect();

        let string_literals = if self.inner.options.strings {
            self.string_literals
                .iter()
                .map(|x| str_cached!(x.as_str()))
                .collect_vec()
        } else {
            vec![]
        };

        let mut opcodes =
            construct_opcode_list(&var_names, &self.num_literals, &string_literals, false);

        let composite_opcodes =
            Self::get_composite_opcodes(&self.classes, max_seq_size, self.inner.options.strings);

        opcodes.extend(composite_opcodes.into_iter().filter(self.get_filter()));

        opcodes
    }

    pub fn opcode_count(&self, max_seq_size: usize) -> usize {
        self.get_opcodes(max_seq_size).len()
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
        max_seq_size: usize,
        strings: bool,
    ) -> OpcodesList {
        let mut composite_opcodes = OpcodesList::new();
        add_num_opcodes(
            &mut composite_opcodes,
            &ALL_BIN_NUM_OPCODES,
            &ALL_UNARY_NUM_OPCODES,
            &ALL_UPDATE_NUM_OPCODES,
        );
        if strings {
            add_str_opcodes(&mut composite_opcodes, &ALL_BIN_STR_OPCODES);
        }
        add_array_opcodes(
            &mut composite_opcodes,
            &[ValueType::Number, ValueType::String],
        );
        add_dom_opcodes(&mut composite_opcodes);

        add_set_opcodes(
            &mut composite_opcodes,
            &[ValueType::Number, ValueType::String],
        );

        let mut value_types = vec![ValueType::Number, ValueType::String, ValueType::Null];
        value_types.extend(
            classes
                .classes_names()
                .map(|x| ValueType::class_value_type(x.clone())),
        );
        value_types.push(ValueType::array_value_type(&ValueType::Number));
        value_types.push(ValueType::array_value_type(&ValueType::String));
        value_types.push(ValueType::set_value_type(&ValueType::Number));
        value_types.push(ValueType::set_value_type(&ValueType::String));
        for size in 2..=max_seq_size {
            add_seq_opcodes(&mut composite_opcodes, size, &value_types);
        }

        composite_opcodes.extend(Self::get_classes_opcodes(classes, true));

        composite_opcodes
    }

    pub fn get_classes_opcodes(classes: &TsClasses, add_global: bool) -> OpcodesList {
        let mut composite_opcodes = OpcodesList::new();

        for class in classes.user_classes() {
            composite_opcodes.extend_from_slice(&class.member_opcodes);
            composite_opcodes.extend_from_slice(&class.method_opcodes);
            composite_opcodes.extend_from_slice(&class.constructor_opcodes);
        }

        if add_global {
            if let Some(global_class) = classes.get_global_class() {
                composite_opcodes.extend_from_slice(&global_class.variables_opcodes);
                composite_opcodes.extend_from_slice(&global_class.function_opcodes);
            }
        }

        composite_opcodes
    }

    fn get_context_array(
        &self,
        engine_ctx: &mut EngineContext,
    ) -> Result<ContextArray, SnythesisTaskError> {
        let variables = &self.inner.variables;
        let examples = &self.inner.examples;

        let mut values = Vec::with_capacity(examples.len());

        for example in examples {
            values.push(example.create_context(variables, &self.classes, engine_ctx)?);
        }

        Ok(values.into())
    }

    pub fn task_name(path: &Path) -> String {
        PathBuf::from(path.file_name().unwrap())
            .display()
            .to_string()
    }

    pub fn check_if_skipped(path: &Path) -> Result<(), SnythesisTaskError> {
        let reader = std::fs::File::open(path).map_err(|e| io_err!(e))?;
        let json: serde_json::Map<String, serde_json::Value> =
            serde_json::from_reader(reader).map_err(|e| parse_err!("json", e))?;
        if let Some(skip_reason) = json.get("skip") {
            return Err(skip_err!(skip_reason.as_str().unwrap()));
        }
        Ok(())
    }

    pub fn from_json_file(path: &Path) -> Result<SnythesisTask, SnythesisTaskError> {
        Self::check_if_skipped(path)?;

        let reader = std::fs::File::open(path).map_err(|e| io_err!(e))?;
        let mut inner: SnythesisTaskInner =
            serde_json::from_reader(reader).map_err(|e| parse_err!("json", e))?;
        inner.verify()?;
        inner.extend_examples();

        let variables = &inner.variables;

        let mut dir = PathBuf::from(path);
        dir.pop();

        let mut string_literals = HashSet::<_, BuildHasherDefault<DefaultHasher>>::from_iter(
            Self::DEFAULT_STRING_LITERALS.map(|x| x.to_string()),
        );
        if let Some(user_lit) = &inner.string_literals {
            string_literals = HashSet::from_iter(user_lit.clone());
        }

        let mut num_literals =
            HashSet::<_, BuildHasherDefault<DefaultHasher>>::from_iter(Self::DEFAULT_NUM_LITERALS);
        if let Some(user_lit) = &inner.int_literals {
            num_literals = HashSet::from_iter(user_lit.clone());
        }

        let mut builder = TsClassesBuilder::new();
        if let Some(classes_code) = &inner.classes {
            for code in classes_code {
                builder.add_classes(code).map_err(|e| parse_err!(code, e))?;
            }
        }

        if let Some(ts_files) = &inner.ts_files {
            for ts_file in ts_files {
                let full_path = match ts_file.is_relative() {
                    true => path.parent().unwrap().join(ts_file),
                    false => ts_file.clone(),
                };
                builder
                    .add_files(&full_path)
                    .map_err(|e| parse_err!(String::from(full_path.to_string_lossy()), e))?;
            }
        }

        let classes = builder.finalize();

        for (var, var_type) in variables {
            if let TaskType::Object(obj_type) = var_type {
                if classes
                    .get_user_class(&class_name!(obj_type.as_str()))
                    .is_none()
                {
                    return Err(verify_err!(
                        "Variable {} has an unknown object type {}",
                        var,
                        obj_type
                    ));
                }
            }
        }

        for example in &mut inner.examples {
            if let Some(return_type) = &inner.return_type {
                if let Some(output) = &mut example.output {
                    return_type.load_value(dir.as_path(), output)?;
                }
            }
            for (var, var_type) in variables {
                if example.input.contains_key(var) {
                    var_type.load_value(dir.as_path(), example.input.get_mut(var).unwrap())?;
                }
            }
        }

        if inner.version == 1 {
            inner.upgrade_from_version_1()?;
        }

        let oop_category = if inner.classes.is_some() || inner.ts_files.is_some() {
            SynthesisOOPCategory::FullOOP
        } else if inner.variables.values().any(|v| v.is_object())
            || inner
                .return_type
                .as_ref()
                .unwrap_or(&TaskType::Bool)
                .is_object()
        {
            SynthesisOOPCategory::PrimitiveObjects
        } else {
            SynthesisOOPCategory::Primitive
        };

        Ok(Self {
            path: path.into(),
            name: Self::task_name(path),
            string_literals,
            num_literals,
            classes,
            inner,
            oop_category,
        })
    }

    pub fn source(&self) -> Option<&String> {
        self.inner.source.as_ref()
    }

    pub fn category(&self) -> SnythesisTaskCategory {
        SnythesisTaskCategory {
            oop_category: self.oop_category,
            side_effects: self.inner.solution.has_side_effects,
        }
    }
}
