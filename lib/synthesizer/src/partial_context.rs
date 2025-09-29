use std::collections::HashSet;
use std::sync::Arc;

use ruse_object_graph::graph_walk::ObjectGraphWalker;
use ruse_object_graph::GraphsMap;
use ruse_object_graph::{connected_components::GraphsMapWeakComponents, value::Value};

use crate::context::*;

impl Context {
    #[cfg(feature = "prune_graph_map")]
    fn get_partial_graphs_map(&self, _connected_variables: &HashSet<VariableName>) -> Arc<GraphsMap> {
        self.graphs_map.get_graphs_for_roots(_connected_variables.into_iter()).into()
    }
    
    #[cfg(not(feature = "prune_graph_map"))]
    fn get_partial_graphs_map(&self, _connected_variables: &HashSet<VariableName>) -> Arc<GraphsMap> {
        self.graphs_map.clone()
    }

    pub fn get_partial_context(&self, connected_variables: HashSet<VariableName>) -> Option<Box<Self>> {
        let mut values = ValuesMap::default();
        let mut hashes = ValuesHashMap::default();

        for var in &connected_variables {
            let var_value = self.values.get(var)?.clone();
            let var_hash = unsafe { *self.hashes.get(var).unwrap_unchecked() };
            values.insert(var.clone(), var_value);
            hashes.insert(var.clone(), var_hash);
        }

        let partial_graphs_map = self.get_partial_graphs_map(&connected_variables);

        Some(
            Self {
                hashes: hashes.into(),
                values: values.into(),
                graphs_map: partial_graphs_map,
                graph_id_gen: self.graph_id_gen.clone(),
                outputs: Vec::new().into(),
            }
            .into(),
        )
    }
}

pub(crate) struct PartialContextBuilder<'a> {
    context_array: &'a ContextArray,
    weak_components: Option<Vec<GraphsMapWeakComponents>>,
}

impl<'a> PartialContextBuilder<'a> {    
    fn get_connected_variables_with_closure(
        ctx: &Context,
        required_variables: &[VariableName],
        weak_components: &GraphsMapWeakComponents,
    ) -> Option<HashSet<VariableName>> {
        let mut connected_variables = HashSet::new();

        for var in required_variables {
            connected_variables.insert(var.clone());

            if let Value::Object(obj_val) = ctx.values.get(var)? {
                for (name, other_obj_val) in ctx
                    .values
                    .iter()
                    .filter_map(|(name, val)| Some((name, val.obj()?)))
                {
                    if weak_components.is_connected(obj_val.node, other_obj_val.node) {
                        connected_variables.insert(name.clone());
                    }
                }
            }
        }

        Some(connected_variables)
    }

    fn get_connected_variables_with_reachable(
        ctx: &Context,
        required_variables: &[VariableName],
    ) -> Option<HashSet<VariableName>> {
        let mut connected_variables = HashSet::from_iter(required_variables.iter().cloned());
        let walker = ObjectGraphWalker::from_nodes(
            &ctx.graphs_map,
            connected_variables.iter().filter_map(|var| {
                let root = ctx.graphs_map.get_root(var)?;
                Some((root.graph, root.node))
            }),
        );
        for (_, node, _) in walker {
            if let Some(root_names) = ctx.graphs_map.node_root_names(&node) {
                connected_variables.extend(root_names.cloned());
            }
        }

        Some(connected_variables)
    }
}

#[allow(dead_code)]
impl<'a> PartialContextBuilder<'a> {
    pub fn new_with_closure(context_array: &'a ContextArray) -> Self {
        let weak_components = Self::compute_weak_components(context_array);
        Self {
            context_array,
            weak_components: Some(weak_components),
        }
    }

    pub fn new_with_reachable(context_array: &'a ContextArray) -> Self {
        Self {
            context_array,
            weak_components: None,
        }
    }

    fn compute_weak_components(context_array: &'a ContextArray) -> Vec<GraphsMapWeakComponents> {
        context_array
            .iter()
            .map(|ctx| GraphsMapWeakComponents::from_graphs_map(&ctx.graphs_map))
            .collect()
    }

    pub fn get_partial_context(&self, required_variables: &[VariableName]) -> Option<ContextArray> {
        let mut ctxs = Vec::<Box<Context>>::with_capacity(self.context_array.len());

        if let Some(weak_components_vec) = &self.weak_components {
            for (ctx, weak_components) in self.context_array.iter().zip(weak_components_vec.iter())
            {
                let connected_variables =
                    Self::get_connected_variables_with_closure(ctx, required_variables, weak_components)?;
                ctxs.push(ctx.get_partial_context(connected_variables)?);
            }
        } else {
            for ctx in self.context_array.iter() {
                let connected_variables =
                    Self::get_connected_variables_with_reachable(ctx, required_variables)?;
                ctxs.push(ctx.get_partial_context(connected_variables)?);
            }
        }

        Some(ContextArray {
            inner: ctxs,
            depth: self.context_array.depth,
        })
    }
}

impl<'a> PartialContextBuilder<'a> {
    #[cfg(feature = "take_variable_closure")]
    pub fn new(context_array: &'a ContextArray) -> Self {
        Self::new_with_closure(context_array)
    }

    #[cfg(not(feature = "take_variable_closure"))]
    pub fn new(context_array: &'a ContextArray) -> Self {
        Self::new_with_reachable(context_array)
    }
}
