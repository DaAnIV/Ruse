#!/bin/bash

# Parse arguments: -l/--log, -o/--output, -t/--target
LOG_FILE=""
OUTPUT_DIR=""
TARGET=""
BACKTRACE=false

while [[ $# -gt 0 ]]; do
    key="$1"
    case $key in
        -l|--log)
            LOG_FILE="--log $2"
            shift
            shift
            ;;
        -o|--output)
            OUTPUT_DIR="$2"
            shift
            shift
            ;;
        --target)
            TARGET="$2"
            shift
            shift
            ;;
        --backtrace)
            BACKTRACE=true
            shift
            ;;
        *)
            OTHER_ARGS+=("$1")
            shift
            ;;
    esac
done

# Set defaults if not provided
# LOG_FILE="${LOG_FILE:-results/log.jsonl}"
OUTPUT_DIR="${OUTPUT_DIR:-results/results}"
TARGET="${TARGET:-release}"

rm -rf ${OUTPUT_DIR}

if [ "$BACKTRACE" = true ]; then
    export RUST_BACKTRACE=1
fi

./target/${TARGET}/Ruse run \
    --output ${OUTPUT_DIR} \
    ${LOG_FILE} \
    ${OTHER_ARGS[@]}

# cd log_viewer
# ./process_run.sh ../results/log.jsonl ../results/result.json 
# cd ..
