use crate::exit;
use std::path::PathBuf;
use std::sync::Arc;

use crate::cli::GlobalArgs;
use crate::solve;
use clap::ValueHint;
use conjure_cp::instantiate::{instantiate_model, validate_instantiation_conditions};
use conjure_cp::pipeline::Pipeline;
use conjure_cp::solutions::SolutionOptions;
use conjure_cp_cli::utils::conjure::{get_solutions_from_conjure, solutions_to_json};
use conjure_cp_cli::utils::testing::normalize_solutions_for_comparison;

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
    let pipeline = Pipeline::new(global_args.library_config())?;
    pipeline.with_settings(|| run_test_solve(&pipeline, global_args, local_args))
}

fn run_test_solve(
    pipeline: &Pipeline,
    global_args: GlobalArgs,
    local_args: Args,
) -> anyhow::Result<()> {
    let input_file = local_args.input_file;
    let param_file = local_args.param_file;
    let context = pipeline.context();
    {
        let mut context = context.write().unwrap();
        context.essence_file_name = Some(input_file.to_string_lossy().into_owned());
        context.param_file_name = param_file
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned());
    }
    let unified_model = pipeline.with_settings(|| -> anyhow::Result<_> {
        let problem_model = pipeline.parse_file(&input_file)?;
        match param_file.as_ref() {
            Some(param_file) => {
                let param_model = solve::parse_param(pipeline, param_file, &problem_model)?;
                Ok(instantiate_model(problem_model, param_model)?)
            }
            None => {
                let mut problem_model = problem_model;
                validate_instantiation_conditions(&mut problem_model)?;
                Ok(problem_model)
            }
        }
    })?;
    let rewritten_model = pipeline.rewrite(unified_model)?;
    let our_solutions = pipeline.solve(
        rewritten_model,
        &SolutionOptions {
            number_of_solutions: 0,
            solver_input_file: global_args.save_solver_input_file.clone(),
            rule_trace_cdp: global_args.rule_trace_cdp,
            ..Default::default()
        },
    )?;

    let conjure_solutions = pipeline.with_settings(|| {
        get_solutions_from_conjure(
            input_file.to_str().unwrap(),
            param_file.as_deref().map(|f| f.to_str().unwrap()),
            Arc::clone(&context),
        )
    })?;

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
