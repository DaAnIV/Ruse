#!/bin/bash

if [ "$#" -ne 1 ]; then
    echo "Illegal number of parameters"
    exit 1
fi

NAME=$1

BENCHMARKS=(
    -b ../tasks/benchmarks/new_ruse/full_oop/binary_search_tree_delete_two_children.sy
    -b ../tasks/benchmarks/new_ruse/full_oop/graph_cycle.sy
    -b ../tasks/benchmarks/new_ruse/full_oop/graph_one_way_connected.sy
    -b ../tasks/benchmarks/new_ruse/full_oop/graph.sy
    -b ../tasks/benchmarks/new_ruse/full_oop/user_names_aliasing.sy
    -b ../tasks/benchmarks/new_ruse/full_oop/user_names_connected.sy
    -b ../tasks/benchmarks/new_ruse/full_oop/user_names.sy
    -b ../tasks/benchmarks/new_ruse/full_oop/set_subtree.sy
    -b ../tasks/benchmarks/new_ruse/full_oop/user_names_simple.sy
    -b ../tasks/benchmarks/new_ruse/simple/x_y_mut_inc.sy
    -b ../tasks/benchmarks/new_ruse/simple/seq.sy
    -b ../tasks/benchmarks/fromSobeq/may/sobeq-new/FirstAndLast.sy
    -b ../tasks/benchmarks/fromSobeq/may/FrAngel/IsAllPositive.sy
    -b ../tasks/benchmarks/fromFrangel/other/abcd.sy
    -b ../tasks/benchmarks/fromFrangel/other/abc.sy
    -b ../tasks/benchmarks/fromFrangel/other/ab.sy
    -b ../tasks/benchmarks/fromSobeq/must/sobeq-new/MoveFromAToB.sy
    -b ../tasks/benchmarks/fromSobeq/may/probe/count-total-words-in-a-cellmodified.sy
    -b ../tasks/benchmarks/fromSobeq/may/sobeq-new/NegativeIndex.sy
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
