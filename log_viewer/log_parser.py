import re
import pandas as pd
import numpy as np

def __open_log_file__(log_path):
    """Open a log file and return a pandas DataFrame."""
    log = pd.read_json(log_path, lines=True)
    thread_prefix_len = len('ThreadId(')
    log['threadId'] = log['threadId'].apply(lambda x: int(x[thread_prefix_len:-1]))
    return log

def get_unique_thread_ids(log_path):
    """Get list of unique thread IDs from the log file."""
    try:
        log = __open_log_file__(log_path)
        thread_ids = log['threadId'].unique()
        thread_ids.sort()
        return thread_ids
    except Exception:
        return []

def __filter_logs__(log, level=None, thread_id=None, message_filter=None):
    if level:
        log = log[log['level'].str.lower() == level.lower()]
    if thread_id:
        log = log[log['threadId'] == int(thread_id)]
    if message_filter:
        log = log[log['fields'].apply(lambda x: message_filter.lower() in x['message'].lower())]

    return log

def get_logs(log_path, level=None, thread_id=None, message_filter=None, start_index=None, end_index=None):
    """Read logs from a file and return a pandas filtered DataFrame."""

    print(f"log_path: {log_path}")
    print(f"level: {level}, thread_id: {thread_id}, message_filter: {message_filter}")

    log = __open_log_file__(log_path)
    total_count = len(log)

    log = __filter_logs__(log, level, thread_id, message_filter)
    filtered_count = len(log)

    if start_index is not None and end_index is not None:
        log = log.iloc[start_index:end_index]

    if len(log) == 0:
        return log, filtered_count, total_count

    log['message'] = log['fields'].apply(parse_message)
    log['filename'] = log[['filename', 'fields']].apply(parse_filename, axis=1)
    log['line_number'] = log[['line_number','fields']].apply(parse_line_number, axis=1)
    log['fields'].apply(parse_log_fields)

    return log, filtered_count, total_count

def parse_message(fields):
    """Get message from fields and add panic payload if it exists."""
    message = fields.pop('message', '')
    
    # Add panic payload to message if it exists
    if 'panic.payload' in fields:
        payload = fields.pop('panic.payload')
        if message:
            message = f"{message}\npayload: {payload}"
        else:
            message = f"payload: {payload}"

    return message

def parse_log_fields(fields):
    """Parse log fields and return a dictionary."""

    # Handle panic backtrace specially
    if 'panic.backtrace' in fields and fields['panic.backtrace'] != 'disabled backtrace':
        # Store the original backtrace
        fields['panic.full_backtrace'] = fields['panic.backtrace']
        # Create filtered version
        fields['panic.backtrace'] = filter_backtrace(fields['panic.backtrace'])

    return fields

def filter_backtrace(backtrace):
    """Filter out system and cargo package lines from backtrace."""
    if not isinstance(backtrace, str):
        return backtrace

    backtrace = pd.Series(backtrace)
    backtrace = backtrace.str.split(r'\n\s*\d+:',expand=True).transpose()
    backtrace = backtrace[0].str.split('\n', expand=True)
    backtrace.columns = ['function', 'location']

    backtrace['function'] = backtrace['function'].str.strip()
    backtrace['location'] = backtrace['location'].str.strip()

    # Drop first line (panic hook)
    backtrace.drop(index=0, inplace=True)

    # Drop lines that don't contain a location
    backtrace = backtrace[backtrace['location'].notna()]
    backtrace = backtrace[backtrace['location'].str.len() > 0]

    # Drop lines that contain system or cargo package names
    backtrace = backtrace[~backtrace['location'].str.contains(r'/rustc/|/.cargo/|/.rustup/|/.cargo/registry/|/.cargo/git/')]

    return backtrace.apply(lambda x: f'{x.name}: {x["function"]}\n\t{x["location"]}', axis=1).str.cat(sep='\n')

def parse_filename(data):
    """Get filename from fields."""    
    (file_name, fields) = data

    # Modify location to panic location
    if 'panic.location' in fields:
        location = fields['panic.location'].split(':')
        return location[0].strip()
    
    return file_name

def parse_line_number(data):
    """Get line number from fields."""
    (line_number, fields) = data

    # Modify location to panic location
    if 'panic.location' in fields:
        location = fields['panic.location'].split(':')
        return location[1].strip()
    
    return line_number
