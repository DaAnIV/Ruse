#!/bin/bash

rm -rf results/ruse_all_tasks_results
rm -rf results/ruse_all_tasks_log.jsonl

../target/release/Ruse run \
    -o results/ruse_all_tasks_results \
    --log results/ruse_all_tasks_log.jsonl \
    -t 3600 \
    --workers-count 64 \
    --max-iterations 6 \
    --max-mutations 3 \
    --max-sequence-size 3 \
    --max-task-mem 100GiB \
    -b ../tasks/benchmarks/
