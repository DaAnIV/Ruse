use crate::{
    graph_equality::{self, NodeMatcherMap},
    graph_map_value::*,
    value::Value,
    Attributes, FieldName, GraphIndex, GraphsMap, NodeIndex, RootName,
};
use std::{fmt::Debug, fmt::Display, hash::Hash};

#[derive(Debug, Clone)]
pub struct ObjectFieldLoc {
    pub graph: GraphIndex,
    pub node: NodeIndex,
    pub field: FieldName,
    pub attrs: Attributes,
}

impl Eq for ObjectFieldLoc {}
impl PartialEq for ObjectFieldLoc {
    fn eq(&self, other: &Self) -> bool {
        self.graph == other.graph && self.node == other.node && self.field == other.field
    }
}
impl Hash for ObjectFieldLoc {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.graph.hash(state);
        self.node.hash(state);
        self.field.hash(state);
    }
}

#[derive(Debug, Clone)]
pub struct RootLoc {
    pub root: RootName,
    pub attrs: Attributes,
}

impl Eq for RootLoc {}
impl PartialEq for RootLoc {
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root
    }
}
impl Hash for RootLoc {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.root.hash(state);
    }
}

#[derive(Debug, Clone)]
pub enum Location {
    Temp,
    Root(RootLoc),
    ObjectField(ObjectFieldLoc),
}

#[derive(Debug, Clone)]
pub struct LocValue {
    pub loc: Location,
    pub val: Value,
}

impl Location {
    pub fn is_temp(&self) -> bool {
        matches!(&self, Location::Temp)
    }

    pub fn is_var(&self) -> bool {
        matches!(&self, Location::Root(_))
    }

    pub fn is_object_field(&self) -> bool {
        matches!(&self, Location::ObjectField(_))
    }

    pub fn var(&self) -> Option<&'_ RootLoc> {
        match &self {
            Location::Root(l) => Some(l),
            _ => None,
        }
    }

    pub fn object_field(&self) -> Option<&'_ ObjectFieldLoc> {
        match &self {
            Location::ObjectField(l) => Some(l),
            _ => None,
        }
    }

    fn get_node_id(&self, graphs_map: &GraphsMap) -> Option<(GraphIndex, NodeIndex)> {
        match self {
            Location::Root(l) => Some(
                graphs_map
                    .get_root(&l.root)
                    .map(|r| (r.graph, r.node))
                    .unwrap(),
            ),
            Location::ObjectField(l) => Some(
                graphs_map
                    .get(&l.graph)
                    .and_then(|g| g.get_neighbor_id(&l.node, &l.field))
                    .unwrap_or((l.graph, l.node)),
            ),
            Location::Temp => None,
        }
    }
}

impl Location {
    fn eq(
        &self,
        self_graphs_map: &GraphsMap,
        other: &Self,
        other_graphs_map: &GraphsMap,
        is_primitive: bool,
    ) -> bool {
        if self.is_temp() || other.is_temp() {
            return self.is_temp() == other.is_temp();
        }            
        match (self, other) {
            (Location::Root(self_root_loc), Location::Root(other_root_loc)) => {
                if self_root_loc.root == other_root_loc.root {
                    return true;
                }
            }
            _ => ()
        }

        if is_primitive {
            match (self, other) {
                (Location::Root(_), Location::Root(_)) => {
                    return false;
                }
                (
                    Location::ObjectField(self_object_field_loc),
                    Location::ObjectField(other_object_field_loc),
                ) => {
                    if self_object_field_loc.field != other_object_field_loc.field {
                        return false;
                    }
                }
                _ => return false,
            };
        }

        let self_node = self.get_node_id(self_graphs_map).unwrap();
        let other_node = other.get_node_id(other_graphs_map).unwrap();

        let mut equal_nodes = NodeMatcherMap::new();
        equal_nodes.insert(self_node, other_node);
        return graph_equality::equal_graphs_by_root_names_with_map(
            self_graphs_map,
            other_graphs_map,
            self_graphs_map.common_roots(&other_graphs_map),
            &mut equal_nodes,
        );
    }
}

impl Hash for Location {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Location::Temp => state.write_u8(0),
            _ => state.write_u8(1),
        }
    }
}

impl Display for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Location::Temp => write!(f, "Temp"),
            Location::Root(root_loc) => write!(f, "{}", root_loc.root),
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

    #[inline]
    pub fn readonly(&self) -> bool {
        match &self.loc() {
            Location::Temp => false,
            Location::Root(root_loc) => root_loc.attrs.readonly,
            Location::ObjectField(object_field_loc) => object_field_loc.attrs.readonly,
        }
    }

    pub fn get_obj_field_loc_value(
        &self,
        graphs_map: &GraphsMap,
        field_name: &FieldName,
    ) -> Option<Self> {
        let obj = self.val().obj()?;
        let field = obj.get_field_value(field_name, graphs_map)?;
        let attrs = obj.get_field_attrs(field_name, graphs_map)?;
        let loc = match &self.loc() {
            Location::Temp => Location::Temp,
            Location::Root(_l) => Location::ObjectField(ObjectFieldLoc {
                graph: obj.graph_id,
                node: obj.node,
                field: field_name.clone(),
                attrs,
            }),
            Location::ObjectField(_l) => Location::ObjectField(ObjectFieldLoc {
                graph: obj.graph_id,
                node: obj.node,
                field: field_name.clone(),
                attrs,
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
            && self.loc.eq(
                self_graphs_map,
                &other.loc,
                other_graphs_map,
                self.val.is_primitive(),
            )
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

#[macro_export]
macro_rules! temp_value {
    ($val:expr) => {
        $crate::location::LocValue {
            val: $val,
            loc: $crate::location::Location::Temp,
        }
    };
}
