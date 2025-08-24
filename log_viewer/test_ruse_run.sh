#!/bin/bash

# Set Node.js path
export PATH="$HOME/nodejs/bin:$PATH"

echo "Testing Ruse run processing..."
echo "================================"

# Test with both log and result files
echo "1. Testing with log file and result file..."
./process_log.sh sample_logs.jsonl result.json

echo ""
echo "Test completed!"
