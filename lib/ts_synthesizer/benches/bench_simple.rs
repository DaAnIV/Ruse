use criterion::*;
use ruse_object_graph as object_graph;
use ruse_object_graph::*;
use ruse_synthesizer::{context::Context, vnum};
use ruse_ts_synthesizer::*;
use std::sync::Arc;

fn simple_synthesize_1(c: &mut Criterion) {
    let mut group = c.benchmark_group("simple_synthesize_1");

    for i in 2..=10 {
        group.throughput(Throughput::Elements(i as u64));
        group.bench_function(format!("Synthesize {:0>4}", i), |b| {
            b.iter_batched(
                || {                    
                    let cache = object_graph::Cache::new();
                    let ctx = Arc::new([
                        Context::with_values(
                            [
                                (str_cached!(cache; "x"), vnum!(Number::from(4u64))),
                                (str_cached!(cache; "y"), vnum!(Number::from(2u64))),
                            ]
                            .into(),
                        ),
                        Context::with_values(
                            [
                                (str_cached!(cache; "x"), vnum!(Number::from(5u64))),
                                (str_cached!(cache; "y"), vnum!(Number::from(3u64))),
                            ]
                            .into(),
                        ),
                    ]);
                    let opcodes = construct_opcode_list(
                        &[str_cached!(cache; "x"), str_cached!(cache; "y")],
                        &[-1f64, 1f64],
                        &ALL_BIN_NUM_OPCODES,
                        &ALL_UNARY_NUM_OPCODES,
                        &ALL_UPDATE_NUM_OPCODES,
                        false,
                        &[],
                        &[],
                        &[],
                    );

                    let synthesizer = TsSynthesizer::with_context_and_opcodes(
                        ctx.clone(),
                        opcodes.clone(),
                        &cache,
                    );
                    (synthesizer, ctx, cache)
                },
                |(mut synthesizer, ctx, cache)| {
                    for i in 2..=i {
                        synthesizer.synthesize_for_size(&ctx, i, &cache);
                    }
                },
                BatchSize::PerIteration
            )
        });
    }
    group.finish()
}

criterion_group!(benches_simple_synthesize, simple_synthesize_1);
