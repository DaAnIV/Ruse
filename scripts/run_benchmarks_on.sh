#!/bin/bash

rm -rf results/results

RUST_BACKTRACE=1 ./target/profiling/Ruse run \
    --timeout 300 \
    --max-iterations 5 \
    --max-mutations 3 \
    --max-sequence-size 3 \
    --workers-count 64 \
    --output results/results \
    --log results/log.jsonl \
    -vvvv \
    --pretty \
    ${@/#/-b }

# cd log_viewer
# ./process_run.sh ../results/log.jsonl ../results/result.json 
# cd ..
