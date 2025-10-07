#!/bin/bash

if [ "$#" -ne 1 ]; then
    echo "Illegal number of parameters"
    exit 1
fi

NAME=$1

BENCHMARKS=(
    -b ../tasks/benchmarks/new_ruse/overhead
)

rm -rf results/${NAME}_results
mkdir -p results/${NAME}_results

for i in {1..4}; do
    echo "Run #$i"
    ../target/release/Ruse run \
        -o results/${NAME}_results/run_${i} \
        --log results/${NAME}_results/${NAME}_log_${i}.jsonl \
        -t 7200 \
        --workers-count 96 \
        --max-iterations 6 \
        --max-mutations 3 \
        --max-sequence-size 3 \
        --max-task-mem 100GiB \
        "${BENCHMARKS[@]}" \
        --embedding-overhead-csv results/${NAME}_results/embedding_overhead_${i}.csv
done

python3 ../scripts/merge_results.py --delete results/${NAME}_results/run_*
