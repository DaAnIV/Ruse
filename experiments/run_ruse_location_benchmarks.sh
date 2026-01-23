#!/bin/bash

NAME="ruse_experiments"

while [[ $# -gt 0 ]]; do
    key="$1"
    case $key in
        --name)
            NAME="$2"
            shift
            shift
            ;;
    esac
done

cd ..
cargo build --profile=release --features release_trace_max_level_info --features check_location_equality_via_graphs
cd -
bash ./run_ruse_regular_benchmarks.sh ${NAME}_full_location_checking

cd ..
cargo build --profile=release --features release_trace_max_level_info --features simple_location_equality_check
cd - 
bash ./run_ruse_regular_benchmarks.sh ${NAME}_simple_location_checking

cd ..
cargo build --profile=release --features release_trace_max_level_info
cd -
bash ./run_ruse_regular_benchmarks.sh ${NAME}_no_location_checking
