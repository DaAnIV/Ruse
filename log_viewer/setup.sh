#!/bin/bash

echo "🔍 Ruse Viewer Setup Script"
echo "=========================="

# Set Node.js path
export PATH="$HOME/nodejs/bin:$PATH"

# Check if Node.js is installed
if ! command -v node &> /dev/null; then
    echo "❌ Node.js is not installed at ~/nodejs/bin/"
    echo "Please ensure Node.js is installed in ~/nodejs/"
    echo "Current PATH: $PATH"
    exit 1
fi

# Check if npm is installed
if ! command -v npm &> /dev/null; then
    echo "❌ npm is not installed at ~/nodejs/bin/"
    echo "Please ensure npm is installed in ~/nodejs/"
    exit 1
fi

echo "✅ Node.js version: $(node --version)"
echo "✅ npm version: $(npm --version)"

# Install dependencies
echo "📦 Installing dependencies..."
npm install

if [ $? -eq 0 ]; then
    echo "✅ Dependencies installed successfully!"
else
    echo "❌ Failed to install dependencies"
    exit 1
fi

# Create directories if they don't exist
mkdir -p uploads

# Test preprocessor with sample Ruse run
echo "🧪 Testing preprocessor with sample Ruse run..."
node scripts/preprocess_logs.js sample_logs.jsonl result.json

if [ $? -eq 0 ]; then
    echo "✅ Preprocessor test successful!"
else
    echo "❌ Preprocessor test failed"
    exit 1
fi

echo ""
echo "🎉 Setup complete!"
echo ""
echo "To start the application:"
echo "  npm start"
echo ""
echo "Then open your browser to: http://localhost:3000"
echo ""
echo "To preprocess additional Ruse runs:"
echo "  npm run preprocess -- /path/to/your/logfile.log /path/to/your/result.json"
