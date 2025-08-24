# Log Viewer

A web application for viewing and filtering JSON log files with support for special extensions and panic handling.

## Features

- Load and parse JSON lines log files
- Smart caching system for fast reloading
- Advanced filtering by level, target, timestamp, and content
- Support for special extensions (e.g., `.mermaid` for diagrams)
- Enhanced panic log handling with backtrace filtering
- Real-time log viewing with modern UI

## Quick Start

### Method 1: Using the setup script
```bash
./setup.sh
./start.sh
```

### Method 2: Manual setup
1. Install dependencies:
   ```bash
   PATH=$HOME/nodejs/bin:$PATH npm install
   ```

2. Start the server:
   ```bash
   PATH=$HOME/nodejs/bin:$PATH node server.js
   ```

3. Open your browser to `http://localhost:3000`

4. Upload or select log files to view

## Usage

### Preprocessing Logs
To preprocess logs for faster loading:
```bash
PATH=$HOME/nodejs/bin:$PATH node scripts/preprocess_logs.js /path/to/logfile.log
```

Or use the npm script:
```bash
PATH=$HOME/nodejs/bin:$PATH npm run preprocess -- /path/to/logfile.log
```

### Log Format
Expected JSON lines format:
```json
{"timestamp": "2023-01-01T00:00:00Z", "level": "INFO", "target": "app", "filename": "main.rs", "line_number": 42, "threadId": "main", "fields": {"message": "Hello world"}}
```

### Special Extensions
- Keys ending with `.mermaid` contain Mermaid diagrams
- Panic logs include `panic.backtrace` and `panic.location` fields

## File Structure

- `server.js` - Express server
- `scripts/preprocess_logs.js` - Log preprocessing script
- `public/` - Frontend web interface
- `cache/` - Cached processed logs
- `uploads/` - Uploaded log files
