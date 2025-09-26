import datetime
import json
import multiprocessing
import os
import subprocess
import time

import jsonlines

def get_local_dir():
    return os.path.dirname(os.path.realpath(__file__))

def get_workspace_root():
    notebook_path = get_local_dir()
    workspace_root = notebook_path
    while not os.path.exists(os.path.join(workspace_root, "Cargo.toml")):
        parent_dir = os.path.dirname(workspace_root)
        if parent_dir == workspace_root:
            raise Exception("Could not find workspace root")
        workspace_root = parent_dir
    return os.path.relpath(workspace_root, notebook_path)

def spawn_ruse(cmd):
    if os.fork() != 0:
        return

    proc = subprocess.Popen(cmd, close_fds=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    proc.wait()


def run_ruse_executable(args, in_background=False, ignore_output=False, profile="release"):
    ruse_bin = os.path.join(get_workspace_root(), "target", profile, "Ruse")
    cmd = [ruse_bin, "run"] + args

    if in_background:
        cmd = ["nohup"] + cmd
        ignore_output = True
    
    print(' '.join(cmd))

    if in_background:
        p = multiprocessing.Process(target=spawn_ruse, args=[cmd], daemon=True)
        p.start()
        p.join()
        return

    if ignore_output:
        subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    else:
        subprocess.run(cmd)

def run_ruse(tasks, results_dir, *,
             log_file = None,
             timeout=datetime.timedelta(hours=1),
             max_iterations=5,
             max_sequence_size=2,
             max_mutations=3,
             max_memory_usage="100GiB",
             workers_count=64,
             dry_run=False,
             in_background=False):
    args = [
        "-o", results_dir,
        "-t", str(int(timeout.total_seconds())),
        "--workers-count", str(workers_count),
        "--max-iterations", str(max_iterations),
        "--max-mutations", str(max_mutations),
        "--max-sequence-size", str(max_sequence_size),
        "--max-task-mem", max_memory_usage]

    if log_file is not None:
        args.append("--log")
        args.append(log_file)

    if dry_run:
        args.append("--dry-run")    
    
    for task in tasks:
        args.append("-b")
        args.append(task)
    
    run_ruse_executable(args, in_background, ignore_output=in_background or dry_run)

def parse_log(log_file):
    with jsonlines.open(log_file, "r") as reader:
        log = [line for line in reader]
    return log

def get_ruse_pid(results_dir, timeout_seconds=10):
    metadata_path = os.path.join(results_dir, "metadata.json")
    timeout = time.time() + timeout_seconds
    while not os.path.exists(metadata_path):
        if time.time() > timeout:
            raise Exception("Timeout waiting for metadata file")
        time.sleep(0.1)

    with open(metadata_path, "r") as f:
        metadata = json.load(f)
    return metadata['pid']

def check_process_running(pid):
    try:
        os.kill(pid, 0)
    except OSError:
        return False
    return True

def wait_for_process(pid, timeout_seconds=None):
    start_time = time.time()
    while check_process_running(pid):
        if timeout_seconds is not None and time.time() - start_time > timeout_seconds:
            raise TimeoutError("Process did not finish in time")
        time.sleep(0.1)
