# Log Viewer

A web-based log viewer application that allows you to view and filter application logs in real-time.

## Features

- View logs from multiple log files
- Filter logs by severity level (DEBUG, INFO, WARNING, ERROR)
- Adjust the number of lines displayed
- Auto-refresh every 5 seconds
- Modern, responsive UI
- Color-coded log levels for better visibility
- Display of detailed log metadata (file, line number, thread ID, etc.)

## Setup

1. Navigate to the log_viewer directory:
```bash
cd log_viewer
```

2. Create a virtual environment (recommended):
```bash
python -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate
```

3. Install dependencies:
```bash
pip install -r requirements.txt
```

4. Create a `logs` directory in the project root:
```bash
mkdir logs
```

5. Run the application:
```bash
python log_viewer.py
```

6. Open your browser and navigate to `http://localhost:5000`

## Usage

1. Select a log file from the dropdown menu
2. Choose a log level to filter (optional)
3. Select the number of lines to display
4. Click the refresh button or wait for auto-refresh

## Log Format Support

The application expects JSON-formatted logs where each line is a JSON object with the following structure:

```json
{
    "timestamp": "2024-03-21T10:30:45.123Z",
    "level": "INFO",
    "fields": {
        "message": "Your log message here"
    },
    "target": "module_name",
    "filename": "source_file.rs",
    "line_number": 42,
    "threadId": "thread-1"
}
```

All fields are optional, but the following are recommended for best results:
- `timestamp`: The time when the log entry was created
- `level`: The log level (DEBUG, INFO, WARNING, ERROR)
- `fields.message`: The main log message
- `target`: The module or component that generated the log
- `filename`: The source file where the log was generated
- `line_number`: The line number in the source file
- `threadId`: The ID of the thread that generated the log

## Directory Structure

```
log_viewer/
├── log_viewer.py
├── requirements.txt
├── README.md
├── logs/
└── templates/
    └── index.html
``` 