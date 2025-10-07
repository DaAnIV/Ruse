import os
import sys
import json
import argparse

def merge_run_results(run_dir):
    metadata = None
    tasks = []
    tasks_paths = []

    for name in os.listdir(run_dir):
        with open(os.path.join(run_dir, name), 'r') as f:
            if name == 'metadata.json':
                metadata = json.load(f)
            else:
                task = json.load(f)
                tasks_paths.append(task["path"])
                tasks.append(task)

    if metadata is None:
        raise ValueError("metadata.json not found.")

    missing_task = False
    for path in metadata["benchmarks"]:
        if path not in tasks_paths:
            print(f"Missing task {os.path.basename(path)} result.", file=sys.stderr)
            missing_task = True

    if missing_task:
        raise ValueError("Some task results are missing.")

    merged_result = {
        'metadata': metadata,
        'tasks': tasks
    }

    return merged_result

def main():
    parser = argparse.ArgumentParser(description="Merge results from a run.")
    parser.add_argument('run_dir', nargs='+', help='Directory containing input files to merge')
    args = parser.parse_args()

    for run_dir in args.run_dir:
        merged_result = merge_run_results(run_dir)
        output_file = run_dir + '.json'

        with open(output_file, 'w') as out_file:
            json.dump(merged_result, out_file, indent=4)

        print(f"Merged {len(merged_result['tasks'])} tasks from {run_dir}.")

if __name__ == '__main__':
    main()