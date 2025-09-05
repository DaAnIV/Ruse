#!/bin/bash


RUST_BACKTRACE=1 ./target/profiling/ruse_benchmarks run \
    --timeout 300 \
    --max-iterations 5 \
    --max-context-depth 3 \
    --max-sequence-size 10 \
    --workers-count 64 \
    --output results/result.json \
    --log results/log.jsonl \
    --pretty \
    -vvvv \
    ${@/#/-b }

# cd log_viewer
# ./process_run.sh ../results/log.jsonl ../results/result.json 
# cd ..
