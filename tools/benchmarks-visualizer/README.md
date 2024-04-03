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
```

## RUN DASHBOARD MANUALLY

To run Dash dashboard manually simply execute the app.py script:
```bash
python3 src/app.py
```

To then visualize the dashboard (for now, until hosted), open a web browser and navigate to `http://127.0.0.1:8050/`:
```bash
# for windows 10/11
start http://127.0.0.1:8050/

# for macOS
open http://127.0.0.1:8050/ 

# for Linux (most desktop env)
xdg-open http://127.0.0.1:8050/
```