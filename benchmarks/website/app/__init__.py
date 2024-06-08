import logging
from flask import Flask
from flask_moment import Moment
from config import Config
from .routes import routes
from .benchmarks import Benchmarks

moment = Moment()

def create_app(benchmarks):
    app = Flask(__name__)
    app.config.from_object(Config)
    with open(benchmarks) as f:
        app.config['benchmarks'] = Benchmarks(f)

    if not app.debug:
        app.logger.setLevel(logging.INFO)

    moment.init_app(app)
    app.register_blueprint(routes)

    app.logger.info('Created app')

    return app

from . import routes
