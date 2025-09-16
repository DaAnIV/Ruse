use criterion::*;

mod bench_misc;
mod bench_multithreaded;

criterion_main!(bench_misc::benches_misc);
