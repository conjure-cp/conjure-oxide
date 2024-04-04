#!/bin/bash
# @author: Pedro Gronda Garrigues

# prompt for updating Conjure Native data
read -p "Do you want to update Conjure Native exhaustive solver stats? (yes/no) " update_native
if [[ $update_native == "yes" ]]; then
    # build and run conjure_native_benchmarks
    echo "Building Conjure Native benchmarks..."
    CARGO_TARGET_DIR=./tools/benchmarks-visualizer cargo build --bin conjure_native_benchmarks

    if [[ $? -eq 0 ]]; then
        echo "STATUS: Running Conjure Native exhaustive tests..."
        ./tools/benchmarks-visualizer/debug/conjure_native_benchmarks
        python3 ./src/download_visualizations.py # download the visualizations after update stats
    else
        echo "STATUS: Build failed for Conjure Native benchmarks."
        exit 1
    fi
fi

# prompt for updating Conjure Oxide data
read -p "Do you want to update Conjure Oxide solver stats? (yes/no) " update_oxide
if [[ $update_oxide == "yes" ]]; then
    # run conjure_oxide_benchmarks.sh script
    echo "STATUS: Running Conjure Oxide benchmarks..."
    ./src/conjure_oxide_benchmarks.sh
    python3 ./src/download_visualizations.py # download the visualizations after update stats
fi

# prompt for updating Conjure Oxide data
read -p "Would you like to generate the static dashboard (.html file)? (yes/no) " static_dashboard
if [[ $static_dashboard == "yes" ]]; then
    # run conjure_oxide_benchmarks.sh script
    echo "STATUS: Generating .qml file and .html static dashboard..."
    
    # (re)generate the .qml file
    python3 ./src/generate_qml_file.py

    # sanity check for Quarto install dependency
    if ! command -v quarto &> /dev/null; then
        echo "STATUS: Quarto could not be found. Please install Quarto before proceeding."
        exit 1
    fi

    # convert .qmd to HTML using Quarto
    quarto render ./html/dashboard.qmd
    if [[ $? -ne 0 ]]; then
        echo "STATUS: Error occurred while generating the static dashboard."
        exit 2
    fi
fi

# execute the Python script for visualization dyanamic dashboard
echo "STATUS: Running the Python visualization app..."
python3 ./src/dash_dashboard.py

# check for any errors in Python script execution
if [[ $? -ne 0 ]]; then
    echo "STATUS: Error occurred while running the Python visualization app."
    exit 2
fi

echo "STATUS: Benchmark visualization process completed successfully."
