use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    hash::{BuildHasherDefault, DefaultHasher},
    path::{Path, PathBuf},
    sync::Arc,
};

use itertools::Itertools;
use ruse_object_graph::{
    value::{Value, ValueType},
    *,
};
use ruse_synthesizer::{
    bank::ProgBank,
    context::{Context, ContextArray, GraphIdGenerator, SynthesizerContext, ValuesMap},
    opcode::{ExprOpcode, OpcodesList},
    synthesizer::SynthesizerPredicate,
};
use ruse_ts_interpreter::ts_class::{TsClasses, TsClassesBuilder};
use ruse_ts_synthesizer::*;

use serde::{Deserialize, Serialize};
use wildmatch::WildMatch;

use crate::{
    error::SnythesisTaskError,
    parse_err,
    predicate_builder::{PredicateBuilder, ValidPredicateBuilder},
    skip_err,
    task_type::{JsonValuesMap, TaskType},
    var_ref::{set_var_refs, verify_no_var_ref_circle, REF_GRAPH_ID},
    verify_err, BankConfig,
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
            None => return Err(verify_err!("{} type is unknown", k)),
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

pub(crate) fn parse_json_values_map<'a, M>(
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
            None => return Err(verify_err!("{} type is unknown", k)),
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

pub(crate) fn parse_json_values_array<'a, V, T>(
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
    #[serde(skip_serializing_if = "Option::is_none")]
    common: Option<SnythesisTaskExamples>,
}

impl SnythesisTaskInner {
    fn extend_examples(&mut self) {
        if let Some(common) = &self.common {
            for example in self.examples.as_mut().unwrap().iter_mut() {
                example.extend(common);
            }
        }
    }

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
            return Err(verify_err!(
                "All examples must define at least one input"
            ));
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

        if self.return_type.is_some()
            && (has_common_output || examples.iter().any(|x| x.output.is_none()))
        {
            return Err(verify_err!(
                "All examples should have an output if the return type is given"
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
        for example in self.examples.as_mut().unwrap().iter_mut() {
            example.upgrade_from_version_1(&self.variables.as_ref().unwrap(), &self.return_type)?
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct SnythesisTask {
    inner: SnythesisTaskInner,
    classes: Box<TsClasses>,
    class_names: Vec<ObjectType>,
    pub string_literals: HashSet<String, BuildHasherDefault<DefaultHasher>>,
    pub num_literals: HashSet<i64, BuildHasherDefault<DefaultHasher>>,
}

impl SnythesisTask {
    const DEFAULT_STRING_LITERALS: [&str; 2] = ["", " "];
    const DEFAULT_NUM_LITERALS: [i64; 2] = [0, 1];

    pub fn get_synthesizer(
        self,
        mut max_context_depth: usize,
        iteration_workers_count: usize,
        bank_config: BankConfig,
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

        let bank = bank_config.new_bank();

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

        let builder = PredicateBuilder {
            output_array,
            state_array,
            predicate_js,
            graphs_map: predicate_graphs_map,
            cache: cache.clone(),
        };

        Ok(builder.finalize())
    }

    fn get_valid_predicate(
        &self,
        _cache: &Cache,
    ) -> Result<SynthesizerPredicate, SnythesisTaskError> {
        let builder = ValidPredicateBuilder {};
        Ok(builder.finalize())
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
        inner.extend_examples();

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
                    return Err(verify_err!(
                        "Variable {} has an unknown object type {}",
                        var,
                        obj_type
                    ));
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
}
