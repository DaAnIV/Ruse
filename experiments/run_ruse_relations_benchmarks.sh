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


for i in {1..4}; do
    echo "Run #$i"
    ../target/release/Ruse run \
        -o results/${NAME}_results/run_${i} \
        --log results/${NAME}_results/${NAME}_log_${i}.jsonl \
        -t 3600 \
        --workers-count 96 \
        --max-iterations 6 \
        --max-mutations 3 \
        --max-sequence-size 3 \
        --max-task-mem 100GiB \
        "${BENCHMARKS[@]}"
done

for i in {1..4}; do
    python3 ../scripts/merge_results.py results/${NAME}_results/run_${i}
done
