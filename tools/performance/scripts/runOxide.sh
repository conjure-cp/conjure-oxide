#!/bin/bash

echo "conjure oxide ${1} ${2} ${3}"

PROBLEM=$1
ESSENCE=$2
VALIDATE=$3

FULL_ESSENCE="$(pwd)/tests/${PROBLEM}/${ESSENCE}"

DATA_DIR="$(pwd)/data/NV_CO/${PROBLEM}/"
rm -r -f $DATA_DIR
mkdir $DATA_DIR
JSON_FILE="${DATA_DIR}/oxide-stats.json"

# look into adding one flag 
if [[ $VALIDATE == "validate" ]]; then
    ../../target/release/conjure_oxide test-solve --solver Minion $FULL_ESSENCE
    SUCCESS=$?
    if [[ $SUCCESS == 0 ]]; then
        ../../target/release/conjure_oxide solve --solver Minion --info-json-path=$JSON_FILE $FULL_ESSENCE #this means it uses precompiled oxide
    else
        echo "solution failed validation"
    fi
else
    ../../target/release/conjure_oxide solve --solver Minion --info-json-path=$JSON_FILE $FULL_ESSENCE #this means it uses precompiled oxide
fi 

rm -f $(pwd)/*.log
rm -f $(pwd)/conjure_oxide_log.json
rm -f $(pwd)/tests/$PROBLEM/*.solution