#!/bin/bash

if [[ "$#" -ne 1 ]]; then
    echo "$0 <benchmark output json>"
    exit 1
fi

SCRIPT_DIR=$(dirname "$0")

# pushd $SCRIPT_DIR
flask --app "$SCRIPT_DIR/app:create_app(benchmarks='$1')" run --debug
# popd