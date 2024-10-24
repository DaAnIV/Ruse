use std::collections::{HashMap, HashSet, VecDeque};

use crate::context::{Context, ContextArray, GraphIdGenerator, VariableName};
use itertools::{self, izip};
use ruse_object_graph::{
    graph_equality, value::Value, vobj, GraphIndex, GraphsMap, NodeIndex, ObjectGraph,
};

#[derive(Default)]
struct NodeMatches {
    direction_1: HashMap<NodeIndex, (GraphIndex, NodeIndex)>,
    direction_2: HashMap<NodeIndex, (GraphIndex, NodeIndex)>,
}

enum MatchDirection {
    Graph1To2,
    Graph2To1,
}

struct NodeMatchesSingle<'a> {
    matches: &'a mut NodeMatches,
    direction: MatchDirection,
}

impl NodeMatches {
    fn add_match(&mut self, n_1: (GraphIndex, NodeIndex), n_2: (GraphIndex, NodeIndex)) {
        self.direction_1.insert(n_1.1, n_2);
        self.direction_2.insert(n_2.1, n_1);
    }

    fn get_match_1_to_2(&self, n_1: &NodeIndex) -> Option<&(GraphIndex, NodeIndex)> {
        self.direction_1.get(&n_1)
    }

    fn get_match_2_to_1(&self, n_2: &NodeIndex) -> Option<&(GraphIndex, NodeIndex)> {
        self.direction_2.get(n_2)
    }

    fn get_single_direction(&mut self, direction: MatchDirection) -> NodeMatchesSingle {
        NodeMatchesSingle {
            matches: self,
            direction: direction,
        }
    }
}

impl<'a> NodeMatchesSingle<'a> {
    fn add_match(&mut self, n_1: (GraphIndex, NodeIndex), n_2: (GraphIndex, NodeIndex)) {
        match self.direction {
            MatchDirection::Graph1To2 => self.matches.add_match(n_1, n_2),
            MatchDirection::Graph2To1 => self.matches.add_match(n_2, n_1),
        }
    }

    fn get_match(&self, n: &NodeIndex) -> Option<&(GraphIndex, NodeIndex)> {
        match self.direction {
            MatchDirection::Graph1To2 => self.matches.direction_1.get(n),
            MatchDirection::Graph2To1 => self.matches.direction_2.get(n),
        }
    }
}

pub fn merge_context_arrays(
    p_1_array: &ContextArray,
    q_1_array: &ContextArray,
    p_2_array: &ContextArray,
    q_2_array: &ContextArray,
) -> Result<(ContextArray, ContextArray), ()> {
    if q_1_array == p_2_array {
        let mut q_2_hat = q_2_array.clone();
        q_2_hat.extend_graphs_map(q_1_array);
        return Ok((p_1_array.clone(), q_2_hat));
    }

    let mut merged_pre_ctx_vec = Vec::with_capacity(p_1_array.len());
    let mut merged_post_ctx_vec = Vec::with_capacity(p_1_array.len());

    for (p_1, q_1, p_2, q_2) in izip!(
        p_1_array.iter(),
        q_1_array.iter(),
        p_2_array.iter(),
        q_2_array.iter()
    ) {
        let (merged_pre_ctx, merged_post_ctx) = merge_context(p_1, q_1, p_2, q_2)?;
        merged_pre_ctx_vec.push(merged_pre_ctx);
        merged_post_ctx_vec.push(merged_post_ctx);
    }

    Ok((ContextArray::from(merged_pre_ctx_vec), ContextArray::from(merged_post_ctx_vec)))
}

pub(crate) fn merge_context(
    p_1: &Context,
    q_1: &Context,
    p_2: &Context,
    q_2: &Context,
) -> Result<(Context, Context), ()> {
    let mut p_1_hat = p_1.values.as_ref().clone();
    let mut q_2_hat = q_2.values.as_ref().clone();
    let mut p_1_map_hat = p_1.graphs_map.as_ref().clone();
    let mut q_2_map_hat = q_2.graphs_map.as_ref().clone();
    // let mut p_1_hat_graph = Arc::new(p_1.graph.as_ref().clone());
    // let mut q_2_hat_graph = Arc::new(q_2.graph.as_ref().clone());

    let mut pre_values = HashMap::<VariableName, Value>::new();
    let mut post_values = HashMap::<VariableName, Value>::new();

    let mut intersection = Vec::new();
    let mut only_p_2 = Vec::new();
    let mut only_q_1 = Vec::new();

    let mut variables = HashSet::new();
    variables.extend(q_1.variables());
    variables.extend(p_2.variables());

    for var in variables {
        match (q_1.get_var_loc_value(var), p_2.get_var_loc_value(var)) {
            (None, Some(pre_val_2)) => match pre_val_2.val() {
                Value::Primitive(_) => {
                    pre_values.insert(var.clone(), pre_val_2.val().clone());
                }
                Value::Object(o) => only_p_2.push((var, o.clone())),
            },
            (Some(post_val_1), None) => match post_val_1.val() {
                Value::Primitive(_) => {
                    post_values.insert(var.clone(), post_val_1.val().clone());
                }
                Value::Object(o) => only_q_1.push((var, o.clone())),
            },
            (Some(post_val_1), Some(pre_val_2)) => match (post_val_1.val(), pre_val_2.val()) {
                (Value::Primitive(prim_1), Value::Primitive(prim_2)) => {
                    if prim_1 != prim_2 {
                        return Err(());
                    }
                    if let Some(pre_val_1) = p_1.get_var_loc_value(var) {
                        pre_values.insert(var.clone(), pre_val_1.val().clone());
                    }
                }
                (Value::Object(o_1), Value::Object(o_2)) => {
                    intersection.push((var, o_1.clone(), o_2.clone()))
                }
                (_, _) => return Err(()),
            },
            (None, None) => continue,
        }
    }

    let mut matches = NodeMatches::default();

    for (_, o_1, o_2) in &intersection {
        if !sim_walk_equal(
            &q_1.graphs_map,
            o_1.graph_id,
            o_1.node,
            &p_2.graphs_map,
            o_2.graph_id,
            o_2.node,
            &mut matches,
        ) {
            return Err(());
        }
    }
    for (var, o_2) in &only_p_2 {
        let mut new_graph = ObjectGraph::new(p_1.graph_id_gen.get_id_for_graph());
        if !embed(
            &mut new_graph,
            &p_2.graph_id_gen,
            &p_2.graphs_map,
            o_2.graph_id,
            o_2.node,
            triplet_new_nodes(p_1, q_1),
            &mut matches.get_single_direction(MatchDirection::Graph2To1),
        ) {
            return Err(());
        }
        let new_var_value = matches.get_match_2_to_1(&o_2.node).unwrap();
        debug_assert!(new_var_value.0 == new_graph.id);
        new_graph.set_as_root((*var).clone(), new_var_value.1);
        p_1_map_hat.insert_graph(new_graph.into());
        p_1_hat.insert((*var).clone(), vobj!(new_var_value.0, new_var_value.1));
    }
    for (var, o_1) in &only_q_1 {
        let mut new_graph = ObjectGraph::new(q_2.graph_id_gen.get_id_for_graph());
        if !embed(
            &mut new_graph,
            &q_1.graph_id_gen,
            &q_1.graphs_map,
            o_1.graph_id,
            o_1.node,
            HashSet::new(),
            &mut matches.get_single_direction(MatchDirection::Graph1To2),
        ) {
            return Err(());
        }
        let new_var_value = matches.get_match_1_to_2(&o_1.node).unwrap();
        new_graph.set_as_root((*var).clone(), new_var_value.1);
        q_2_map_hat.insert_graph(new_graph.into());
        q_2_hat.insert((*var).clone(), vobj!(new_var_value.0, new_var_value.1));
    }

    q_2_map_hat.extend(&q_1.graphs_map);

    Ok((
        Context::with_values(p_1_hat, p_1_map_hat.into(), p_1.graph_id_gen.clone()),
        Context::with_values(q_2_hat, q_2_map_hat.into(), q_2.graph_id_gen.clone()),
    ))
}

fn triplet_new_nodes(p_ctx: &Context, q_ctx: &Context) -> HashSet<NodeIndex> {
    q_ctx
        .reachable_nodes()
        .difference(&p_ctx.reachable_nodes())
        .map(|x| *x)
        .collect()
}

fn embed(
    new_graph: &mut ObjectGraph,
    graph_id_gen: &GraphIdGenerator,
    graphs_map: &GraphsMap,
    graph_id: GraphIndex,
    node_id: NodeIndex,
    new: HashSet<NodeIndex>,
    matches: &mut NodeMatchesSingle,
) -> bool {
    if matches.get_match(&node_id).is_some() {
        return true;
    }

    let mut q = VecDeque::new();
    let new_graph_id = new_graph.id;

    q.push_back((graph_id, node_id, graph_id_gen.get_id_for_node()));
    while let Some((cur_graph_id, cur_node_id, new_id)) = q.pop_front() {
        let graph = &graphs_map[cur_graph_id];
        let node = graph.get_node(&cur_node_id).unwrap();
        let new_node = new_graph.add_node(new_id, node.obj_type.clone(), node.fields.clone());
        matches.add_match((cur_graph_id, cur_node_id), (new_graph_id, new_node.id));
        for (field_name, old_neig) in &node.pointers {
            if let Some((new_neig_graph, new_neig_id)) = matches.get_match(old_neig.index()) {
                if new.contains(new_neig_id) {
                    return false;
                }
                if *new_neig_graph == new_graph_id {
                    new_node.insert_internal_edge(field_name.clone(), *new_neig_id);
                } else {
                    new_node.insert_chain_edge(field_name.clone(), *new_neig_graph, *new_neig_id);
                }
            } else {
                let new_neig_id = graph_id_gen.get_id_for_node();
                new_node.insert_internal_edge(field_name.clone(), new_neig_id);
                match old_neig {
                    ruse_object_graph::EdgeEndPoint::Internal(old_neig_node) => {
                        q.push_back((cur_graph_id, *old_neig_node, new_neig_id))
                    }
                    ruse_object_graph::EdgeEndPoint::Chain(old_neig_graph, old_neig_node) => {
                        q.push_back((*old_neig_graph, *old_neig_node, new_neig_id))
                    }
                }
            }
        }
    }

    true
}

fn sim_walk_equal(
    graphs_map_1: &GraphsMap,
    graph_1_id: GraphIndex,
    n_1: NodeIndex,
    graphs_map_2: &GraphsMap,
    graph_2_id: GraphIndex,
    n_2: NodeIndex,
    matches: &mut NodeMatches,
) -> bool {
    let mut nodes_1_to_2 = HashMap::default();
    if !graph_equality::sim_walk_equal(
        graphs_map_1,
        graph_1_id,
        n_1,
        graphs_map_2,
        graph_2_id,
        n_2,
        &mut nodes_1_to_2,
    ) {
        return false;
    }
    for (a, b) in nodes_1_to_2 {
        matches.add_match(a, b);
    }
    true
}
