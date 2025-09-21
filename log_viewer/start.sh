#!/bin/bash

echo "🔍 Starting Ruse Viewer..."
echo "========================"
echo "Server will be available at: http://localhost:3000"
echo "Press Ctrl+C to stop the server"
echo ""

# Function to kill all background jobs
kill_background_jobs() {
    echo "Killing background jobs..."
    kill $(jobs -p) 2>/dev/null # Kill all running background jobs
}

# Trap SIGINT (Ctrl+C) and call the function
trap kill_background_jobs INT

$HOME/mongodb/bin/mongod --dbpath $HOME/mongodb/data --port 27017 --bind_ip 127.0.0.1 --logpath $HOME/mongodb/logs/mongod.log --logappend&

sleep 1

# Start the server
node server.js&

wait