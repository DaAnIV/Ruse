import argparse
import sh
import json
import subprocess
import select
import os

def print_log(filename, tail):
    # Check if terminal supports colors
    use_colors = os.getenv('TERM') != 'dumb' and os.getenv('NO_COLOR') is None

    colors = {
        'INFO': '\033[38;5;10m',        # Green for INFO
        'WARNING': '\033[38;5;11m',     # Yellow for WARNING
        'ERROR': '\033[38;5;9m',        # Red for ERROR
        'DEBUG': '\033[38;5;255m',        # White for DEBUG
        'TRACE': '\033[38;5;255m',        # White for TRACE

        'timestamp': '\033[38;5;238m',  # Light gray for timestamp
        'thread_id': '\033[38;5;44m',   # Cyan for thread_id
        'target': '\033[38;5;238m',     # Light gray for target
        'location': '\033[38;5;238m',   # Light gray for location
        'message': '\033[38;5;255m',    # White for message

        'DEFAULT': '\033[0m',           # Reset
    }

    if tail:
        lines = sh.tail('-F', filename, _iter=True)
    else:
        with open(filename, 'r') as f:
            lines = f.readlines()

    for line in lines:
            log = json.loads(line)
            
            # Extract timestamp and format it
            # timestamp = log['timestamp']
            level = log['level']
            # thread_id = log.get('threadId', 'Unknown')
            # target = log.get('target', 'unknown')
            filename = log.get('filename', 'unknown')
            line_number = log.get('line_number', 'unknown')
            message = log['fields']['message']
            
            if use_colors:
                # Apply colors more simply - just color the level and keep the rest 
                # output += f"{colors['timestamp']}{timestamp}{colors['DEFAULT']}"
                level = f"{colors[level]}{level}{colors['DEFAULT']}"
                # output += f" {colors['thread_id']}{thread_id}{colors['DEFAULT']}"
                # output += f" {colors['target']}{target}{colors['DEFAULT']}"
                # output += f" {colors['location']}{filename}:{line_number}{colors['DEFAULT']}"
                message = f"{colors['message']}{message}{colors['DEFAULT']}"
                print(f'{level}\t{message}')
            else:
                # Fallback without colors
                print(f"{level} {filename}:{line_number} {message}")

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument('filename', type=str)
    parser.add_argument('-t', '--tail', action='store_true')
    args = parser.parse_args()
    print_log(args.filename, args.tail)