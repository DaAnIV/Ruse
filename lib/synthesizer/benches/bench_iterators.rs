use criterion::*;
use itertools::Itertools;
// use criterion::async_executor::FuturesExecutor;
use ruse_synthesizer::iterator::IterationsIterator;
use ruse_object_graph::GraphIdGenerator;
use ruse_synthesizer::context::{ContextArray, VariableMap};
use ruse_synthesizer::iterator_test_helpers::{add_iteration, TestBank};
use ruse_synthesizer::synthesizer_context::SynthesizerContext;

// async fn create_bank(num_iterations: usize) -> TestBank {
//     let _id_gen = GraphIdGenerator::default();
//     let syn_ctx =
//         SynthesizerContext::from_context_array(ContextArray::default(), VariableMap::default());
//     let mut bank = TestBank::default();
//     for i in 0..num_iterations {
//         add_iteration(&mut bank, 3 + i, &syn_ctx).await;
//     }
//     bank
// }

fn iterations_iterator_with_filter(
    iterations_count: usize,
    arg_count: usize,
) -> impl Iterator<Item = Vec<usize>> {
    let last_iteration = iterations_count - 1;
    (0..arg_count)
        .map(move |_| 0..=last_iteration)
        .multi_cartesian_product()
        .filter(move |iterations| iterations.iter().any(|&i| i == last_iteration))
}

fn bench_iterations_iterator(c: &mut Criterion) {
    for iterations_count in 2..6 {
        let mut group = c.benchmark_group(&format!("iterations_iterator/{}", iterations_count));
        for arg_count in 1..6 {
            assert_eq!(
                IterationsIterator::new(iterations_count, arg_count).count(),
                iterations_iterator_with_filter(iterations_count, arg_count).count()
            );

            group.bench_with_input(
                BenchmarkId::new("WithCutoff", arg_count),
                &arg_count,
                |b, &arg_count| {
                    b.iter(|| {
                        let iter = IterationsIterator::new(iterations_count, arg_count);
                        iter.count();
                    })
                },
            );
            group.bench_with_input(
                BenchmarkId::new("WithFilter", arg_count),
                &arg_count,
                |b, &arg_count| {
                    b.iter(|| {
                        let iter = iterations_iterator_with_filter(iterations_count, arg_count);
                        iter.count();
                    })
                },
            );
        }
        group.finish();
    }
}

criterion_group!(benches_iterators, bench_iterations_iterator);
