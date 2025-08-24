#!/bin/bash

# Set Node.js path
export PATH="$HOME/nodejs/bin:$PATH"

echo "🔍 Starting Log Viewer..."
echo "========================"
echo "Server will be available at: http://localhost:3000"
echo "Press Ctrl+C to stop the server"
echo ""

# Start the server
node server.js