#!/bin/bash

# ROOT_DIR=$(pwd)
WORKING_DIR="$(pwd)/tools/performance"
TEST_DIR="${WORKING_DIR}/tests"
DATA_DIR="${WORKING_DIR}/data"
O0_DATA_DIR="${DATA_DIR}/O0_CN"
O2_DATA_DIR="${DATA_DIR}/O2_CN"
TM_DATA_DIR="${DATA_DIR}/TM_CO"
NV_DATA_DIR="${DATA_DIR}/NV_CO"
CMD_FILE="${WORKING_DIR}/commands.txt"
VALIDATE=$1

rm -f ${CMD_FILE}
touch ${CMD_FILE}
rm -r $DATA_DIR
mkdir $DATA_DIR
mkdir $O0_DATA_DIR
mkdir $O2_DATA_DIR
mkdir $TM_DATA_DIR
mkdir $NV_DATA_DIR

pushd $TEST_DIR 
for prob in *; do #go through all tests folders in directory
    find $prob -name *.essence -o -name *.eprime | #go through all essence files for this problem, best if only 1
    while read essence
    do
        essence="${essence#$prob}"
        essence="${essence#/}"
        echo "writing to command file"
        echo "./scripts/runConjure.sh ${prob} ${essence} -O0 ${VALIDATE}" >> ${CMD_FILE}
        echo "./scripts/runConjure.sh ${prob} ${essence} -O2 ${VALIDATE}" >> ${CMD_FILE}
        echo "./scripts/runOxide.sh ${prob} ${essence} ${VALIDATE}" >>${CMD_FILE}

        #treemorph rewriter flag not yet added
        # echo "runOxide.sh ${prob} ${essence} --use-treemorph-rewriter" >>${CMD_FILE}
    done
done
popd