use crate::*;

use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::{self, EdgeRef};
use petgraph::visit::{VisitMap, Visitable};
use rand::Rng;
use std::collections::HashMap;
use petgraph_gen::{random_gnp_graph, random_gnm_graph};

fn get_unseen_index(n: u32, seen: &mut dyn VisitMap<u32>) -> Option<u32> {
    for i in 0..n {
        if seen.visit(i) {
            return Some(i);
        }
    }
    return None;
}

fn generate_random_str<R: Rng + ?Sized>(max_size: usize, rng: &mut R) -> String {
    let str_size = rng.gen_range(0..=max_size);
    let mut rstr = Vec::with_capacity(str_size);
    for _ in 0..max_size {
        rstr.push(rng.gen_range(('a' as u8)..=('z' as u8)));
    }
    String::from_utf8(rstr).expect("Bad random string")
}

fn graph_to_random_object_graph<R: Rng + ?Sized>(
    cache: &Cache,
    rng: &mut R,
    base: DiGraph<(), ()>
) -> ObjectGraph {
    let n = base.node_count();
    let m = base.edge_count();
    let mut discovered = base.visit_map();

    let mut map = HashMap::<NodeIndex, NodeIndex>::with_capacity(n);
    let mut graph = ObjectGraph::with_capacity(n, m);
    let root_field_string = str_cached!(cache; "r");
    let data_field_string = str_cached!(cache; "a");
    while let Some(r) = get_unseen_index(n.try_into().unwrap(), &mut discovered) {
        let root_fields = fields!((root_field_string.clone(), PrimitiveValue::Number(rng.next_u64().into())));
        let root_name = scached!(cache; generate_random_str(5, rng));
        let obj_name = scached!(cache; generate_random_str(5, rng));
        let idx = graph.add_root(root_name, ObjectData::new(obj_name, root_fields));
        map.insert(r.into(), idx);

        let mut bfs = visit::Bfs::new(&base, r.into());
        while let Some(nx) = bfs.next(&base) {
            if discovered.visit(nx) {
                let data_fields = fields!((data_field_string.clone(), PrimitiveValue::Number(rng.next_u64().into())));
                let obj_name = scached!(cache; generate_random_str(5, rng));
                let new_idx = graph.add_node(ObjectData::new(obj_name, data_fields));
                map.insert(nx, new_idx);
            }
        }
    }

    for edge in base.edge_references() {
        let object = map.get(&edge.source()).unwrap();
        let field = map.get(&edge.target()).unwrap();
        let edge_name = scached!(cache; format!("{}_{}", object.index(), field.index()));
        graph.add_edge(*object, *field, &edge_name);
    }

    return graph;
}

pub fn random_gnp_object_graph<R: Rng + ?Sized>(
    cache: &Cache,
    rng: &mut R,
    n: usize,
    p: f64,
) -> ObjectGraph {
    let base: DiGraph<(), ()> = random_gnp_graph(rng, n, p);
    graph_to_random_object_graph(cache, rng, base)
}

pub fn random_gnm_object_graph<R: Rng + ?Sized>(
    cache: &Cache,
    rng: &mut R,
    n: usize,
    m: usize,
) -> ObjectGraph {
    let base: DiGraph<(), ()> = random_gnm_graph(rng, n, m);
    graph_to_random_object_graph(cache, rng, base)
}
