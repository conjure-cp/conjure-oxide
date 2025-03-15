#!/bin/bash

WORKING_DIR="$(pwd)/tools/performance"
CMD_FILE="${WORKING_DIR}/commands.txt"

pushd $WORKING_DIR
parallel --no-notice :::: ${CMD_FILE}
popd