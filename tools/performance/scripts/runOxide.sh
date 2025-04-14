#!/bin/bash

#May not be best method as requires compilation each time, which feels inefficient and messy?
PROBLEM=$1
ESSENCE=$2

FULL_ESSENCE="$(pwd)/tests/${PROBLEM}/${ESSENCE}"

DATA_DIR="$(pwd)/data/NV_CO/${PROBLEM}/"
rm -r -f $DATA_DIR
mkdir $DATA_DIR
JSON_FILE="${DATA_DIR}/oxide-stats.json"

# look into adding one flag 
../../target/release/conjure_oxide solve --solver Minion --info-json-path=$JSON_FILE $FULL_ESSENCE #this means it uses precompiled oxide

rm $(pwd)/*.log
rm $(pwd)/conjure_oxide_log.json