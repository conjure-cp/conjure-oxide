//! This is a small example showing how to use this library to report solver stats from conjure.

use conjure_cp_bench::{
    executor::conjure::{ConjureExecutor, ConjureExecutorBuilder},
    mode::{BenchmarkingMode, FixedIterationsMode, ModeOutputKey},
    model_sources::models_from_directory_tree,
};
use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    conjure_executable: PathBuf,
    models_directory: PathBuf,
    i: usize,
}

fn main() {
    let cli = Cli::parse();

    // an instance of conjure to run the benchmarks on
    let executor: ConjureExecutor = ConjureExecutorBuilder::new(cli.conjure_executable)
        .add_flag("--solver=minion".to_owned())
        .build();

    let benchmark_mode = FixedIterationsMode::new(cli.i);

    // convert models_directory to absolute path before passing down the line, otherwise things
    // break.
    let models_directory = std::fs::canonicalize(&cli.models_directory).unwrap();
    let models_and_params = models_from_directory_tree(&models_directory).unwrap();

    let results = benchmark_mode.run(&models_and_params, &executor).unwrap();
    println!("{0: <20.20} {1: <20.20} mean", "instance", "counterName");
    for (
        ModeOutputKey {
            model,
            param,
            counter_name,
        },
        v,
    ) in results.counters.into_iter()
    {
        let v_mean: Option<f64> = v
            .iter()
            .copied()
            .flatten()
            .enumerate()
            .reduce(|(_, x_acc), (i, x)| (i, (x_acc + x)))
            .map(|(v_size, v_sum)| v_sum / (v_size as f64));

        let v_mean_string = match v_mean {
            Some(v) => format!("{v:.02}"),
            None => String::from("N/A"),
        };

        let instance_string = param.unwrap_or_else(|| model);

        println!("{instance_string: <20.20} {counter_name: <20.20}  {v_mean_string}");
    }
}
