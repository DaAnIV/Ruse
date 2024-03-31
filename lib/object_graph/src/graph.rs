use bitcode;
use petgraph::stable_graph::StableDiGraph;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::iter::zip;
use std::ptr;
use std::sync::Arc;

// Reexport
pub use petgraph::graph::{EdgeIndex, NodeIndex};

use crate::PrimitiveValue;

use super::{FieldsMap, ObjectData};

pub type GraphType = StableDiGraph<ObjectData, Arc<String>>;
pub type RootsMap = BTreeMap<Arc<String>, NodeIndex>;

pub struct ObjectGraph {
    pub(super) graph: GraphType,
    pub(crate) roots: RootsMap,

    serialized_buffer: bitcode::Buffer,
    pub(super) serialized: Option<Vec<u8>>,
    pub(super) hash: u64,
}

#[derive(bitcode::Encode)]
struct SerializableNode {
    id: usize,
    fields: Arc<FieldsMap>,
    pointers: Vec<(u64, usize)>,
}

#[derive(bitcode::Encode)]
struct SerializableGraph {
    roots: Vec<(u64, usize)>,
    nodes: Vec<SerializableNode>,
}

impl ObjectGraph {
    pub fn new() -> Self {
        ObjectGraph {
            graph: Default::default(),
            roots: Default::default(),
            serialized_buffer: bitcode::Buffer::new(),
            serialized: None,
            hash: 0,
        }
    }

    pub fn with_capacity(nodes: usize, edges: usize) -> Self {
        ObjectGraph {
            graph: GraphType::with_capacity(nodes, edges),
            roots: Default::default(),
            serialized_buffer: bitcode::Buffer::new(),
            serialized: None,
            hash: 0,
        }
    }

    pub fn add_node(&mut self, data: ObjectData) -> NodeIndex {
        self.serialized = None;
        self.graph.add_node(data)
    }

    fn serialize_graph_from_root(g: &ObjectGraph, r: &NodeIndex) -> Vec<u8> {
        let mut seen = HashMap::with_capacity(g.node_count());
        let mut graph = SerializableGraph {
            roots: Vec::with_capacity(0),
            nodes: Vec::with_capacity(g.node_count()),
        };

        Self::add_node_to_serializable_graph(g, r, &mut seen, &mut graph);
        Self::fix_serializable_graph_edges(&mut graph, seen);

        bitcode::encode(&graph).expect("Failed to serialize")
    }

    fn slow_equal_roots(a: (&ObjectGraph, &NodeIndex), b: (&ObjectGraph, &NodeIndex)) -> bool {
        Self::serialize_graph_from_root(a.0, a.1) == Self::serialize_graph_from_root(b.0, b.1)
    }

    fn union_visit_node(
        out: &mut ObjectGraph,
        g: &ObjectGraph,
        n: NodeIndex,
        q: &mut VecDeque<NodeIndex>,
        seen: &mut HashMap<(u64, NodeIndex), NodeIndex>,
        pointers: &mut Vec<(NodeIndex, (u64, NodeIndex), Arc<String>)>,
    ) -> NodeIndex {
        let ptr = ptr::addr_of!(g) as u64;

        if let Some(out_idx) = seen.get(&(ptr, n)) {
            return *out_idx;
        }

        let node = g.node_weight(n).unwrap();
        let new_object_data = ObjectData::new(node.fields.clone());
        let new_node = out.add_node(new_object_data);

        seen.insert((ptr, n), new_node);
        for (e, nei) in node.pointers.values() {
            pointers.push((new_node, (ptr, *nei), g.edge_weight(*e).unwrap()));
            q.push_back(*nei);
        }

        new_node
    }

    pub fn union(graphs: &[ObjectGraph]) -> ObjectGraph {
        let mut seen = HashMap::new();
        let mut pointers = vec![];
        let mut q = VecDeque::new();
        let mut out = graphs[0].clone();

        for g in graphs.iter().skip(1) {
            for r in &g.roots {
                if out.roots.contains_key(r.0) {
                    debug_assert!(Self::slow_equal_roots((g, r.1), (&out, &out.roots[r.0])))
                } else {
                    let new_node_idx =
                        Self::union_visit_node(&mut out, g, *r.1, &mut q, &mut seen, &mut pointers);
                    out.set_as_root(r.0.clone(), new_node_idx);
                    while let Some(n) = q.pop_front() {
                        Self::union_visit_node(&mut out, g, n, &mut q, &mut seen, &mut pointers);
                    }
                }
            }
        }

        for (new_source, old_target, name) in pointers {
            out.add_edge(new_source, seen[&old_target], name);
        }

        out
    }

    pub fn remove_node(&mut self, idx: NodeIndex) -> Option<ObjectData> {
        self.serialized = None;
        self.graph.remove_node(idx)
    }

    pub fn set_field(&mut self, object: NodeIndex, field: PrimitiveValue, field_name: Arc<String>) {
        // Todo: Handle field being a pointer
        self.serialized = None;
        let node = self.graph.node_weight_mut(object).unwrap();
        let mut new_fields = (*node.fields).clone();
        new_fields.insert(field_name, field);
        node.fields = new_fields.into();
    }

    pub fn remove_field(&mut self, object: NodeIndex, field_name: &Arc<String>) {
        self.serialized = None;
        let node = self.graph.node_weight_mut(object).unwrap();
        let mut new_fields = (*node.fields).clone();
        new_fields.remove(field_name);
        node.fields = new_fields.into();
    }

    pub fn add_edge(
        &mut self,
        object: NodeIndex,
        field: NodeIndex,
        field_name: Arc<String>,
    ) -> EdgeIndex {
        self.serialized = None;
        let edge = self.graph.add_edge(object, field, field_name.clone());
        let node = self.graph.node_weight_mut(object).unwrap();
        let mut new_pointers = (*node.pointers).clone();
        new_pointers.insert(field_name.clone(), (edge, field));
        node.pointers = new_pointers.into();
        return edge;
    }

    pub fn remove_edge(&mut self, idx: EdgeIndex) -> Option<Arc<String>> {
        self.serialized = None;
        let (node_idx, _) = self.graph.edge_endpoints(idx)?;
        let field_name = self.edge_weight(idx).unwrap().clone();
        let node = self.graph.node_weight_mut(node_idx).unwrap();
        let mut new_pointers = (*node.pointers).clone();
        new_pointers.remove(&field_name);
        node.pointers = new_pointers.into();
        self.graph.remove_edge(idx)
    }

    pub fn node_weight(&self, a: NodeIndex) -> Option<&ObjectData> {
        self.graph.node_weight(a)
    }

    pub fn edge_weight(&self, a: EdgeIndex) -> Option<Arc<String>> {
        match self.graph.edge_weight(a) {
            Some(e) => Some(e.clone()),
            None => None,
        }
    }

    pub fn add_root(&mut self, r: Arc<String>, data: ObjectData) -> NodeIndex {
        self.serialized = None;
        let index = self.add_node(data);
        self.roots.insert(r, index);
        return index;
    }

    pub fn set_as_root(&mut self, r: Arc<String>, index: NodeIndex) {
        self.serialized = None;
        self.roots.insert(r, index);
    }

    pub fn remove_root(&mut self, r: &Arc<String>) {
        self.serialized = None;
        self.roots.remove(r);
    }

    pub fn get_root(&self, r: &Arc<String>) -> NodeIndex {
        self.roots[r]
    }

    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    pub fn generate_serialized_data(&mut self) -> Result<(), bitcode::Error> {
        let mut seen = HashMap::with_capacity(self.graph.node_count());
        let mut graph = SerializableGraph {
            roots: Vec::with_capacity(self.roots.len()),
            nodes: Vec::with_capacity(self.node_count()),
        };

        for root in self.roots.values() {
            Self::add_node_to_serializable_graph(&self, root, &mut seen, &mut graph);
        }

        for (k, root) in &self.roots {
            graph.roots.push((Arc::as_ptr(&k) as u64, seen[root]));
        }

        Self::fix_serializable_graph_edges(&mut graph, seen);

        self.serialized = Some(self.serialized_buffer.encode(&graph)?.to_vec());

        let mut s = DefaultHasher::default();
        self.serialized.hash(&mut s);
        self.hash = s.finish();

        Ok(())
    }

    fn add_node_to_serializable_graph(
        g: &ObjectGraph,
        root: &NodeIndex,
        seen: &mut HashMap<NodeIndex, usize>,
        graph: &mut SerializableGraph,
    ) {
        let mut q: VecDeque<NodeIndex> = VecDeque::new();

        q.push_back(*root);
        while let Some(node_idx) = q.pop_front() {
            if seen.insert(node_idx, graph.nodes.len()).is_none() {
                let node = g.node_weight(node_idx).unwrap();

                graph.nodes.push({
                    SerializableNode {
                        id: graph.nodes.len(),
                        fields: node.fields.clone(),
                        pointers: node
                            .pointers
                            .iter()
                            .map(|(k, v)| (Arc::as_ptr(&k) as u64, v.1.index()))
                            .collect(),
                    }
                });

                for (_, nei) in node.pointers.values() {
                    q.push_back(*nei);
                }
            }
        }
    }

    fn fix_serializable_graph_edges(
        graph: &mut SerializableGraph,
        seen: HashMap<NodeIndex, usize>,
    ) {
        graph.nodes.iter_mut().for_each(|x| {
            for p in x.pointers.iter_mut() {
                let old_idx = NodeIndex::<u32>::new(p.1);
                p.1 = seen[&old_idx];
            }
        });
    }
}

impl Clone for ObjectGraph {
    fn clone(&self) -> Self {
        match &self.serialized {
            Some(s) => {
                Self {
                    graph: self.graph.clone(),
                    roots: self.roots.clone(),
                    serialized_buffer: bitcode::Buffer::with_capacity(s.len()),
                    serialized: self.serialized.clone(),
                    hash: 0,
                }
            },
            None => {
                Self {
                    graph: self.graph.clone(),
                    roots: self.roots.clone(),
                    serialized_buffer: Default::default(),
                    serialized: None,
                    hash: 0,
                }
            },
        }
    }
}

impl Debug for ObjectGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectGraph")
            .field("graph", &self.graph)
            .field("roots", &self.roots)
            .finish()
    }
}

impl Hash for ObjectGraph {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}

impl<'a> Eq for ObjectGraph {}

impl<'a> PartialEq for ObjectGraph {
    fn eq(&self, other: &ObjectGraph) -> bool {
        let self_buf = self.serialized.as_ref().unwrap();
        let other_buf = other.serialized.as_ref().unwrap();

        if self.hash != other.hash {
            return false;
        }
        if self.node_count() != other.node_count() {
            return false;
        }
        if self.edge_count() != other.edge_count() {
            return false;
        }
        for (root_a, root_b) in zip(self.roots.keys(), other.roots.keys()) {
            if root_a != root_b {
                return false;
            }
        }
        return self_buf == other_buf;
    }
}
