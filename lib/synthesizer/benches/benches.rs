use criterion::*;

mod bench_misc;
mod bench_multithreaded;
mod bench_iterators;

criterion_main!(bench_misc::benches_misc, bench_iterators::benches_iterators);
