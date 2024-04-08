use criterion::*;

mod bench_simple;

criterion_main!(
    bench_simple::benches_simple_synthesize
);
