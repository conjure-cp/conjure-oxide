#!/bin/bash

echo "${1} ${2} ${3}"
PROBLEM=$1
ESSENCE=$2
PARAM=$3

# BASE_ESSENCE="${ESSENCE%.*}"
WORKING_DIR="$(pwd)"
FULL_ESSENCE="${WORKING_DIR}/tests/${PROBLEM}/${ESSENCE}"
if [ $PARAM = "-O0" ]; then
    DATA_DIR="$(pwd)/data/O0_CN/${PROBLEM}/"
elif [ $PARAM = "-O2" ]; then
    DATA_DIR="$(pwd)/data/O2_CN/${PROBLEM}/"
else
    echo "Optimisation mode not recognised: ${$3}"
    exit 1
fi

rm -r -f $DATA_DIR
mkdir $DATA_DIR

conjure solve -o $DATA_DIR --number-of-solutions=all $FULL_ESSENCE #what oxide does right now
SOLUTION=${WORKING_DIR}/tests/${PROBLEM}
SOLUTION="${SOLUTION}/*.solution"

rm $SOLUTION

rm -f $DATA_DIR/*.{eprime,eprime-solution,eprime-infor,eprime-*,conjure-checksum}
# rm -v !($DATA_DIR/*.stats.json)
# pushd $DATA_DIR
# rm -v !(*.stats.json)
# popd
# rm -v !($DATA_DIR/)