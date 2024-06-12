import json
import datetime
import random
from itertools import accumulate
from bokeh.embed import components
from bokeh.plotting import ColumnDataSource, figure
from bokeh.models import Range1d, HoverTool, DataRange1d, ResetTool

class BenchmarkIteration:
    def __init__(self, iteration):
        self.time = datetime.timedelta(seconds=iteration['time']['secs'], microseconds=iteration['time']['nanos']/1000)
        self.statistics = iteration['statistics']

class BenchmarkTask:
    def __init__(self, task) -> None:
        self.json = task
        self.name = task['name']
        self.error = task['error']
        self.found = task['found']
        if self.found is None: 
            self.found = "-"
        if task['iterations'] is None:
            self.iterations = []
        else:
            self.iterations = list(map(BenchmarkIteration, task['iterations']))
        self.statistics = task['total_statistics']
        if task['total_time'] is None:
            self.took = datetime.timedelta(milliseconds=-1)
        else:
            self.took = datetime.timedelta(seconds=task['total_time']['secs'], microseconds=task['total_time']['nanos']/1000)
        
    def get_evaluated_bank_size_graph(self):
        if self.error: 
            return None

        p = figure(height=350, sizing_mode="stretch_width", x_axis_label="iterations", y_axis_label="programs")

        x = list(range(len(self.iterations)+1))
        # y_max = max(self.statistics['Evaluated'], self.statistics['BankSize'])+1

        source = ColumnDataSource(data=dict(
            iteration=x,
            evaluated = [0] + list(accumulate(map(lambda x: x.statistics['Evaluated'], self.iterations))),
            inserted = [0] + list(accumulate(map(lambda x: x.statistics['BankSize'], self.iterations)))
        ))
        p.x_range = Range1d(0, len(self.iterations)+1, bounds='auto')
        p.y_range = DataRange1d(start=0, bounds='auto')
        p.xaxis.minor_tick_line_color = None

        p.line('iteration', 'evaluated', source=source, legend_label="Evaluated", color="blue")
        p.line('iteration', 'inserted',  source=source, legend_label="Bank Size", color="red")
        evaluated_plot = p.circle('iteration', 'evaluated', source=source, legend_label="Evaluated", color="blue")
        inserted_plot = p.circle('iteration', 'inserted',  source=source, legend_label="Bank Size", color="red")
        
        p.add_tools(HoverTool(
            renderers=[evaluated_plot],
            tooltips=[
                ("iteration", "$index"),
                ("evaluated", "@evaluated")
            ]
        ))
        p.add_tools(HoverTool(
            renderers=[inserted_plot],
            tooltips=[
                ("iteration", "$index"),
                ("inserted", "@inserted")
            ]
        ))

        return components(p)

class Benchmarks:
    def __init__(self, file) -> None:
        content = json.load(file)
        self.date = datetime.datetime.fromtimestamp(content['timestamp'], tz=datetime.timezone.utc)
        self.sysinfo = content['sysinfo']
        self.tasks = dict(map(lambda x: (x['name'], BenchmarkTask(x)), content['tasks']))

    def get_graphs(self):
        # Creating Plot Figure
        p = figure(height=350, sizing_mode="stretch_width")
    
        # Defining Plot to be a Scatter Plot
        p.circle(
            [i for i in range(10)],
            [random.randint(1, 50) for j in range(10)],
            size=20,
            color="navy",
            alpha=0.5
        )
    
        # Get Chart Components
        return components(p)