#!/bin/bash

# Set Node.js path
export PATH="$HOME/nodejs/bin:$PATH"

# Check if we have both log and result files
if [ $# -eq 2 ]; then
    echo "Processing Ruse run with log file: $1 and result file: $2"
    node scripts/preprocess_logs.js "$1" "$2"
else
    echo "Usage: $0 <log_file_path> <result_file_path>"
    echo "  Both log file and result file are required for a complete Ruse run"
    exit 1
fi