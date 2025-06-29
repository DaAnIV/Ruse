import base64
import json
import sys
import pandas as pd
import graphviz

def flatten_data(y, sep='_'):
    out = {}

    def flatten(x, name=''):
        if type(x) is dict:
            for a in x:
                flatten(x[a], name + a + sep)
        elif type(x) is list:
            i = 0
            for a in x:
                flatten(a, name + str(i) + sep)
                i += 1
        else:
            out[name[:-1]] = x

    flatten(y)
    return out

class LogFile:
    def __init__(self, log_path):
        """Open a log file and return a pandas DataFrame."""

        self.__log = pd.read_json(log_path, lines=True)

        thread_prefix_len = len('ThreadId(')
        self.__log['threadId'] = self.__log['threadId'].apply(lambda x: int(x[thread_prefix_len:-1]))

        self.thread_ids = self.__log['threadId'].unique()
        self.thread_ids.sort()

        self.has_graphviz = True
        try:
            graphviz.version()
        except graphviz.ExecutableNotFound:
            self.has_graphviz = False
            print("Graphviz is not installed. Please install graphviz to render dot files.", file=sys.stderr)

    def __filter_logs__(self, level=None, thread_id=None, message_filter=None):
        filtered_logs = self.__log.copy()
        if level:
            filtered_logs = filtered_logs[filtered_logs['level'].str.lower() == level.lower()]
        if thread_id:
            filtered_logs = filtered_logs[filtered_logs['threadId'] == int(thread_id)]
        if message_filter:
            filtered_logs = filtered_logs[filtered_logs['fields'].apply(lambda x: message_filter.lower() in x['message'].lower())]

        return filtered_logs

    def get_logs(self, level=None, thread_id=None, message_filter=None, start_index=None, end_index=None):
        filtered_logs = self.__filter_logs__(level, thread_id, message_filter)
        filtered_count = len(filtered_logs)

        if start_index is not None and end_index is not None:
            filtered_logs = filtered_logs.iloc[start_index:end_index]

        if len(filtered_logs) == 0:
            return filtered_logs, filtered_count
        
        filtered_logs['message'] = filtered_logs['fields'].apply(self.__parse_message__)
        filtered_logs['filename'] = filtered_logs[['filename', 'fields']].apply(self.__parse_filename__, axis=1)
        filtered_logs['line_number'] = filtered_logs[['line_number','fields']].apply(self.__parse_line_number__, axis=1)
        filtered_logs['fields'].apply(self.__parse_log_fields__)

        return filtered_logs, filtered_count


    def __parse_message__(self, fields):
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

    def __render_dot_fields__(self, fields):
        """Render dot files to svg."""

        if not self.has_graphviz:
            return

        def process_fields(fields_dict):
            dot_srcs = []
            for key, value in fields_dict.items():
                if isinstance(value, dict):
                    process_fields(value)
                elif isinstance(value, list):
                    for item in value:
                        if isinstance(item, dict):
                            process_fields(item)
                elif key.endswith('.dot'):
                    dot_srcs.append((key, value))

            for (key, value) in dot_srcs:
                src = graphviz.Source(value)
                rendered = src.pipe(format='svg')
                fields_dict[key + '.svg'] = base64.b64encode(rendered).decode('utf-8')

        process_fields(fields)
    
    def __parse_log_fields__(self, fields):
        """Parse log fields and return a dictionary."""

        # Handle panic backtrace specially
        if 'panic.backtrace' in fields and fields['panic.backtrace'] != 'disabled backtrace':
            # Store the original backtrace
            fields['panic.full_backtrace'] = fields['panic.backtrace']
            # Create filtered version
            fields['panic.backtrace'] = self.__filter_backtrace__(fields['panic.backtrace'])

        self.__parse_json__(fields)

        # Render dot files to svg
        self.__render_dot_fields__(fields)

        return fields

    def __parse_json__(self, fields):
        """Flatten json fields."""

        json_keys = []
        for key, _ in fields.items():
            if key.endswith('.json'):
                json_keys.append(key)

        for key in json_keys:
            json_data = fields.pop(key)
            json_data = json.loads(json_data)
            key = key.replace('.json', '')
            fields.update({key: json_data})


    def __filter_backtrace__(self, backtrace):
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

    def __parse_filename__(self, data):
        """Get filename from fields."""    
        (file_name, fields) = data

        # Modify location to panic location
        if 'panic.location' in fields:
            location = fields['panic.location'].split(':')
            return location[0].strip()
        
        return file_name

    def __parse_line_number__(self, data):
        """Get line number from fields."""
        (line_number, fields) = data

        # Modify location to panic location
        if 'panic.location' in fields:
            location = fields['panic.location'].split(':')
            return location[1].strip()
        
        return line_number
