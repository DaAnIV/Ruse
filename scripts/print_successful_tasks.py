import json
import shutil
import os.path
import datetime
from argparse import ArgumentParser

def parse_args():
    parser = ArgumentParser('Results helper')
    parser.add_argument('results', help="The result file to parser")
    return parser.parse_args()


def main():
    args = parse_args()

    with open(args.results) as fp:
        results = json.load(fp)

    count = 0    
    tasks = results['tasks']
    for task in tasks:
        if task['error'] is not None: continue

        count += 1
        took = datetime.timedelta(seconds = task["total_time"]["secs"], microseconds=task["total_time"]["nanos"] // 1000)

        print(f'task {os.path.basename(task["path"])} passed')
        print(f'solution: "{task["found"]}" in {took.total_seconds():.3f} seconds')
        print(f'{task["path"]}')
        print()

    print(f'{count}/{len(tasks)}')

if __name__ == '__main__':
    main()