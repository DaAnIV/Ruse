#!/bin/bash

if [ "$#" -ne 1 ]; then
    echo "Illegal number of parameters"
fi

NAME=$1

rm -rf results/${NAME}_results
rm -rf results/${NAME}_log.jsonl

mkdir -p results/${NAME}_results

BENCHMARKS=(
    -b ../tasks/benchmarks/new_ruse/relations/
)

../target/release/Ruse run \
    -o results/${NAME}_results/run \
    --log results/${NAME}_results/${NAME}_log.jsonl \
    -t 3600 \
    --workers-count 1 \
    --max-iterations 6 \
    --max-mutations 3 \
    --max-sequence-size 3 \
    --max-task-mem 100GiB \
    "${BENCHMARKS[@]}"

python3 ../scripts/merge_results.py --delete results/${NAME}_results/run

