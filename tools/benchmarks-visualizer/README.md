# BENCHMARKS VISUALIZER

A dashboard to visualize useful statistics comparing Conjure (native) and Conjure Oxide

Stats included thus far:
 - Num of Nodes (incomplete, need comparison between node count for native vs oxide)
 - Time elapsed (milliseconds, ms)

Author: Pedro Gronda Garrigues (220027583)

## DEPENDENCIES

To download requirement dependencies, follow the folling steps.

*Note: this is done manually instead of in build.sh for safety reasons.

1. Make sure you are in the correct repo directory
```bash
cd ./tools/benchmarks-visualizer
```

2. Create virtual environment
```bash
python -m venv env
```

3. Activate virtual environment
```bash
# for Windows 10/11
.\env\Scripts\activate

# for macOS/Linux
source env/bin/activate
```

4. Install required packages
```bash
pip install -r requirements.txt
```

## EXECUTE

A shell script is provided, which includes a prompted option to rerun all exhaustive tests for both Conjure and Conjure Oxide:
```bash
./build.sh
```

## UPDATE MANUALLY

To update conjure native solver solutions statistics and oxide statistics, please execute the following (already included in build.sh):
```bash
# ensure you are in ./tools/benchmarks-visualizer directory
cd ./tools/benchmarks-visualizer

# to write data to ./data for conjure native for solver(s)
CARGO_TARGET_DIR=./tools/benchmarks-visualizer cargo build --bin conjure_native_benchmarks
./tools/benchmarks-visualizer/debug/conjure_native_benchmarks

# to write data to ./data for conjure oxide (Minion supported for now) -- warning: very verbose
./src/conjure_oxide_benchmarks.sh

# WARNING: ensure figures are written to ./figure dir for static dashboard
python3 ./src/download_visualizations.py
```

## GET STATIC DASHBOARD

To get static HTML dashboard located in `tools/benchmarks-visualizer/html/dashboard.html` run the following commands:
```bash
# WARNING: ensure figures are written to ./figure dir for static dashboard
python3 ./src/download_visualizations.py

# (re)generate the .qml file
python3 ./src/generate_qml_file.py

# render to HTML using quarto
quarto render ./html/dashboard.qmd
```