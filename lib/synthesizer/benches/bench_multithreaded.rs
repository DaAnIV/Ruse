use criterion::*;

// use dashmap;
use rand::{rngs::StdRng, SeedableRng};
// use rayon::{self, iter::{IntoParallelRefIterator, ParallelIterator}};
use ruse_object_graph::{generator::*, GraphsMap, graph_map_value::{GraphMapWrap, GraphMapValue}};
use ruse_object_graph::{Cache, ObjectGraph};
use std::{collections::HashSet, sync::Arc};

const SEED: u64 = 100;

// const RANGE: [usize; 8] = [10, 20, 50, 100, 200, 500, 1000, 2000, 5000, 10000];

// fn get_graphs_from_range(cache: &Cache) -> Vec<ObjectGraph> {
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

// fn get_serialized_graphs_from_range(cache: &Cache) -> Vec<ObjectGraph> {
//     let mut graphs = get_graphs_from_range(cache);
//     for g in &mut graphs {
//         g.generate_serialized_data();
//     }
//     return graphs;
// }

fn hash_insertion(c: &mut Criterion) {
    let cache = Cache::new();
    let mut rng = StdRng::seed_from_u64(SEED);
    let mut map = GraphsMap::default();

    let graphs: Vec<Arc<ObjectGraph>> = (0..10000usize)
        .map(|i| {
            let g = Arc::new(object_graph_generator::random_gnm_object_graph(&cache, i, &mut rng, 20, 40));
            map.insert_graph(g.clone());
            g
        })
        .collect();

    let mut group = c.benchmark_group("hash_insertion_graph");

    group.bench_function("iterative std::HashSet", |b| {
        b.iter_batched(
            || HashSet::<GraphMapValue<'_, ObjectGraph>>::new(),
            |mut std_hashset| {
                graphs.iter().for_each(|g| {
                    // sleep(time::Duration::from_nanos(1));
                    std_hashset.insert(g.wrap(&map));
                });
                std_hashset
            },
            BatchSize::LargeInput,
        )
    });

    // group.bench_function("parallel dashmap", |b| {
    //     b.iter_batched(|| dashmap::DashSet::<Arc<ObjectGraph>>::new(), |dash_map| {
    //         graphs.par_iter().for_each(|g| {
    //             // sleep(time::Duration::from_nanos(1));
    //             dash_map.insert(g.clone());
    //         });
    //         dash_map
    //     }, BatchSize::LargeInput)
    // });

    group.finish();
}

criterion_group!(multithreading_benches, hash_insertion);
