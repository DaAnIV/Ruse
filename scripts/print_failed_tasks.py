import json
import shutil
import os.path
from argparse import ArgumentParser

def parse_args():
    parser = ArgumentParser('Results helper')
    parser.add_argument('results', help="The result file to parser")
    parser.add_argument('--no-deadline', action='store_true')
    parser.add_argument('--copy', help='Copy failed tasks to this dir')
    return parser.parse_args()


def main():
    args = parse_args()

    with open(args.results) as fp:
        results = json.load(fp)

    count = 0    
    tasks = results['tasks']
    for task in tasks:
        if task['error'] is None: continue
        if args.no_deadline and task['error'] == 'deadline has elapsed': continue

        count += 1
        print(f'task {os.path.basename(task["path"])} failed')
        print(f'error: {task["error"]}')
        print(f'{task["path"]}')
        if args.copy is not None:
            shutil.copy(task["path"], args.copy)
        print()

    print(f'{count}/{len(tasks)}')

if __name__ == '__main__':
    main()