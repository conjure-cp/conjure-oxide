#!/bin/bash

echo "${1} ${2} ${3}"
PROBLEM=$1
ESSENCE=$2
PARAM=$3
VALIDATE=$4

WORKING_DIR="$(pwd)"
FULL_ESSENCE="${WORKING_DIR}/tests/${PROBLEM}/${ESSENCE}"
if [[ $PARAM = "-O0" ]]; then
    DATA_DIR="$(pwd)/data/O0_CN/${PROBLEM}/"
elif [[ $PARAM = "-O2" ]]; then
    DATA_DIR="$(pwd)/data/O2_CN/${PROBLEM}/"
else
    echo "Optimisation mode not recognised: ${$3}"
    exit 1
fi

rm -r -f $DATA_DIR
mkdir $DATA_DIR

if [[ $VALIDATE = "validate" ]]; then
    conjure solve -o $DATA_DIR --number-of-solutions=all --validate-solutions $FULL_ESSENCE 
else
    conjure solve -o $DATA_DIR --number-of-solutions=all $FULL_ESSENCE 
fi

rm -f $DATA_DIR/*.{eprime,eprime-solution,eprime-infor,eprime-*,conjure-checksum,solution}