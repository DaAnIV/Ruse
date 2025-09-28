use std::collections::HashSet;

#[cfg(not(feature = "dont_take_variable_closure"))]
use itertools::Itertools;

#[cfg(not(feature = "dont_take_variable_closure"))]
use ruse_object_graph::{connected_components::GraphsMapWeakComponents, value::Value};

use crate::context::*;

impl Context {
    fn get_partial_context<'a, I>(
        &self,
        required_variables: I,
        #[cfg(not(feature = "dont_take_variable_closure"))]
        weak_components: &GraphsMapWeakComponents,
    ) -> Option<Box<Self>>
    where
        I: IntoIterator<Item = &'a VariableName>,
    {
        let mut values = ValuesMap::default();
        let mut hashes = ValuesHashMap::default();

        let mut connected_variables = HashSet::new();
        for var in required_variables {
            connected_variables.insert(var);
            #[cfg(not(feature = "dont_take_variable_closure"))]
            if let Value::Object(obj_val) = self.values.get(var)? {
                for (name, other_obj_val) in self
                    .values
                    .iter()
                    .filter_map(|(name, val)| Some((name, val.obj()?)))
                {
                    if weak_components.is_connected(obj_val.node, other_obj_val.node) {
                        connected_variables.insert(name);
                    }
                }
            }
        }

        for var in connected_variables.iter().map(|var| *var) {
            let var_value = self.values.get(var)?.clone();
            let var_hash = unsafe { *self.hashes.get(var).unwrap_unchecked() };
            values.insert(var.clone(), var_value);
            hashes.insert(var.clone(), var_hash);
        }

        #[cfg(feature = "prune_graph_map")]
        let partial_graphs_map = self
            .graphs_map
            .get_graphs_for_roots(connected_variables.into_iter());
        #[cfg(not(feature = "prune_graph_map"))]
        let partial_graphs_map = self.graphs_map.clone();

        Some(
            Self {
                hashes: hashes.into(),
                values: values.into(),
                graphs_map: partial_graphs_map.into(),
                graph_id_gen: self.graph_id_gen.clone(),
                outputs: Vec::new().into(),
            }
            .into(),
        )
    }
}

impl ContextArray {
    fn get_partial_context<'a, I>(
        &self,
        required_variables: I,
        #[cfg(not(feature = "dont_take_variable_closure"))] weak_components: &Vec<
            GraphsMapWeakComponents,
        >,
    ) -> Option<Self>
    where
        I: IntoIterator<Item = &'a VariableName> + Copy,
    {
        let mut ctxs = Vec::<Box<Context>>::with_capacity(self.len());

        #[cfg(not(feature = "dont_take_variable_closure"))]
        for (ctx, ctx_weak_components) in self.iter().zip_eq(weak_components.iter()) {
            ctxs.push(ctx.get_partial_context(required_variables, ctx_weak_components)?);
        }
        #[cfg(feature = "dont_take_variable_closure")]
        for ctx in self.iter() {
            ctxs.push(ctx.get_partial_context(required_variables)?);
        }

        Some(Self {
            inner: ctxs,
            depth: self.depth,
        })
    }    

    #[cfg(not(feature = "dont_take_variable_closure"))]
    fn compute_weak_components(&self) -> Vec<GraphsMapWeakComponents> {
        self.inner
            .iter()
            .map(|ctx| GraphsMapWeakComponents::from_graphs_map(&ctx.graphs_map))
            .collect()
    }
}

pub struct PartialContextBuilder<'a> {
    context_array: &'a ContextArray,
    #[cfg(not(feature = "dont_take_variable_closure"))]
    weak_components: Vec<GraphsMapWeakComponents>,
}

impl<'a> PartialContextBuilder<'a> {
    pub fn new(context_array: &'a ContextArray) -> Self {
        Self {
            context_array,
            #[cfg(not(feature = "dont_take_variable_closure"))]
            weak_components: context_array.compute_weak_components(),
        }
    }

    pub fn get_partial_context(&self, required_variables: &[VariableName]) -> Option<ContextArray> {
        self.context_array.get_partial_context(
            required_variables,
            #[cfg(not(feature = "dont_take_variable_closure"))]
            &self.weak_components,
        )
    }
}
