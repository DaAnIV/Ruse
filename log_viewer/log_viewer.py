from flask import Flask, render_template, jsonify, request
import os
import json
from datetime import datetime
import re

app = Flask(__name__)

# Configuration
LOG_DIR = "logs"  # Directory where logs are stored
DEFAULT_LOG_FILE = "app.json"  # Default log file to display
PAGE_SIZE = 100  # Number of logs per page

def filter_backtrace(backtrace):
    """Filter out system and cargo package lines from backtrace."""
    if not isinstance(backtrace, str):
        return backtrace

    # Split into lines and filter
    lines = backtrace.split('\n')
    filtered_lines = []

    i = 0
    while i < len(lines):
        line = lines[i]
        # Check if this is a trace start (starts with number and colon)
        if re.match(r'^\s*\d+:', line):
            # Check if this is a single-line trace
            if i + 1 >= len(lines) or re.match(r'^\s*\d+:', lines[i + 1]) is not None:
                # Single-line trace, skip it
                i += 1
                continue

            # Two-line trace, check the second line for system packages
            location_line = lines[i + 1]
            if not any(pkg in location_line.lower() for pkg in ['/rustc/', '/.cargo/', '/std/', '/core/', '/alloc/', '/.rustup/']):
                # Keep both lines
                filtered_lines.append(line.strip())
                filtered_lines.append('\t' + location_line.strip())
            i += 2

    return '\n'.join(filtered_lines)

def parse_log_line(line):
    """Parse a single JSON log line and extract all fields."""
    try:
        log_entry = json.loads(line.strip())
        # Get all fields, with message as a special case
        fields = log_entry.get('fields', {})
        message = fields.pop('message', '')  # Remove message from fields to display separately
        
        # Handle panic backtrace specially
        if 'panic.backtrace' in fields:
            # Store the original backtrace
            fields['panic.full_backtrace'] = fields['panic.backtrace']
            # Create filtered version
            fields['panic.backtrace'] = filter_backtrace(fields['panic.backtrace'])
        
        # Add panic payload to message if it exists
        if 'panic.payload' in fields:
            payload = fields.pop('panic.payload')
            if message:
                message = f"{message}\npayload: {payload}"
            else:
                message = f"payload: {payload}"
        
        filename = log_entry.get('filename', '')
        line_number = log_entry.get('line_number', '')
        
        # Modify location to panic location
        if 'panic.location' in fields:
            location = fields['panic.location'].split(':')
            filename = location[0]
            line_number = location[1]
        
        return {
            'timestamp': log_entry.get('timestamp', ''),
            'level': log_entry.get('level', ''),
            'message': message,
            'fields': fields,  # All other fields
            'target': log_entry.get('target', ''),
            'filename': filename,
            'line_number': line_number,
            'thread_id': log_entry.get('threadId', '')
        }
    except json.JSONDecodeError:
        return None

def get_log_files():
    """Get list of available log files."""
    if not os.path.exists(LOG_DIR):
        os.makedirs(LOG_DIR)
    return [f for f in os.listdir(LOG_DIR) if f.endswith('.json')]

def get_unique_thread_ids(log_path):
    """Get list of unique thread IDs from the log file."""
    thread_ids = set()
    try:
        with open(log_path, 'r') as f:
            for line in f:
                parsed = parse_log_line(line)
                if parsed and parsed['thread_id']:
                    thread_ids.add(parsed['thread_id'])
    except Exception:
        pass
    return sorted(list(thread_ids))

def count_matching_logs(log_path, level='', thread_id='', message_filter=''):
    """Count the number of logs matching the filters."""
    count = 0
    try:
        with open(log_path, 'r') as f:
            for line in f:
                parsed = parse_log_line(line)
                if parsed and (not level or parsed['level'].lower() == level.lower()) and \
                   (not thread_id or parsed['thread_id'] == thread_id) and \
                   (not message_filter or message_filter.lower() in parsed['message'].lower()):
                    count += 1
    except Exception:
        pass
    return count

@app.route('/')
def index():
    """Render the main page."""
    log_files = get_log_files()
    return render_template('index.html', log_files=log_files)

@app.route('/api/threads')
def get_threads():
    """API endpoint to get unique thread IDs."""
    log_file = request.args.get('file', DEFAULT_LOG_FILE)
    log_path = os.path.join(LOG_DIR, log_file)
    if not os.path.exists(log_path):
        return jsonify({'error': 'Log file not found'}), 404

    thread_ids = get_unique_thread_ids(log_path)
    return jsonify(thread_ids)

@app.route('/api/logs/count')
def get_log_count():
    """API endpoint to get total number of logs matching filters."""
    log_file = request.args.get('file', DEFAULT_LOG_FILE)
    level = request.args.get('level', '')
    thread_id = request.args.get('thread_id', '')

    log_path = os.path.join(LOG_DIR, log_file)
    if not os.path.exists(log_path):
        return jsonify({'error': 'Log file not found'}), 404

    count = count_matching_logs(log_path, level, thread_id)
    return jsonify({'count': count})

@app.route('/api/logs')
def get_logs():
    """API endpoint to get log entries with pagination."""
    log_file = request.args.get('file', DEFAULT_LOG_FILE)
    level = request.args.get('level', '')
    thread_id = request.args.get('thread_id', '')
    message_filter = request.args.get('message', '')
    page = request.args.get('page', 1, type=int)

    log_path = os.path.join(LOG_DIR, log_file)
    if not os.path.exists(log_path):
        return jsonify({'error': 'Log file not found'}), 404

    entries = []
    current_count = 0
    start_index = (page - 1) * PAGE_SIZE
    end_index = start_index + PAGE_SIZE

    with open(log_path, 'r') as f:
        for line in f:
            parsed = parse_log_line(line)
            if parsed and (not level or parsed['level'].lower() == level.lower()) and \
               (not thread_id or parsed['thread_id'] == thread_id) and \
               (not message_filter or message_filter.lower() in parsed['message'].lower()):
                if start_index <= current_count < end_index:
                    entries.append(parsed)
                current_count += 1
                if current_count >= end_index:
                    break

    return jsonify({
        'entries': entries,
        'page': page,
        'page_size': PAGE_SIZE,
        'total_count': count_matching_logs(log_path, level, thread_id, message_filter)
    })

if __name__ == '__main__':
    app.run(debug=True, port=5000)
