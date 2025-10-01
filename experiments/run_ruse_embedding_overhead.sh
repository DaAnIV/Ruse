#!/bin/bash

NAME=${1:-ruse_embedding_overhead};

BENCHMARKS=(
    -b ../tasks/benchmarks/new_ruse/binary_search_tree/binary_search_tree_delete_two_children.sy
    -b ../tasks/benchmarks/new_ruse/relations/
    -b ../tasks/benchmarks/new_ruse/simple/x_y_mut_inc.sy
    -b ../tasks/benchmarks/new_ruse/simple/seq.sy
    -b ../tasks/benchmarks/fromSobeq/may/sobeq-new/FirstAndLast.sy
    -b ../tasks/benchmarks/fromSobeq/may/FrAngel/IsAllPositive.sy
)

rm -rf results/${NAME}_results
rm -rf results/${NAME}_log.jsonl

echo "results/${NAME}_log.jsonl"

../target/release/Ruse run \
    -o results/${NAME}_results \
    --log results/${NAME}_log.jsonl \
    -t 36000 \
    --workers-count 96 \
    --max-iterations 6 \
    --max-mutations 3 \
    --max-sequence-size 3 \
    --max-task-mem 100GiB \
    "${BENCHMARKS[@]}" \
    --embedding-overhead-csv results/${NAME}_embedding_overhead.csv

