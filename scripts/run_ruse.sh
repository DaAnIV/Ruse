#!/bin/bash

SCRIPT_DIR=$(dirname "$0")

LOG_FILE=""
OUTPUT_DIR=""
TARGET=""
BACKTRACE=false
UPLOAD_RUN=false

while [[ $# -gt 0 ]]; do
    key="$1"
    case $key in
        -l|--log)
            LOG_FILE="$2"
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
        --upload-run)
            UPLOAD_RUN=true
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
LOG_FILE="${LOG_FILE:-results/log.jsonl}"
OUTPUT_DIR="${OUTPUT_DIR:-results/results}"
TARGET="${TARGET:-release}"

if [ "$BACKTRACE" = true ]; then
    export RUST_BACKTRACE=1
fi

rm -rf ${OUTPUT_DIR}

./target/${TARGET}/Ruse run \
    --output ${OUTPUT_DIR} \
    --log ${LOG_FILE} \
    ${OTHER_ARGS[@]}

OUTPUT_DIR_ABS_PATH=$(realpath ${OUTPUT_DIR})
LOG_FILE_ABS_PATH=$(realpath ${LOG_FILE})

# Function to kill all background jobs
kill_background_jobs() {
    echo "Killing background jobs..."
    kill $(jobs -p) 2>/dev/null # Kill all running background jobs
}

# Trap SIGINT (Ctrl+C) and call the function
trap kill_background_jobs INT

if [ "$UPLOAD_RUN" = true ]; then
    cd ${SCRIPT_DIR}/../log_viewer
    ./process_run.sh ${LOG_FILE_ABS_PATH} ${OUTPUT_DIR_ABS_PATH}
    cd -

    # Stop MongoDB
    kill_background_jobs
else
    echo "./process_run.sh ${LOG_FILE_ABS_PATH} ${OUTPUT_DIR_ABS_PATH}"
fi

# cd log_viewer
# ./process_run.sh ../results/log.jsonl ../results/result.json 
# cd ..
