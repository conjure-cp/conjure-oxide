//! conjure_oxide solve sub-command
#![allow(clippy::unwrap_used)]
use std::{
    fs::File,
    io::Write as _,
    path::PathBuf,
    process::exit,
    sync::{Arc, RwLock},
};

use anyhow::{anyhow, ensure};
use clap::ValueHint;
use conjure_cp::defaults::DEFAULT_RULE_SETS;
use conjure_cp::parse::tree_sitter::parse_essence_file_native;
use conjure_cp::{
    Model,
    ast::comprehension::USE_OPTIMISED_REWRITER_FOR_COMPREHENSIONS,
    context::Context,
    rule_engine::{resolve_rule_sets, rewrite_morph, rewrite_naive},
    solver::{Solver, adaptors},
};
use conjure_cp::{
    parse::conjure_json::model_from_json, rule_engine::get_rules, solver::SolverFamily,
};
use conjure_cp_cli::find_conjure::conjure_executable;
use conjure_cp_cli::utils::conjure::{get_minion_solutions, get_sat_solutions, solutions_to_json};
use serde_json::to_string_pretty;

use crate::cli::{GlobalArgs, LOGGING_HELP_HEADING};

#[derive(Clone, Debug, clap::Args)]
pub struct Args {
    /// The input Essence file
    #[arg(value_name = "INPUT_ESSENCE", value_hint = ValueHint::FilePath)]
    pub input_file: PathBuf,

    /// Save execution info as JSON to the given filepath.
    #[arg(long ,value_hint=ValueHint::FilePath,help_heading=LOGGING_HELP_HEADING)]
    pub info_json_path: Option<PathBuf>,

    /// Do not run the solver.
    ///
    /// The rewritten model is printed to stdout in an Essence-style syntax
    /// (but is not necessarily valid Essence).
    #[arg(long, default_value_t = false)]
    pub no_run_solver: bool,

    /// Number of solutions to return. 0 returns all solutions
    #[arg(long, default_value_t = 0, short = 'n')]
    pub number_of_solutions: i32,

    /// Save solutions to the given JSON file
    #[arg(long, short = 'o', value_hint = ValueHint::FilePath,help_heading=LOGGING_HELP_HEADING)]
    pub output: Option<PathBuf>,
}

pub fn run_solve_command(global_args: GlobalArgs, solve_args: Args) -> anyhow::Result<()> {
    let input_file = solve_args.input_file.clone();

    // each step is in its own method so that similar commands
    // (e.g. testsolve) can reuse some of these steps.

    let context = init_context(&global_args, input_file)?;
    let model = parse(&global_args, Arc::clone(&context))?;
    let rewritten_model = rewrite(model, &global_args, Arc::clone(&context))?;

    if solve_args.no_run_solver {
        println!("{}", &rewritten_model);

        // TODO: we want to be able to do let solver = match family {....}, but something weird is
        // happening in the types..
        if let Some(path) = global_args.save_solver_input_file {
            eprintln!("Writing solver input file to {}", path.display());
            let mut file = File::create(path).unwrap();

            match global_args.solver {
                SolverFamily::Sat => {
                    let solver = Solver::new(adaptors::Sat::default());
                    let solver = solver.load_model(rewritten_model)?;
                    solver.write_solver_input_file(&mut file)?;
                }
                SolverFamily::Minion => {
                    let solver = Solver::new(adaptors::Minion::default());
                    let solver = solver.load_model(rewritten_model)?;
                    solver.write_solver_input_file(&mut file)?;
                }
            };
        }
    } else {
        match global_args.solver {
            SolverFamily::Sat => {
                run_sat_solver(&global_args, &solve_args, rewritten_model)?;
            }
            SolverFamily::Minion => {
                run_minion(&global_args, &solve_args, rewritten_model)?;
            }
        }
    }

    // still do postamble even if we didn't run the solver
    if let Some(ref path) = solve_args.info_json_path {
        let context_obj = context.read().unwrap().clone();
        let generated_json = &serde_json::to_value(context_obj)?;
        let pretty_json = serde_json::to_string_pretty(&generated_json)?;
        File::create(path)?.write_all(pretty_json.as_bytes())?;
    }
    Ok(())
}

/// Initialises the context for solving.
pub(crate) fn init_context(
    global_args: &GlobalArgs,
    input_file: PathBuf,
) -> anyhow::Result<Arc<RwLock<Context<'static>>>> {
    let target_family = global_args.solver;
    let mut extra_rule_sets: Vec<&str> = DEFAULT_RULE_SETS.to_vec();
    for rs in &global_args.extra_rule_sets {
        extra_rule_sets.push(rs.as_str());
    }

    if global_args.no_use_expand_ac {
        extra_rule_sets.pop_if(|x| x == &"Better_AC_Comprehension_Expansion");
    }

    ensure!(
        target_family == SolverFamily::Minion || target_family == SolverFamily::Sat,
        "Only the Minion and SAT solvers is currently supported!"
    );

    let rule_sets = match resolve_rule_sets(target_family, &extra_rule_sets) {
        Ok(rs) => rs,
        Err(e) => {
            tracing::error!("Error resolving rule sets: {}", e);
            exit(1);
        }
    };

    let pretty_rule_sets = rule_sets
        .iter()
        .map(|rule_set| rule_set.name)
        .collect::<Vec<_>>()
        .join(", ");

    tracing::info!("Enabled rule sets: [{}]", pretty_rule_sets);
    tracing::info!(
        target: "file",
        "Rule sets: {}",
        pretty_rule_sets
    );

    let rules = get_rules(&rule_sets)?.into_iter().collect::<Vec<_>>();
    tracing::info!(
        target: "file",
        "Rules: {}",
        rules.iter().map(|rd| format!("{rd}")).collect::<Vec<_>>().join("\n")
    );
    let context = Context::new_ptr(
        target_family,
        extra_rule_sets.iter().map(|rs| rs.to_string()).collect(),
        rules,
        rule_sets.clone(),
    );

    context.write().unwrap().file_name = Some(input_file.to_str().expect("").into());

    Ok(context)
}

pub(crate) fn parse(
    global_args: &GlobalArgs,
    context: Arc<RwLock<Context<'static>>>,
) -> anyhow::Result<Model> {
    let input_file: String = context
        .read()
        .unwrap()
        .file_name
        .clone()
        .expect("context should contain the input file");

    tracing::info!(target: "file", "Input file: {}", input_file);
    if global_args.use_native_parser {
        parse_essence_file_native(input_file.as_str(), context.clone()).map_err(|e| e.into())
    } else {
        conjure_executable()
            .map_err(|e| anyhow!("Could not find correct conjure executable: {}", e))?;

        let mut cmd = std::process::Command::new("conjure");
        let output = cmd
            .arg("pretty")
            .arg("--output-format=astjson")
            .arg(input_file)
            .output()?;

        let conjure_stderr = String::from_utf8(output.stderr)?;

        ensure!(conjure_stderr.is_empty(), conjure_stderr);

        let astjson = String::from_utf8(output.stdout)?;

        if cfg!(feature = "extra-rule-checks") {
            tracing::info!("extra-rule-checks: enabled");
        } else {
            tracing::info!("extra-rule-checks: disabled");
        }

        model_from_json(&astjson, context.clone()).map_err(|e| anyhow!(e))
    }
}

pub(crate) fn rewrite(
    model: Model,
    global_args: &GlobalArgs,
    context: Arc<RwLock<Context<'static>>>,
) -> anyhow::Result<Model> {
    tracing::info!("Initial model: \n{}\n", model);

    let rule_sets = context.read().unwrap().rule_sets.clone();

    let new_model = if global_args.use_optimised_rewriter {
        USE_OPTIMISED_REWRITER_FOR_COMPREHENSIONS.store(true, std::sync::atomic::Ordering::Relaxed);
        tracing::info!("Rewriting the model using the optimising rewriter");
        rewrite_morph(
            model,
            &rule_sets,
            global_args.check_equally_applicable_rules,
        )
    } else {
        tracing::info!("Rewriting the model using the default / naive rewriter");
        if global_args.exit_after_unrolling {
            tracing::info!("Exiting after unrolling");
        }
        rewrite_naive(
            &model,
            &rule_sets,
            global_args.check_equally_applicable_rules,
            global_args.exit_after_unrolling,
        )?
    };

    tracing::info!("Rewritten model: \n{}\n", new_model);
    Ok(new_model)
}

fn run_minion(global_args: &GlobalArgs, cmd_args: &Args, model: Model) -> anyhow::Result<()> {
    let out_file: Option<File> = match &cmd_args.output {
        None => None,
        Some(pth) => Some(
            File::options()
                .create(true)
                .truncate(true)
                .write(true)
                .open(pth)?,
        ),
    };

    let solutions = get_minion_solutions(
        model,
        cmd_args.number_of_solutions,
        &global_args.save_solver_input_file,
    )?;
    tracing::info!(target: "file", "Solutions: {}", solutions_to_json(&solutions));

    let solutions_json = solutions_to_json(&solutions);
    let solutions_str = to_string_pretty(&solutions_json)?;
    match out_file {
        None => {
            println!("Solutions:");
            println!("{solutions_str}");
        }
        Some(mut outf) => {
            outf.write_all(solutions_str.as_bytes())?;
            println!(
                "Solutions saved to {:?}",
                &cmd_args.output.clone().unwrap().canonicalize()?
            )
        }
    }
    Ok(())
}

fn run_sat_solver(global_args: &GlobalArgs, cmd_args: &Args, model: Model) -> anyhow::Result<()> {
    let out_file: Option<File> = match &cmd_args.output {
        None => None,
        Some(pth) => Some(
            File::options()
                .create(true)
                .truncate(true)
                .write(true)
                .open(pth)?,
        ),
    };

    let solutions = get_sat_solutions(
        model,
        cmd_args.number_of_solutions,
        &global_args.save_solver_input_file,
    )?;
    tracing::info!(target: "file", "Solutions: {}", solutions_to_json(&solutions));

    let solutions_json = solutions_to_json(&solutions);
    let solutions_str = to_string_pretty(&solutions_json)?;
    match out_file {
        None => {
            println!("Solutions:");
            println!("{solutions_str}");
        }
        Some(mut outf) => {
            outf.write_all(solutions_str.as_bytes())?;
            println!(
                "Solutions saved to {:?}",
                &cmd_args.output.clone().unwrap().canonicalize()?
            )
        }
    }
    Ok(())
}
