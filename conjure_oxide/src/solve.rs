//! conjure_oxide solve sub-command
#![allow(clippy::unwrap_used)]
use std::{
    fs::File,
    io::Write,
    path::PathBuf,
    process::exit,
    sync::{Arc, RwLock},
};

use anyhow::{anyhow, ensure};
use clap::ValueHint;
use conjure_core::{
    context::Context,
    pro_trace::{
        create_consumer, display_message, json_trace_close, set_kind_filter, set_rule_filter,
        set_rule_set_filter, specify_trace_files, Consumer, Kind,
    },
    rule_engine::{resolve_rule_sets, rewrite_naive},
    Model,
};
use conjure_oxide::{
    defaults::DEFAULT_RULE_SETS,
    find_conjure::conjure_executable,
    get_rules, model_from_json, parse_essence_file_native,
    utils::conjure::{get_minion_solutions, get_sat_solutions, solutions_to_json},
    SolverFamily,
};
use serde_json::to_string_pretty;

use crate::cli::GlobalArgs;

#[derive(Clone, Debug, clap::Args)]
pub struct Args {
    /// The input Essence file
    #[arg(value_name = "INPUT_ESSENCE", value_hint = ValueHint::FilePath)]
    pub input_file: PathBuf,

    /// Save execution info as JSON to the given filepath.
    #[arg(long ,value_hint=ValueHint::FilePath)]
    pub info_json_path: Option<PathBuf>,

    /// Do not run the solver.
    ///
    /// The rewritten model is printed to stdout in an Essence-style syntax (but is not necessarily
    /// valid Essence).
    #[arg(long, default_value_t = false)]
    pub no_run_solver: bool,

    /// Number of solutions to return. 0 returns all solutions
    #[arg(long, default_value_t = 0, short = 'n')]
    pub number_of_solutions: i32,

    /// Save solutions to the given JSON file
    #[arg(long, short = 'o', value_hint = ValueHint::FilePath)]
    pub output: Option<PathBuf>,
}

pub fn run_solve_command(global_args: GlobalArgs, solve_args: Args) -> anyhow::Result<()> {
    let input_file = solve_args.input_file.clone();

    set_kind_filter(global_args.kind_filter.clone());
    set_rule_filter(global_args.rule_name_filter.clone());
    set_rule_set_filter(global_args.rule_set_filter.clone());

    // Determining the file for the output of the trace

    let files = specify_trace_files(
        input_file.to_string_lossy().into_owned(),
        global_args.trace_file.clone(),
        &global_args.formatter,
    );
    // Consumer for protrace
    let consumer: Option<Consumer> = global_args.tracing.then(|| {
        create_consumer(
            global_args.trace_output.as_str(),
            global_args.verbosity.clone(),
            global_args.formatter.as_str(),
            files.0.clone(),
            files.1.clone(),
        )
    });

    let context = init_context(&global_args, input_file)?;
    let model = parse(&global_args, Arc::clone(&context))?;
    let rewritten_model = rewrite(model, &global_args, Arc::clone(&context), consumer)?;

    if global_args.trace_output == "file"
        && (global_args.formatter == "json" || global_args.formatter == "both")
    {
        json_trace_close(files.0.clone());
    }

    if solve_args.no_run_solver {
        println!("{}", rewritten_model);
    } else {
        match global_args.solver {
            SolverFamily::SAT => {
                run_sat_solver(&solve_args, rewritten_model)?;
            }
            SolverFamily::Minion => {
                run_minion(&solve_args, rewritten_model)?;
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

    ensure!(
        target_family == SolverFamily::Minion || target_family == SolverFamily::SAT,
        "Only the Minion and SAT solvers is currently supported!"
    );

    let rule_sets = match resolve_rule_sets(target_family, &extra_rule_sets) {
        Ok(rs) => rs,
        Err(e) => {
            display_message(
                format!("Error resolving rule sets: {}", e),
                None,
                Kind::Error,
            );
            exit(1);
        }
    };

    let pretty_rule_sets = rule_sets
        .iter()
        .map(|rule_set| rule_set.name)
        .collect::<Vec<_>>()
        .join(", ");

    display_message(
        format!("Enabled rule sets: [{}]\n", pretty_rule_sets),
        None,
        Kind::Rules,
    );

    display_message(
        format!("Rule sets: {}\n", pretty_rule_sets),
        None,
        Kind::Rules,
    );

    let rules = get_rules(&rule_sets)?.into_iter().collect::<Vec<_>>();

    display_message(
        format!(
            "Rules: {}\n",
            rules
                .iter()
                .map(|rd| format!("{}", rd))
                .collect::<Vec<_>>()
                .join("\n")
        ),
        None,
        Kind::Rules,
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

    if global_args.enable_native_parser {
        parse_essence_file_native(input_file.as_str(), context.clone()).map_err(|e| anyhow!(e))
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
            display_message("extra-rule-checks: enabled".to_string(), None, Kind::Rules);
        } else {
            display_message("extra-rule-checks: disabled".to_string(), None, Kind::Rules);
        }

        model_from_json(&astjson, context.clone()).map_err(|e| anyhow!(e))
    }
}

pub(crate) fn rewrite(
    model: Model,
    global_args: &GlobalArgs,
    context: Arc<RwLock<Context<'static>>>,
    consumer: Option<Consumer>,
) -> anyhow::Result<Model> {
    display_message(format!("Initial model: \n{}\n", model), None, Kind::Model);

    let rule_sets = context.read().unwrap().rule_sets.clone();
    let new_model = rewrite_naive(
        &model,
        &rule_sets,
        global_args.check_equally_applicable_rules,
        consumer,
    )?;

    display_message(
        format!("Rewritten model: \n{}\n", new_model),
        None,
        Kind::Model,
    );

    Ok(new_model)
}

fn run_minion(cmd_args: &Args, model: Model) -> anyhow::Result<()> {
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

    let solutions = get_minion_solutions(model, cmd_args.number_of_solutions)?;

    let solutions_json = solutions_to_json(&solutions);
    let solutions_str = to_string_pretty(&solutions_json)?;
    match out_file {
        None => {
            println!("Solutions:\n{}", solutions_str);
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

fn run_sat_solver(cmd_args: &Args, model: Model) -> anyhow::Result<()> {
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

    let solutions = get_sat_solutions(model, cmd_args.number_of_solutions)?;

    let solutions_json = solutions_to_json(&solutions);
    let solutions_str = to_string_pretty(&solutions_json)?;
    match out_file {
        None => {
            display_message(
                format!("Solutions:\n{}", solutions_str),
                None,
                Kind::Default,
            );
        }
        Some(mut outf) => {
            outf.write_all(solutions_str.as_bytes())?;
            display_message(
                format!(
                    "Solution saved to {:?}",
                    &cmd_args.output.clone().unwrap().canonicalize()?
                ),
                None,
                Kind::Default,
            );
        }
    }
    Ok(())
}
