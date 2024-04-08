use criterion::*;

use dashmap;
use rand::{rngs::StdRng, SeedableRng};
use rayon::{self, iter::{IntoParallelRefIterator, ParallelIterator}};
use ruse_object_graph::{ObjectGraph, Cache};
use ruse_object_graph::generator::*;
use std::{collections::HashSet, sync::Arc};

const SEED: u64 = 100;

// const RANGE: [usize; 8] = [10, 20, 50, 100, 200, 500, 1000, 2000, 5000, 10000];

// fn get_graphs_from_range(cache: &mut Cache) -> Vec<ObjectGraph> {
//     let mut graphs = Vec::with_capacity(RANGE.len());
//     for n in RANGE {
//         let mut rng = StdRng::seed_from_u64(SEED);
//         graphs.push(random_gnp_object_graph(
//             cache,
//             &mut rng,
//             n as usize,
//             1f64 / f64::sqrt(n as f64),
//         ))
//     }
//     return graphs;
// }

// fn get_serialized_graphs_from_range(cache: &mut Cache) -> Vec<ObjectGraph> {
//     let mut graphs = get_graphs_from_range(cache);
//     for g in &mut graphs {
//         g.generate_serialized_data()
//             .expect("Failed to serialize graph");
//     }
//     return graphs;
// }

fn hash_insertion(c: &mut Criterion) {
    let mut cache = Cache::new();
    let mut rng = StdRng::seed_from_u64(SEED);

    let graphs: Vec<Arc<ObjectGraph>> = (0..10000).map(|_| {
        let mut g = random_gnm_object_graph(&mut cache, &mut rng, 20, 40);
        g.generate_serialized_data().expect("Failed to serialize data");
        Arc::new(g)
    }).collect();

    let mut group = c.benchmark_group("hash_insertion_graph");

    group.bench_function("iterative std::HashSet", |b| {
        b.iter_batched(|| HashSet::<Arc<ObjectGraph>>::new(), |mut std_hashset| {
            graphs.iter().for_each(|g| {
                // sleep(time::Duration::from_nanos(1));
                std_hashset.insert(g.clone());
            });
            std_hashset
        }, BatchSize::LargeInput)
    });

    group.bench_function("parallel dashmap", |b| {
        b.iter_batched(|| dashmap::DashSet::<Arc<ObjectGraph>>::new(), |dash_map| {
            graphs.par_iter().for_each(|g| {
                // sleep(time::Duration::from_nanos(1));
                dash_map.insert(g.clone());
            });
            dash_map
        }, BatchSize::LargeInput)
    });

    group.finish();
}

criterion_group!(multithreading_benches, hash_insertion);
