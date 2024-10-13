// #[cfg(bench)]
// mod benchmarks {
    use criterion::*;

    mod bench_object_graph;
    
    criterion_main!(bench_object_graph::object_graph_benches);
// }
