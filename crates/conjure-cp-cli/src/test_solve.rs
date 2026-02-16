use std::path::PathBuf;
use std::process::exit;
use std::sync::Arc;

use clap::ValueHint;
use conjure_cp_cli::utils::conjure::{
    get_solutions, get_solutions_from_conjure, solutions_to_json,
};
use conjure_cp_cli::utils::testing::normalize_solutions_for_comparison;

use crate::cli::GlobalArgs;
use crate::solve::{self, init_solver};

#[derive(Clone, Debug, clap::Args)]
pub struct Args {
    /// The input Essence problem file
    #[arg(value_name = "INPUT_ESSENCE",value_hint=ValueHint::FilePath)]
    pub input_file: PathBuf,

    /// The input Essence parameter file
    #[arg(value_name = "PARAM_ESSENCE", value_hint=ValueHint::FilePath)]
    pub param_file: Option<PathBuf>,
}

pub fn run_test_solve_command(global_args: GlobalArgs, local_args: Args) -> anyhow::Result<()> {
    // stealing most of the steps of the solve command, except the solver stuff.
    let input_file = local_args.input_file;
    let param_file = local_args.param_file;

    let context = solve::init_context(&global_args, input_file.clone(), param_file)?;

    // TODO: This is very hacky
    let ctx_lock = context.read().unwrap();
    let input_file_name = ctx_lock
        .input_file_name
        .as_ref()
        .expect("context should contain the problem input file");
    let param_file_name = ctx_lock.param_file_name.as_ref();

    // parse models
    let problem_model = solve::parse(&global_args, Arc::clone(&context), input_file_name)?;
    let unified_model = match param_file_name {
        Some(param_file_name) => {
            let param_model = solve::parse(&global_args, Arc::clone(&context), param_file_name)?;
            solve::merge_models(problem_model, param_model)?
        }
        None => problem_model,
    };

    let rewritten_model = solve::rewrite(unified_model, &global_args, Arc::clone(&context))?;

    let solver = init_solver(&global_args);

    // now we are stealing from the integration tester

    let our_solutions = get_solutions(
        solver,
        rewritten_model,
        0,
        &global_args.save_solver_input_file,
    )?;

    let conjure_solutions =
        get_solutions_from_conjure(input_file.to_str().unwrap(), Arc::clone(&context))?;

    let our_solutions = normalize_solutions_for_comparison(&our_solutions);
    let conjure_solutions = normalize_solutions_for_comparison(&conjure_solutions);

    let mut our_solutions_json = solutions_to_json(&our_solutions);
    let mut conjure_solutions_json = solutions_to_json(&conjure_solutions);

    our_solutions_json.sort_all_objects();
    conjure_solutions_json.sort_all_objects();

    if our_solutions_json == conjure_solutions_json {
        eprintln!("Success: solutions match!");
        exit(0);
    } else {
        eprintln!("=== our solutions:");
        eprintln!("{our_solutions_json}");
        eprintln!("=== conjure's solutions:");
        eprintln!("{conjure_solutions_json}");
        eprintln!("Failure: solutions do not match!");
        exit(1);
    }
}
