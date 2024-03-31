use criterion::*;

use std::iter::zip;
use rand::{rngs::StdRng, seq::IteratorRandom, SeedableRng};
use ruse_object_graph::*;
use ruse_object_graph::generator::*;

const SEED: u64 = 100;

macro_rules! assert_err {
    ($expression:expr, $($pattern:tt)+) => {
        match $expression {
            $($pattern)+ => (),
            ref e => panic!("expected `{}` but got `{:?}`", stringify!($($pattern)+), e),
        }
    }
}

const RANGE: [usize; 8] = [5, 10, 20, 50, 100, 200, 500, 1000];

fn get_graphs_from_range(cache: &mut Cache) -> Vec<ObjectGraph> {
    let mut graphs = Vec::with_capacity(RANGE.len());
    for n in RANGE {
        let mut rng = StdRng::seed_from_u64(SEED);
        graphs.push(random_gnp_object_graph(cache, &mut rng, n as usize, 1f64 / f64::sqrt(n as f64)))
    }
    return graphs;
}

fn get_serialized_graphs_from_range(cache: &mut Cache) -> Vec<ObjectGraph> {
    let mut graphs = get_graphs_from_range(cache);
    for g in &mut graphs {
        g.generate_serialized_data().expect("Failed to serialize graph");
    }
    return graphs;
}

fn graph_serialize(c: &mut Criterion) {
    let mut cache = Cache::new();
    let mut graphs = get_graphs_from_range(&mut cache);

    let mut group = c.benchmark_group("serialize_graph");

    for g in graphs.iter_mut() {
        group.throughput(Throughput::Elements(g.node_count() as u64));
        group.bench_function(format!("Serialize {}", g.node_count()),  |b| {
            b.iter(|| {
                assert_err!(g.generate_serialized_data(), Ok(()));
            })
        });
    }
    group.finish();
}

fn graph_clone(c: &mut Criterion) {
    let mut cache = Cache::new();
    let mut graphs = get_graphs_from_range(&mut cache);

    let mut group = c.benchmark_group("clone_graph");

    for g in graphs.iter_mut() {
        group.throughput(Throughput::Elements(g.node_count() as u64));
        group.bench_function(format!("Clone {}", g.node_count()),  |b| {
            b.iter(|| {
                let _ = g.clone();
            })
        });
    }
    group.finish();
}

fn graph_clone_and_serialize(c: &mut Criterion) {
    let mut cache = Cache::new();
    let mut graphs = get_graphs_from_range(&mut cache);

    let mut group = c.benchmark_group("clone_and_serialize_graph");

    for g in graphs.iter_mut() {
        group.throughput(Throughput::Elements(g.node_count() as u64));
        group.bench_function(format!("Clone & Serialize {}", g.node_count()),  |b| {
            b.iter(|| {
                let mut g_copy = g.clone();
                g_copy.generate_serialized_data().expect("Failed to serialize graph");
            })
        });
    }
    group.finish();
}

fn graph_eq(c: &mut Criterion) {
    let mut cache = Cache::new();
    let mut graphs1 = get_serialized_graphs_from_range(&mut cache);
    let mut graphs2 = graphs1.clone();

    let mut group = c.benchmark_group("graph_eq");

    for (g1, g2) in graphs1.iter_mut().zip(graphs2.iter_mut()) {
        group.throughput(Throughput::Elements(g1.node_count() as u64));
        group.bench_function(format!("Eq {}", g1.node_count()),  |b| {
            b.iter(|| {
                assert_eq!(g1, g2, "Graphs are not equal");
            })
        });
    }
    group.finish();
}

fn graph_almost_eq(c: &mut Criterion) {
    let mut cache = Cache::new();
    let mut rng = StdRng::seed_from_u64(SEED);
    let mut g1 = random_gnp_object_graph(&mut cache, &mut rng, 1000, 1f64 / f64::sqrt(1000f64));
    let mut g2 = g1.clone();

    let remove = g2.graph.edge_indices().choose_multiple(&mut rng, 10);

    for ei in remove {
        g2.remove_edge(ei);
    }
    while g2.edge_count() != g1.edge_count() {
        let add = zip(g2.graph.node_indices(), g2.graph.node_indices())
            .choose_multiple(&mut rng, g1.edge_count() - g2.edge_count());
        for (s, t) in add {
            if g2.graph.contains_edge(s, t) {
                continue;
            }
            g2.add_edge(s, t, scached!(cache; format!("{}_{}", s.index(), t.index())));
        }
    }

    assert_eq!(g1.edge_count(), g2.edge_count(), "Graphs edges count is different");

    g1.generate_serialized_data().expect("Failed to serialize g1");
    g2.generate_serialized_data().expect("Failed to serialize g2");
    c.bench_function("graph_almost_eq", |b| b.iter(|| {
        assert_ne!(g1, g2, "Graphs are not equal");
    }));
}

fn graph_ne(c: &mut Criterion) {
    let mut cache = Cache::new();
    let mut rng = StdRng::seed_from_u64(SEED);

    let mut g1 = random_gnp_object_graph(&mut cache, &mut rng, 1000, 1f64 / f64::sqrt(1000f64));
    let mut g2 = random_gnp_object_graph(&mut cache, &mut rng, 1000, 1f64 / f64::sqrt(1000f64));

    g1.generate_serialized_data().expect("Failed to serialize g1");
    g2.generate_serialized_data().expect("Failed to serialize g2");
    c.bench_function("graph_ne", |b| b.iter(|| {
        assert_ne!(g1, g2, "Graphs are not equal");
    }));
}

criterion_group!(
    object_graph_benches,
    graph_serialize,
    graph_clone,
    graph_eq,
    graph_almost_eq,
    graph_ne,
    graph_clone_and_serialize
);
