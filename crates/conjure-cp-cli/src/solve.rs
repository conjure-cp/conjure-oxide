//! conjure_oxide solve sub-command
#![allow(clippy::unwrap_used)]
use std::time::Duration;
use std::{
    fs::File,
    io::Write as _,
    path::PathBuf,
    process::exit,
    sync::{Arc, RwLock},
};

use anyhow::anyhow;
use clap::ValueHint;
use conjure_cp::defaults::DEFAULT_RULE_SETS;
use conjure_cp::instantiate::instantiate_model;
use conjure_cp::{
    Model,
    context::Context,
    rule_engine::{resolve_rule_sets, rewrite_morph, rewrite_naive},
    settings::{
        Rewriter, set_comprehension_expander, set_current_parser, set_current_rewriter,
        set_current_solver_family, set_minion_discrete_threshold,
    },
    solver::Solver,
};
use conjure_cp::{
    parse::conjure_json::model_from_json, rule_engine::get_rules, settings::SolverFamily,
};
use conjure_cp::{parse::tree_sitter::parse_essence_file_native, solver::adaptors::*};
use conjure_cp_cli::find_conjure::conjure_executable;
use conjure_cp_cli::utils::conjure::{get_solutions, solutions_to_json};
use serde_json::to_string_pretty;

use crate::cli::{GlobalArgs, LOGGING_HELP_HEADING};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumberOfSolutions {
    All,
    Limit(i32),
}

impl NumberOfSolutions {
    fn as_solver_limit(self) -> i32 {
        match self {
            NumberOfSolutions::All => 0,
            NumberOfSolutions::Limit(limit) => limit,
        }
    }
}

fn parse_number_of_solutions(input: &str) -> Result<NumberOfSolutions, String> {
    if input.eq_ignore_ascii_case("all") {
        return Ok(NumberOfSolutions::All);
    }

    let limit = input
        .parse::<i32>()
        .map_err(|_| "expected a positive integer or 'all'".to_string())?;

    if limit <= 0 {
        return Err("expected a positive integer or 'all'".to_string());
    }

    Ok(NumberOfSolutions::Limit(limit))
}

#[derive(Clone, Debug, clap::Args)]
pub struct Args {
    /// The input Essence problem file
    #[arg(value_name = "INPUT_ESSENCE", value_hint = ValueHint::FilePath)]
    pub essence_file: PathBuf,

    /// The input Essence parameter file
    #[arg(value_name = "PARAM_ESSENCE", value_hint = ValueHint::FilePath)]
    pub param_file: Option<PathBuf>,

    /// Save execution info as JSON to the given filepath.
    #[arg(long ,value_hint=ValueHint::FilePath,help_heading=LOGGING_HELP_HEADING)]
    pub info_json_path: Option<PathBuf>,

    /// Do not run the solver.
    ///
    /// The rewritten model is printed to stdout in an Essence-style syntax
    /// (but is not necessarily valid Essence).
    #[arg(long, default_value_t = false)]
    pub no_run_solver: bool,

    /// Number of solutions to return. Use a positive integer, or `all`.
    #[arg(
        long,
        short = 'n',
        default_value = "1",
        value_name = "N|all",
        value_parser = parse_number_of_solutions
    )]
    pub number_of_solutions: NumberOfSolutions,

    /// Save solutions to the given JSON file
    #[arg(long, short = 'o', value_hint = ValueHint::FilePath,help_heading=LOGGING_HELP_HEADING)]
    pub output: Option<PathBuf>,
}

pub fn run_solve_command(global_args: GlobalArgs, solve_args: Args) -> anyhow::Result<()> {
    let essence_file = solve_args.essence_file.clone();
    let param_file = solve_args.param_file.clone();

    // each step is in its own method so that similar commands
    // (e.g. testsolve) can reuse some of these steps.

    let context = init_context(&global_args, essence_file, param_file)?;

    let ctx_lock = context.read().unwrap();
    let essence_file_name = ctx_lock
        .essence_file_name
        .as_ref()
        .expect("context should contain the problem input file");
    let param_file_name = ctx_lock.param_file_name.as_ref();

    // parse models
    let problem_model = parse(&global_args, Arc::clone(&context), essence_file_name)?;

    // unify models
    let unified_model = match param_file_name {
        Some(param_file_name) => {
            let param_model = parse(&global_args, Arc::clone(&context), param_file_name)?;
            instantiate_model(problem_model, param_model)?
        }
        None => problem_model,
    };
    drop(ctx_lock);

    let rewritten_model = rewrite(unified_model, &global_args, Arc::clone(&context))?;

    let solver = init_solver(&global_args);

    if solve_args.no_run_solver {
        println!("{}", &rewritten_model);

        if let Some(path) = global_args.save_solver_input_file {
            let solver = solver.load_model(rewritten_model)?;
            eprintln!("Writing solver input file to {}", path.display());
            let mut file: Box<dyn std::io::Write> = Box::new(File::create(path)?);
            solver.write_solver_input_file(&mut file)?;
        }
    } else {
        run_solver(
            solver,
            &global_args,
            &solve_args,
            rewritten_model,
            Arc::clone(&context),
        )?
    }

    // Print timing stats if --stats was requested
    if global_args.stats {
        let ctx = context.read().unwrap();
        eprintln!("\n--- Timing Statistics ---");
        for (i, rw) in ctx.stats.rewriter_runs.iter().enumerate() {
            if let Some(dur) = rw.rewriter_run_time {
                eprintln!("Rewrite time (run {}): {:.3}s", i + 1, dur.as_secs_f64());
            }
        }
        for (i, sr) in ctx.stats.solver_runs.iter().enumerate() {
            eprintln!(
                "Solver time  (run {}): {:.3}s",
                i + 1,
                sr.conjure_solver_wall_time_s
            );
        }
        if let Some(dur) = ctx.stats.solution_translation_time {
            eprintln!("Solution translation time: {:.3}s", dur.as_secs_f64());
        }
        eprintln!("-------------------------");
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

/// Returns a new Context and Solver for solving.
pub(crate) fn init_context(
    global_args: &GlobalArgs,
    essence_file: PathBuf,
    param_file: Option<PathBuf>,
) -> anyhow::Result<Arc<RwLock<Context<'static>>>> {
    set_current_parser(global_args.parser);
    set_current_rewriter(global_args.rewriter);
    set_comprehension_expander(global_args.comprehension_expander);
    set_current_solver_family(global_args.solver);
    set_minion_discrete_threshold(global_args.minion_discrete_threshold);

    let target_family = global_args.solver;
    let mut extra_rule_sets: Vec<&str> = DEFAULT_RULE_SETS.to_vec();
    for rs in &global_args.extra_rule_sets {
        extra_rule_sets.push(rs.as_str());
    }

    if let SolverFamily::Sat(sat_encoding) = target_family {
        extra_rule_sets.push(sat_encoding.as_rule_set());
    }

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

    context.write().unwrap().essence_file_name = Some(essence_file.to_str().expect("").into());
    if let Some(param_file) = param_file {
        context.write().unwrap().param_file_name = Some(param_file.to_str().expect("").into());
    }

    Ok(context)
}

pub(crate) fn init_solver(global_args: &GlobalArgs) -> Solver {
    let family = global_args.solver;
    let timeout_ms = global_args
        .solver_timeout
        .map(|dur| Duration::from(dur).as_millis())
        .map(|timeout_ms| u64::try_from(timeout_ms).expect("Timeout too large"));

    match family {
        SolverFamily::Minion => Solver::new(Minion::default()),
        SolverFamily::Sat(_) => Solver::new(Sat::default()),
        SolverFamily::Smt(theory_cfg) => Solver::new(Smt::new(timeout_ms, theory_cfg)),
    }
}

pub(crate) fn parse(
    global_args: &GlobalArgs,
    context: Arc<RwLock<Context<'static>>>,
    file_path: &str,
) -> anyhow::Result<Model> {
    tracing::info!(target: "file", "Input file: {}", file_path);

    match global_args.parser {
        conjure_cp::settings::Parser::TreeSitter => {
            parse_essence_file_native(file_path, context.clone()).map_err(|e| e.into())
        }
        conjure_cp::settings::Parser::ViaConjure => parse_with_conjure(file_path, context.clone()),
    }
}

pub(crate) fn parse_with_conjure(
    input_file: &str,
    context: Arc<RwLock<Context<'static>>>,
) -> anyhow::Result<Model> {
    conjure_executable().map_err(|e| anyhow!("Could not find correct conjure executable: {e}"))?;

    let mut cmd = std::process::Command::new("conjure");
    let output = cmd
        .arg("pretty")
        .arg("--output-format=astjson")
        .arg(input_file)
        .output()?;

    if !output.status.success() {
        println!("Parsing error: {}", String::from_utf8(output.stderr)?);
    }

    let astjson = String::from_utf8(output.stdout)?;

    if cfg!(feature = "extra-rule-checks") {
        tracing::info!("extra-rule-checks: enabled");
    } else {
        tracing::info!("extra-rule-checks: disabled");
    }

    model_from_json(&astjson, context.clone()).map_err(|e| anyhow!(e))
}

pub(crate) fn rewrite(
    model: Model,
    global_args: &GlobalArgs,
    context: Arc<RwLock<Context<'static>>>,
) -> anyhow::Result<Model> {
    tracing::info!("Initial model: \n{}\n", model);

    set_current_rewriter(global_args.rewriter);

    let comprehension_expander = global_args.comprehension_expander;
    set_comprehension_expander(comprehension_expander);
    tracing::info!("Comprehension expander: {}", comprehension_expander);

    let rule_sets = context.read().unwrap().rule_sets.clone();

    let new_model = match global_args.rewriter {
        Rewriter::Morph => {
            tracing::info!("Rewriting the model using the morph rewriter");
            rewrite_morph(
                model,
                &rule_sets,
                global_args.check_equally_applicable_rules,
            )
        }
        Rewriter::Naive => {
            tracing::info!("Rewriting the model using the default / naive rewriter");
            rewrite_naive(
                &model,
                &rule_sets,
                global_args.check_equally_applicable_rules,
            )?
        }
    };

    tracing::info!("Rewritten model: \n{}\n", new_model);
    Ok(new_model)
}

fn run_solver(
    solver: Solver,
    global_args: &GlobalArgs,
    cmd_args: &Args,
    model: Model,
    context: Arc<RwLock<Context<'static>>>,
) -> anyhow::Result<()> {
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

    let solve_result = get_solutions(
        solver,
        model,
        cmd_args.number_of_solutions.as_solver_limit(),
        &global_args.save_solver_input_file,
    )?;

    // Store solution translation time in context stats
    context.write().unwrap().stats.solution_translation_time = Some(solve_result.translation_time);

    let solutions = solve_result.solutions;
    tracing::info!(target: "file", "Solutions: {}", solutions_to_json(&solutions));

    let solutions_json = solutions_to_json(&solutions);
    let solutions_str = to_string_pretty(&solutions_json)?;
    // TODO: do we want to print essence solutions instead?
    // let solutions_essence = solutions_to_essence(&solutions).join("\n----\n");
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
