# this sets `CARGO_TARGET_DIR` just for the duration of the command
CARGO_TARGET_DIR=./tools/benchmarks-visualizer cargo build --bin conjure_native_benchmarks
./tools/benchmarks-visualizer/debug/conjure_oxide_benchmarks

cd tools/benchmarks-visualizer