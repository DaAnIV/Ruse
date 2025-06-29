from flask import Flask, render_template, jsonify, request
import os
from log_parser import LogFile

app = Flask(__name__)

# Configuration
LOG_DIR = "logs"  # Directory where logs are stored
DEFAULT_LOG_FILE = "app.json"  # Default log file to display
PAGE_SIZE = 100  # Number of logs per page

def get_log_files():
    """Get list of available log files."""
    if not os.path.exists(LOG_DIR):
        os.makedirs(LOG_DIR)
    return [f for f in os.listdir(LOG_DIR) if f.endswith('.jsonl')]


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

    log_file = LogFile(log_path)

    thread_ids = log_file.thread_ids
    return thread_ids.tolist()

@app.route('/api/logs')
def get_logs_api():
    """API endpoint to get log entries with pagination."""
    log_file = request.args.get('file', DEFAULT_LOG_FILE)
    level = request.args.get('level', '')
    thread_id = request.args.get('thread_id', '')
    message_filter = request.args.get('message', '')
    page = request.args.get('page', 1, type=int)

    log_path = os.path.join(LOG_DIR, log_file)
    if not os.path.exists(log_path):
        return jsonify({'error': 'Log file not found'}), 404

    log_file = LogFile(log_path)

    start_index = (page - 1) * PAGE_SIZE
    end_index = start_index + PAGE_SIZE

    (log, filtered_count) = log_file.get_logs(level, thread_id, message_filter, start_index, end_index)

    return jsonify({
        'entries': log.to_dict(orient='records'),
        'page': page,
        'page_size': PAGE_SIZE,
        'total_count': filtered_count
    })

if __name__ == '__main__':
    app.run(debug=True, port=5000)
