#!/bin/bash

# MongoDB Setup Script for Log Viewer
# This script sets up MongoDB for faster log querying

echo "🍃 Setting up MongoDB for Log Viewer..."
echo "======================================="

# Check if MongoDB is extracted
if [ ! -d "/tmp/monogdb/mongodb-linux-x86_64-debian11-7.0.23" ]; then
    echo "❌ MongoDB not found in /tmp/monogdb/"
    echo "Please extract MongoDB first:"
    echo "cd /tmp/monogdb && tar -zxvf mongodb-linux-x86_64-debian11-7.0.23.tgz"
    exit 1
fi

# Create MongoDB directories
echo "📁 Creating MongoDB directories..."
mkdir -p $HOME/mongodb/data
mkdir -p $HOME/mongodb/logs
mkdir -p $HOME/mongodb/bin

# Copy MongoDB binaries
echo "📦 Installing MongoDB binaries..."
cp /tmp/monogdb/mongodb-linux-x86_64-debian11-7.0.23/bin/* $HOME/mongodb/bin/

# Create MongoDB configuration file
echo "⚙️  Creating MongoDB configuration..."
cat > $HOME/mongodb/mongod.conf << EOF
# MongoDB Configuration File
storage:
  dbPath: $HOME/mongodb/data
  journal:
    enabled: true

systemLog:
  destination: file
  logAppend: true
  path: $HOME/mongodb/logs/mongod.log

net:
  port: 27017
  bindIp: 127.0.0.1

processManagement:
  fork: true
  pidFilePath: $HOME/mongodb/mongod.pid
EOF

# Create start script
echo "🚀 Creating MongoDB start script..."
cat > $HOME/mongodb/start_mongodb.sh << 'EOF'
#!/bin/bash
echo "🍃 Starting MongoDB..."
$HOME/mongodb/bin/mongod --config $HOME/mongodb/mongod.conf
echo "✅ MongoDB started on port 27017"
echo "Data directory: $HOME/mongodb/data"
echo "Logs: $HOME/mongodb/logs/mongod.log"
EOF

chmod +x $HOME/mongodb/start_mongodb.sh

# Create stop script
echo "🛑 Creating MongoDB stop script..."
cat > $HOME/mongodb/stop_mongodb.sh << 'EOF'
#!/bin/bash
echo "🛑 Stopping MongoDB..."
if [ -f $HOME/mongodb/mongod.pid ]; then
    kill $(cat $HOME/mongodb/mongod.pid)
    rm -f $HOME/mongodb/mongod.pid
    echo "✅ MongoDB stopped"
else
    echo "⚠️  MongoDB PID file not found"
    # Try to find and kill mongod process
    pkill -f mongod && echo "✅ MongoDB process killed" || echo "❌ No MongoDB process found"
fi
EOF

chmod +x $HOME/mongodb/stop_mongodb.sh

# Install Node.js dependencies
echo "📦 Installing MongoDB Node.js driver..."
cd $(dirname $0)
PATH=$HOME/nodejs/bin:$PATH npm install

echo ""
echo "✅ MongoDB setup complete!"
echo ""
echo "📋 Next steps:"
echo "1. Start MongoDB:"
echo "   $HOME/mongodb/start_mongodb.sh"
echo ""
echo "2. Test the log viewer with MongoDB:"
echo "   ./process_log.sh ../logs/log.jsonl"
echo ""
echo "3. Start the log viewer:"
echo "   ./start.sh"
echo ""
echo "4. Stop MongoDB when done:"
echo "   $HOME/mongodb/stop_mongodb.sh"
echo ""
echo "🔧 MongoDB will store data in: $HOME/mongodb/data"
echo "📄 MongoDB logs: $HOME/mongodb/logs/mongod.log"
echo "🌐 MongoDB connection: mongodb://localhost:27017"
