use std::collections::{HashMap, VecDeque};

use crate::{error::SnythesisTaskError, parse_err, task_type::TaskType, verify_err};
use itertools::Itertools;
use ruse_object_graph::{graph_walk, str_cached, value::Value, Cache, EdgeEndPoint, GraphsMap};
use ruse_synthesizer::context::ValuesMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VarRef {
    var: String,
    fields: Vec<String>,
}

impl VarRef {
    fn create_value(
        &self,
        values: &ValuesMap,
        graphs_map: &GraphsMap,
        cache: &Cache,
    ) -> Result<Value, SnythesisTaskError> {
        let value = values.get(self.var.as_str()).ok_or(parse_err!(
            format!("{}", self),
            "Pointing to an uninitialized value"
        ))?;
        self.walk_fields(value, graphs_map, cache)
    }

    fn walk_fields(
        &self,
        value: &Value,
        graphs_map: &GraphsMap,
        cache: &Cache,
    ) -> Result<Value, SnythesisTaskError> {
        if self.fields.is_empty() {
            Ok(value.clone())
        } else {
            let mut cur_value = value.clone();
            for field in &self.fields {
                if let Value::Object(obj) = cur_value {
                    cur_value = obj
                        .get_field_value(&str_cached!(cache; field), graphs_map)
                        .ok_or(parse_err!(
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
        let fields = iter.map(|x| x.to_string()).collect();
        VarRef { var, fields }
    }
}

impl std::fmt::Display for VarRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.fields.is_empty() {
            write!(f, "{}", &self.var)
        } else {
            write!(f, "{}.{}", &self.var, self.fields.iter().join("."))
        }
    }
}

pub(crate) fn verify_no_var_ref_circle(
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

pub(crate) const REF_GRAPH_ID: usize = usize::MAX;
pub(crate) const REF_GRAPH_FIELD_NAME: &str = "ref";
pub(crate) const REF_GRAPH_OBJ_TYPE: &str = "VARREF";

pub(crate) fn set_var_refs(
    variables: &HashMap<String, TaskType>,
    values: &mut ValuesMap,
    graphs_map: &mut GraphsMap,
    cache: &Cache,
) -> Result<(), SnythesisTaskError> {
    let mut refs = Vec::new();
    for (graph, node_id, node) in graph_walk::ObjectGraphWalker::from_graphs_map(graphs_map) {
        if graph.id == REF_GRAPH_ID {
            continue;
        }
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
                        graphs_map
                            .node_root_names(&node_id)
                            .map(|x| x.cloned().collect_vec()),
                        VarRef::from(var_ref.as_str()),
                    ));
                }
            }
        }
    }
    for (graph_id, node_id, field_name, root_name_opt, var_ref) in refs {
        let actual_value = var_ref.create_value(values, graphs_map, cache)?;
        let actual_obj = actual_value.obj().unwrap();

        graphs_map.set_edge(
            field_name,
            graph_id,
            node_id,
            actual_obj.graph_id,
            actual_obj.node,
        );
        if let Some(root_names) = root_name_opt {
            for r in root_names {
                graphs_map.set_as_root(r.clone(), actual_obj.graph_id, actual_obj.node);
                if values.contains_key(&r) {
                    values.insert(r, Value::Object(actual_obj.clone()));
                }
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
        if !values.contains_key(var_ref.var.as_str()) {
            var_refs.push_back((key, var_ref));
            continue;
        }
        let value = var_ref.create_value(&values, graphs_map, cache)?;
        if let Value::Object(obj_val) = &value {
            graphs_map.set_as_root(key.clone(), obj_val.graph_id, obj_val.node);
        }
        values.insert(key, value);
    }

    Ok(())
}
