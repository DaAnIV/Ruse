#!/bin/sh

./target/profiling/ruse_benchmarks run \
    -b ./benchmarks/tasks/simple/ \
    -b ./benchmarks/tasks/binary_search_tree/ \
    -b ./benchmarks/tasks/fromDictEnum/benchmarks \
    --timeout 300 \
    --max-iterations 8 \
    --max-context-depth 3 \
    --workers-count 128 \
    --output results/result.json \
    --log results/log.json \
    --pretty
