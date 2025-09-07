use std::path::PathBuf;
use std::process::exit;
use std::sync::Arc;

use clap::ValueHint;
use conjure_cp_cli::utils::conjure::{
    get_minion_solutions, get_sat_solutions, get_solutions_from_conjure, solutions_to_json,
};
use conjure_cp_cli::utils::testing::normalize_solutions_for_comparison;

use crate::cli::GlobalArgs;
use crate::solve;

#[derive(Clone, Debug, clap::Args)]
pub struct Args {
    /// The input Essence file
    #[arg(value_name = "INPUT_ESSENCE",value_hint=ValueHint::FilePath)]
    pub input_file: PathBuf,
}

pub fn run_test_solve_command(global_args: GlobalArgs, local_args: Args) -> anyhow::Result<()> {
    // stealing most of the steps of the solve command, except the solver stuff.
    let input_file = local_args.input_file;

    let context = solve::init_context(&global_args, input_file.clone())?;
    let model = solve::parse(&global_args, Arc::clone(&context))?;
    let rewritten_model = solve::rewrite(model, &global_args, Arc::clone(&context))?;

    // now we are stealing from the integration tester

    let our_solutions = match global_args.solver {
        conjure_cp::solver::SolverFamily::Sat => {
            get_sat_solutions(rewritten_model, 0, &global_args.save_solver_input_file)
        }
        conjure_cp::solver::SolverFamily::Minion => {
            get_minion_solutions(rewritten_model, 0, &global_args.save_solver_input_file)
        }
    }?;

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
