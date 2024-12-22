use criterion::*;
use ruse_object_graph as object_graph;
use ruse_object_graph::*;
use ruse_synthesizer::context::{Context, ContextArray, GraphIdGenerator};
use ruse_ts_synthesizer::*;
use std::sync::Arc;

fn simple_synthesize_1(c: &mut Criterion) {
    let mut group = c.benchmark_group("simple_synthesize_1");
    let mut rt_builder = tokio::runtime::Builder::new_multi_thread();
    let rt = rt_builder.build().expect("test");
    for i in 0..4 {
        group.bench_function(format!("Synthesize {:0>4}", i), |b| {
            b.to_async(&rt).iter_batched(
                || {
                    let cache = Arc::new(object_graph::Cache::new());
                    let id_gen1 = Arc::new(GraphIdGenerator::default());
                    let id_gen2 = Arc::new(GraphIdGenerator::default());
                    let graphs_map1 = GraphsMap::default();
                    let graphs_map2 = GraphsMap::default();

                    let ctx = ContextArray::from(vec![
                        Context::with_values(
                            [
                                (str_cached!(cache; "x"), vnum!(Number::from(4u64))),
                                (str_cached!(cache; "y"), vnum!(Number::from(2u64))),
                            ]
                            .into(),
                            graphs_map1.into(),
                            id_gen1,
                        ),
                        Context::with_values(
                            [
                                (str_cached!(cache; "x"), vnum!(Number::from(5u64))),
                                (str_cached!(cache; "y"), vnum!(Number::from(3u64))),
                            ]
                            .into(),
                            graphs_map2.into(),
                            id_gen2,
                        ),
                    ]);
                    let mut opcodes = construct_opcode_list(
                        &[str_cached!(cache; "x"), str_cached!(cache; "y")],
                        &[-1, 1],
                        &[],
                        false,
                    );

                    add_num_opcodes(
                        &mut opcodes,
                        &ALL_BIN_NUM_OPCODES,
                        &[],
                        &ALL_UPDATE_NUM_OPCODES,
                    );

                    TsSynthesizer::new(
                        ctx,
                        opcodes.clone(),
                        Box::new(|_| false),
                        Box::new(|_| true),
                        2,
                        1,
                        1,
                        cache,
                    )
                },
                |mut synthesizer| async move {
                    for _j in 0..=i {
                        synthesizer.run_iteration().await;
                    }
                },
                BatchSize::PerIteration,
            )
        });
    }
    group.finish()
}

criterion_group!(benches_simple_synthesize, simple_synthesize_1);
