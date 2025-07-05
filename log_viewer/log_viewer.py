from flask import Flask, render_template, jsonify, request
import os
from log_parser import LogFile
import ray
import modin.config as modin_cfg

app = Flask(__name__)

# Configuration
LOG_DIR = "logs"  # Directory where logs are stored
PAGE_SIZE = 100  # Number of logs per page

# Simple in-memory cache for LogFile objects
_log_file_cache = {}

def get_log_files():
    """Get list of available log files."""
    if not os.path.exists(LOG_DIR):
        os.makedirs(LOG_DIR)
    return [f for f in os.listdir(LOG_DIR) if f.endswith('.jsonl')]

def get_cached_log_file(log_path):
    """Get a LogFile object from cache or create and cache it."""
    if log_path not in _log_file_cache:
        print(f"Creating new LogFile object for: {log_path}")
        _log_file_cache[log_path] = LogFile(log_path)
    else:
        print(f"Using cached LogFile object for: {log_path}")
    return _log_file_cache[log_path]

@app.route('/')
def index():
    """Render the file selection page."""
    log_files = get_log_files()
    return render_template('index.html', log_files=log_files)

@app.route('/view/<path:log_file>')
def viewer(log_file):
    """Render the log viewer page for a specific log file."""
    log_path = os.path.join(LOG_DIR, log_file)
    if not os.path.exists(log_path):
        return "Log file not found", 404
    
    # Get filter parameters from URL
    level = request.args.get('level', '')
    thread_id = request.args.get('thread_id', '')
    message_filter = request.args.get('message', '')
    page = request.args.get('page', 1, type=int)
    
    return render_template('viewer.html', 
                         log_file=log_file, 
                         current_level=level,
                         current_thread_id=thread_id,
                         current_message_filter=message_filter,
                         current_page=page)


@app.route('/api/thread-ids/<path:log_file>')
def api_thread_ids(log_file):
    """API endpoint to get thread IDs for a log file."""
    log_path = os.path.join(LOG_DIR, log_file)
    if not os.path.exists(log_path):
        return jsonify({'error': 'Log file not found'}), 404
    
    # Get the log file from cache (or create and cache it)
    log_file_obj = get_cached_log_file(log_path)
    
    return jsonify({
        'thread_ids': log_file_obj.thread_ids.tolist()
    })

@app.route('/api/logs/<path:log_file>')
def api_logs(log_file):
    """API endpoint to get log entries as JSON."""
    log_path = os.path.join(LOG_DIR, log_file)
    if not os.path.exists(log_path):
        return jsonify({'error': 'Log file not found'}), 404
    
    # Get the log file from cache (or create and cache it)
    log_file_obj = get_cached_log_file(log_path)
    
    # Get filter parameters from URL
    level = request.args.get('level', '')
    thread_id = request.args.get('thread_id', '')
    message_filter = request.args.get('message', '')
    page = request.args.get('page', 1, type=int)
    
    # Process the log data server-side
    start_index = (page - 1) * PAGE_SIZE
    end_index = start_index + PAGE_SIZE
    
    (log_entries, filtered_count) = log_file_obj.get_logs(level, thread_id, message_filter, start_index, end_index)
    
    # Convert to list of dictionaries for JSON response
    entries = log_entries.to_dict(orient='records') if not log_entries.empty else []
    
    # Calculate pagination info
    total_pages = (filtered_count + PAGE_SIZE - 1) // PAGE_SIZE
    
    return jsonify({
        'entries': entries,
        'pagination': {
            'current_page': page,
            'total_pages': total_pages,
            'total_count': filtered_count,
            'page_size': PAGE_SIZE
        },
        'filters': {
            'level': level,
            'thread_id': thread_id,
            'message': message_filter
        }
    })

@app.route('/clear-cache/<path:log_file>')
def remove_from_cache(log_file):
    """Clear the log file cache."""
    global _log_file_cache
    del _log_file_cache[log_file]
    return jsonify({'message': f'Removed {log_file} from cache.'})


@app.route('/clear-cache')
def clear_cache():
    """Clear the log file cache."""
    global _log_file_cache
    cache_size = len(_log_file_cache)
    _log_file_cache.clear()
    return jsonify({'message': f'Cache cleared. Removed {cache_size} cached files.'})


@app.route('/cache-status')
def cache_status():
    """Get the current cache status."""
    return jsonify({
        'cached_files': list(_log_file_cache.keys()),
        'cache_size': len(_log_file_cache)
    })

def main():
    ray.init()
    modin_cfg.Engine.put("ray") # Modin will use Ray engine

    app.run(debug=True, port=5000)

if __name__ == '__main__':
    main()
    