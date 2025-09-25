#!/bin/bash

# Function to kill all background jobs
kill_background_jobs() {
    echo "Killing background jobs..."
    kill $(jobs -p) 2>/dev/null # Kill all running background jobs
}

# Trap SIGINT (Ctrl+C) and call the function
trap kill_background_jobs INT

MONGO_PID=$(pidof mongod)

# Check if we have both log and result files
if [ $# -eq 2 ]; then
    if [ -z "$MONGO_PID" ]; then
    # Start MongoDB
        $HOME/mongodb/bin/mongod --dbpath $HOME/mongodb/data --port 27017 --bind_ip 127.0.0.1 --logpath $HOME/mongodb/logs/mongod.log --logappend&

        sleep 1
    fi

    echo "Processing Ruse run with log file: $1 and result file: $2"
    node scripts/preprocess_logs.js "$1" "$2"

    if [ -z "$MONGO_PID" ]; then
        # Stop MongoDB
        kill_background_jobs
    fi
else
    echo "Usage: $0 <log_file_path> <result_file_path>"
    echo "  Both log file and result file are required for a complete Ruse run"
    exit 1
fi