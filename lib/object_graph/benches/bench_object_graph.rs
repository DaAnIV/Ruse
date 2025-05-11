use criterion::*;
use rand::seq::IteratorRandom;

use rand::{rngs::StdRng, SeedableRng};
use ruse_object_graph::generator::object_graph_generator::*;
use ruse_object_graph::*;
use std::iter::zip;

const SEED: u64 = 100;
const RANGE: [usize; 8] = [5, 10, 20, 50, 100, 200, 500, 1000];

fn get_graphs_from_range(initial_id: GraphIndex) -> Vec<GraphsMap> {
    let mut graphs = Vec::with_capacity(RANGE.len());
    for (i, &n) in RANGE.iter().enumerate() {
        let mut rng = StdRng::seed_from_u64(SEED);
        graphs.push(random_gnp_object_graph(
            initial_id + i,
            &mut rng,
            n,
            1f64 / f64::sqrt(n as f64),
        ))
    }
    return graphs;
}

fn get_serialized_graphs_from_range(initial_id: GraphIndex) -> Vec<GraphsMap> {
    return get_graphs_from_range(initial_id);
}

// fn graph_serialize(c: &mut Criterion) {
//     let mut graphs = get_graphs_from_range(0);

//     let mut group = c.benchmark_group("serialize_graph");

//     for g in graphs.iter_mut() {
//         group.throughput(Throughput::Elements(g.node_count() as u64));
//         group.bench_function(format!("Serialize {}", g.node_count()), |b| {
//             b.iter(|| {
//                 g.generate_serialized_data();
//             })
//         });
//     }
//     group.finish();
// }

fn graph_clone(c: &mut Criterion) {
    let graphs = get_graphs_from_range(GraphIndex(0));

    let mut group = c.benchmark_group("clone_graph");

    for map in graphs.iter() {
        let g = map.graphs().next().unwrap();
        group.throughput(Throughput::Elements(g.node_count() as u64));
        group.bench_function(format!("Clone {}", g.node_count()), |b| {
            b.iter(|| {
                let _ = g.clone();
            })
        });
    }
    group.finish();
}

// fn graph_clone_and_serialize(c: &mut Criterion) {
//     let mut graphs = get_graphs_from_range();

//     let mut group = c.benchmark_group("clone_and_serialize_graph");

//     for g in graphs.iter_mut() {
//         group.throughput(Throughput::Elements(g.node_count() as u64));
//         group.bench_function(format!("Clone & Serialize {}", g.node_count()), |b| {
//             b.iter(|| {
//                 let mut g_copy = g.clone();
//                 g_copy.generate_serialized_data();
//             })
//         });
//     }
//     group.finish();
// }

fn graph_eq(c: &mut Criterion) {
    let mut graphs1 = get_serialized_graphs_from_range(GraphIndex(0));
    let mut graphs2 = graphs1.clone();

    let mut group = c.benchmark_group("graph_eq");

    for (g1, g2) in graphs1.iter_mut().zip(graphs2.iter_mut()) {
        group.throughput(Throughput::Elements(g1.node_count() as u64));
        group.bench_function(format!("Eq {}", g1.node_count()), |b| {
            b.iter(|| {
                assert_eq!(g1, g2, "Graphs are not equal");
            })
        });
    }
    group.finish();
}

fn graph_almost_eq(c: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(SEED);
    let map1 = random_gnp_object_graph(GraphIndex(0), &mut rng, 1000, 1f64 / f64::sqrt(1000f64));
    let mut map2 = map1.clone();
    map2.ensure_graph(GraphIndex(0));

    let mut edges = Vec::new();
    for (node_id, node) in map2[GraphIndex(0)].nodes() {
        for (field, _) in node.pointers_iter() {
            edges.push((*node_id, field.clone()));
        }
    }

    let remove_amount = 10;
    let remove = edges.into_iter().choose_multiple(&mut rng, remove_amount);
    for (id, field) in remove {
        map2.remove_edge(&field, GraphIndex(0), id);
    }

    let mut remaining = remove_amount;
    while remaining > 0 {
        let chosen = zip(map2[GraphIndex(0)].node_ids().copied(), map2[GraphIndex(0)].node_ids().copied())
            .choose_multiple(&mut rng, remaining);
        for (s, t) in chosen {
            if map2[GraphIndex(0)].contains_internal_edge(&s, &t) {
                continue;
            }
            map2.set_edge(
                field_name!(format!("{}_{}", s.index(), t.index())),
                GraphIndex(0),
                s,
                GraphIndex(0),
                t,
            );
            remaining -= 1;
        }
    }

    c.bench_function("graph_almost_eq", |b| {
        b.iter(|| {
            assert_ne!(map1, map2, "Graphs are equal");
        })
    });
}

fn graph_ne(c: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(SEED);

    let g1 = random_gnp_object_graph(GraphIndex(0), &mut rng, 1000, 1f64 / f64::sqrt(1000f64));
    let g2 = random_gnp_object_graph(GraphIndex(1), &mut rng, 1000, 1f64 / f64::sqrt(1000f64));

    c.bench_function("graph_ne", |b| {
        b.iter(|| {
            assert_ne!(g1, g2, "Graphs are equal");
        })
    });
}

criterion_group!(
    object_graph_benches,
    graph_clone,
    graph_eq,
    graph_almost_eq,
    graph_ne,
    // graph_serialize,
    // graph_clone_and_serialize
);
