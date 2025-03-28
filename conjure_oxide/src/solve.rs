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
use conjure_core::{
    context::Context,
    pro_trace::{create_consumer, specify_trace_file, Consumer},
    rule_engine::{resolve_rule_sets, rewrite_naive},
    Model,
};
use conjure_oxide::{
    defaults::DEFAULT_RULE_SETS,
    find_conjure::conjure_executable,
    get_rules, model_from_json,
    utils::{
        conjure::{get_minion_solutions, minion_solutions_to_json},
        essence_parser::parse_essence_file_native,
    },
    SolverFamily,
};
use serde_json::to_string_pretty;

use crate::cli::{self, GlobalArgs};

#[derive(Clone, Debug, clap::Args)]
pub struct Args {
    /// The input Essence file
    #[arg(value_name = "INPUT_ESSENCE")]
    pub input_file: PathBuf,

    /// Save execution info as JSON to the given filepath.
    #[arg(long)]
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
    #[arg(long, short = 'o')]
    pub output: Option<PathBuf>,
}

pub fn run_solve_command(global_args: GlobalArgs, solve_args: Args) -> anyhow::Result<()> {
    let input_file = solve_args.input_file.clone();

    let file = specify_trace_file(
        input_file.to_string_lossy().into_owned(),
        global_args.trace_file.clone(),
        global_args.formatter.as_str(),
    );
    //consumer for protrace
    let consumer: Option<Consumer> = global_args.tracing.then(|| {
        create_consumer(
            global_args.trace_output.as_str(),
            global_args.verbosity.clone(),
            global_args.formatter.as_str(),
            file,
        )
    });

    let context = init_context(&global_args, input_file)?;
    let model = parse(&global_args, Arc::clone(&context))?;
    let rewritten_model = rewrite(model, &global_args, Arc::clone(&context), consumer)?;

    if solve_args.no_run_solver {
        println!("{}", rewritten_model);
    } else {
        run_solver(&solve_args, rewritten_model)?;
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
    let target_family = global_args.solver.unwrap_or(SolverFamily::Minion);
    let mut extra_rule_sets: Vec<&str> = DEFAULT_RULE_SETS.to_vec();
    for rs in &global_args.extra_rule_sets {
        extra_rule_sets.push(rs.as_str());
    }

    ensure!(
        target_family == SolverFamily::Minion,
        "Only the Minion solver is currently supported!"
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
        rules.iter().map(|rd| format!("{}", rd)).collect::<Vec<_>>().join("\n")
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
    consumer: Option<Consumer>,
) -> anyhow::Result<Model> {
    tracing::info!("Initial model: \n{}\n", model);

    tracing::info!("Rewriting model...");

    let rule_sets = context.read().unwrap().rule_sets.clone();
    tracing::info!("Rewriting model...");
    let new_model = rewrite_naive(
        &model,
        &rule_sets,
        global_args.check_equally_applicable_rules,
        consumer,
    )?;

    tracing::info!("Rewritten model: \n{}\n", new_model);
    Ok(new_model)
}

fn run_solver(cmd_args: &Args, model: Model) -> anyhow::Result<()> {
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

    let solutions = get_minion_solutions(model, cmd_args.number_of_solutions)?; // ToDo we need to properly set the solver adaptor here, not hard code minion
    tracing::info!(target: "file", "Solutions: {}", minion_solutions_to_json(&solutions));

    let solutions_json = minion_solutions_to_json(&solutions);
    let solutions_str = to_string_pretty(&solutions_json)?;
    match out_file {
        None => {
            println!("Solutions:");
            println!("{}", solutions_str);
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
