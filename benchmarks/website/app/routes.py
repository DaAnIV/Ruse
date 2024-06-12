from datetime import datetime, timezone
from urllib.parse import urlsplit
from flask import render_template, current_app, Blueprint, redirect, url_for
from .benchmarks import Benchmarks, BenchmarkTask, BenchmarkIteration

routes = Blueprint('routes', __name__, url_prefix='/')

@routes.route('/')
@routes.route('/index')
def index():
    return render_template('index.html', title='Home', benchmarks=current_app.config['benchmarks'])

@routes.route('/refresh')
def refresh():
    path = current_app.config['benchmarks_path']
    with open(path) as f:
        current_app.config['benchmarks'] = Benchmarks(f)
    return redirect(url_for('routes.index'))

@routes.route('/task/<task_name>')
def task(task_name):
    task: BenchmarkTask = current_app.config['benchmarks'].tasks[task_name]
    return render_template('task.html', title=task, benchmarks=current_app.config['benchmarks'], task=task, graph=task.get_evaluated_bank_size_graph())

@routes.route('/statistics')
def statistics():
    benchmarks: Benchmarks = current_app.config['benchmarks']
    script, div = benchmarks.get_graphs()
    return render_template('statistics.html', title="Statistics", benchmarks=benchmarks, graphs_div=div, graphs_script=script)


@routes.errorhandler(404)
def not_found_error(error):
    return render_template('404.html'), 404


@routes.errorhandler(500)
def internal_error(error):
    return render_template('500.html'), 500