import json
import shutil
import os.path
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
        print(f'task {os.path.basename(task["path"])} passed')
        print(f'{task["path"]}')
        print()

    print(f'{count}/{len(tasks)}')

if __name__ == '__main__':
    main()