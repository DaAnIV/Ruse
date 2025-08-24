#!/bin/bash

# Set Node.js path
export PATH="$HOME/nodejs/bin:$PATH"

# Start the server
node scripts/preprocess_logs.js $1