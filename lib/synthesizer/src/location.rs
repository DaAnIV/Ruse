use ruse_object_graph::{
    graph_map_value::*, value::Value, CachedString, GraphIndex, GraphsMap, NodeIndex,
};
use std::{fmt::Debug, fmt::Display, hash::Hash};

#[derive(PartialEq, Eq, Debug, Clone, Hash)]
pub struct ObjectFieldLoc {
    pub graph: GraphIndex,
    pub node: NodeIndex,
    pub field: CachedString,
}

#[derive(PartialEq, Eq, Debug, Clone, Hash)]
pub struct VarLoc {
    pub var: CachedString,
}

#[derive(PartialEq, Eq, Debug, Clone, Hash)]
pub enum Location {
    Temp,
    Var(VarLoc),
    ObjectField(ObjectFieldLoc),
}

#[derive(Debug, Clone)]
pub struct LocValue {
    pub(crate) loc: Location,
    pub(crate) val: Value,
}

impl Location {
    pub fn is_temp(&self) -> bool {
        matches!(&self, Location::Temp)
    }

    pub fn is_var(&self) -> bool {
        matches!(&self, Location::Var(_))
    }

    pub fn is_object_field(&self) -> bool {
        matches!(&self, Location::ObjectField(_))
    }

    pub fn var(&self) -> Option<&'_ VarLoc> {
        match &self {
            Location::Var(l) => Some(l),
            _ => None,
        }
    }

    pub fn object_field(&self) -> Option<&'_ ObjectFieldLoc> {
        match &self {
            Location::ObjectField(l) => Some(l),
            _ => None,
        }
    }
}

impl Display for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Location::Temp => write!(f, "Temp"),
            Location::Var(var_loc) => write!(f, "{}", var_loc.var),
            Location::ObjectField(object_field_loc) => write!(
                f,
                "{}:{}.{}",
                object_field_loc.graph, object_field_loc.node, object_field_loc.field
            ),
        }
    }
}

impl LocValue {
    #[inline]
    pub fn val(&self) -> &Value {
        &self.val
    }

    #[inline]
    pub fn loc(&self) -> &Location {
        &self.loc
    }

    pub fn get_obj_field_loc_value(
        &self,
        graphs_map: &GraphsMap,
        field_name: &CachedString,
    ) -> Option<Self> {
        let obj = self.val().obj().unwrap();
        let field = obj.get_field_value(field_name, graphs_map)?;
        let loc = match &self.loc() {
            Location::Temp => Location::Temp,
            Location::Var(_l) => Location::ObjectField(ObjectFieldLoc {
                graph: obj.graph_id,
                node: obj.node,
                field: field_name.clone(),
            }),
            Location::ObjectField(_l) => Location::ObjectField(ObjectFieldLoc {
                graph: obj.graph_id,
                node: obj.node,
                field: field_name.clone(),
            }),
        };

        Some(Self { val: field, loc })
    }
}

impl GraphMapWrap<Self> for LocValue {
    fn wrap<'a>(&'a self, graphs_map: &'a GraphsMap) -> GraphMapValue<'a, Self> {
        GraphMapValue::from(&self, graphs_map)
    }
}

impl GraphMapEq for LocValue {
    fn eq(&self, self_graphs_map: &GraphsMap, other: &Self, other_graphs_map: &GraphsMap) -> bool {
        self.val.wrap(self_graphs_map) == other.val.wrap(other_graphs_map)
    }
}

impl GraphMapHash for LocValue {
    fn calculate_hash<H: std::hash::Hasher>(&self, state: &mut H, graphs_map: &GraphsMap) {
        self.val.wrap(graphs_map).hash(state)
    }
}

impl GraphMapDisplay for LocValue {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>, graphs_map: &GraphsMap) -> core::fmt::Result {
        write!(f, "{}({})", self.val.wrap(graphs_map), self.loc)
    }
}
