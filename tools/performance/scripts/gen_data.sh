#!/bin/bash

WORKING_DIR="$(pwd)/tools/performance"
CMD_FILE="${WORKING_DIR}/commands.txt"

cargo build --release #prevents from needing to recompile each call

pushd $WORKING_DIR
parallel --no-notice :::: ${CMD_FILE}
popd