#!/bin/bash
# @author: Pedro Gronda Garrigues

# NOTE TO FUTURE DEVELOPERS:
#  - This script is intended to run conjure-oxide on all exhaustive tests in the original conjure native repository.
#  - Understand that, until there is full support for all expressions in conjure-oxide, most of the tests will fail.
#  - Therefore, the purpose of the script is to be modular, and ignore failed tests (simply won't write a solution stats file).

# define project directory relative to the script directory
PROJECT_DIR="./tools/benchmarks-visualizer"

# define directory containing the .essence files relative to benchmarks-visualizer directory
REPO_DIR="tests/exhaustive"

# define directory where output files will be written relative to benchmarks-visualizer directory
OUTPUT_DIR="data"

# create output directory if it doesn't exist
mkdir -p $OUTPUT_DIR

# change to parent directory to run cargo command
cd ../..

# define solvers to use for conjure oxide
SOLVERS=(
  "minion"
  "kissat"
)

# find all .essence files in the repository directory and loop through them
find $PROJECT_DIR/$REPO_DIR -type f -name "*.essence" | while read essence_file; do
    
    for solver in "${SOLVERS[@]}"; do
        # prefix to strip from essence file for redefinition of .json output file
        prefix="$PROJECT_DIR/$REPO_DIR"

        # strip essence file path to get JSON output formatting
        stripped_essence_file="${essence_file#$prefix}"    # strip the prefix from the essence file
        stripped_essence_file="${stripped_essence_file#/}" # eliminate the first "/" for command formatting

        # define json
        json_output_file="${stripped_essence_file%.essence}_oxide_$solver.json"

        echo "COMMAND SO FAR: cargo run -- --info-json-path $PROJECT_DIR/$OUTPUT_DIR/$json_output_file $essence_file"

        # Skip the generation if the JSON file already exists
        if [[ -f "$PROJECT_DIR/$OUTPUT_DIR/$json_output_file" ]]; then
            echo "STATUS: Skipping $json_output_file as it already exists."
            continue
        fi
        
        echo "STATUS: Running solver $solver on $test_name"
        if ! timeout 5s cargo run -- --info-json-path "$PROJECT_DIR/$OUTPUT_DIR/$json_output_file $essence_file"; then
            echo "STATUS: Failed to run solver $solver on $test_name; timeout (5 seconds) reached, continuing with next test."
            continue
        fi
        echo "STATUS: Finished running solver $solver on $test_name"
    done
done

# move back into benchmarks_visualizer directory after execution
cd ./tools/benchmarks-visualizer
