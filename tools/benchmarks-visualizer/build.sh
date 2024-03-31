#!/bin/bash
# @author: Pedro Gronda Garrigues

# prompt for updating Conjure Native data
read -p "Do you want to update Conjure Native exhaustive test data? (yes/no) " update_native
if [[ $update_native == "yes" ]]; then
    # build and run conjure_native_benchmarks
    echo "Building Conjure Native benchmarks..."
    CARGO_TARGET_DIR=./tools/benchmarks-visualizer cargo build --bin conjure_native_benchmarks

    if [[ $? -eq 0 ]]; then
        echo "STATUS: Running Conjure Native exhaustive tests..."
        ./tools/benchmarks-visualizer/debug/conjure_native_benchmarks
    else
        echo "STATUS: Build failed for Conjure Native benchmarks."
        exit 1
    fi
fi

# prompt for updating Conjure Oxide data
read -p "Do you want to update Conjure Oxide data? (yes/no) " update_oxide
if [[ $update_oxide == "yes" ]]; then
    # run conjure_oxide_benchmarks.sh script
    echo "STATUS: Running Conjure Oxide benchmarks..."
    ./src/conjure_oxide_benchmarks.sh
fi

# execute the Python script for visualization dashboard
echo "STATUS: Running the Python visualization app..."
python3 ./src/app.py

# check for any errors in Python script execution
if [[ $? -ne 0 ]]; then
    echo "STATUS: Error occurred while running the Python visualization app."
    exit 2
fi

echo "STATUS: Benchmark visualization process completed successfully."
