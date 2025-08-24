# Ruse Viewer

A web application for viewing and analyzing Ruse runs, including log files and result data with support for special extensions and panic handling.

## Features

- Load and analyze complete Ruse runs (log files + result data)
- View individual log files or complete run results
- Smart caching system for fast reloading
- Advanced filtering by level, target, timestamp, and content
- Support for special extensions (e.g., `.mermaid` for diagrams)
- Enhanced panic log handling with backtrace filtering
- Real-time log viewing with modern UI
- Comprehensive run statistics and task analysis

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

4. Upload or select complete Ruse runs to view

## Usage

### Preprocessing Ruse Runs
To preprocess a complete Ruse run (log file + result file required):
```bash
PATH=$HOME/nodejs/bin:$PATH node scripts/preprocess_logs.js /path/to/logfile.log /path/to/result.json
```

**Note**: Both log file and result file are required. The system only processes complete Ruse runs.

Or use the npm script:
```bash
PATH=$HOME/nodejs/bin:$PATH npm run preprocess -- /path/to/logfile.log /path/to/result.json
```

### Log Format
Expected JSON lines format for log files:
```json
{"timestamp": "2023-01-01T00:00:00Z", "level": "INFO", "target": "app", "filename": "main.rs", "line_number": 42, "threadId": "main", "fields": {"message": "Hello world"}}
```

### Result Format
Expected JSON format for result files:
```json
{
  "timestamp": 1756049822,
  "sysinfo": {"name": "Debian GNU/Linux", "kernel": "5.10.0-32-amd64"},
  "tasks": [{"path": "task.sy", "found": "result", "error": null}]
}
```

### Special Extensions
- Keys ending with `.mermaid` contain Mermaid diagrams
- Panic logs include `panic.backtrace` and `panic.location` fields

## File Structure

- `server.js` - Express server
- `scripts/preprocess_logs.js` - Ruse run preprocessing script
- `public/` - Frontend web interface
- `cache/` - Cached processed runs
- `uploads/` - Uploaded files
