#![allow(clippy::expect_used)]
use conjure_cp::bug;
use conjure_cp::rule_engine::get_rules_grouped;

use conjure_cp::defaults::DEFAULT_RULE_SETS;
use conjure_cp::parse::tree_sitter::parse_essence_file_native;
use conjure_cp::rule_engine::rewrite_naive;
use conjure_cp::solver::Solver;
use conjure_cp::solver::adaptors::*;
use conjure_cp_cli::utils::testing::{normalize_solutions_for_comparison, read_human_rule_trace};
use itertools::Itertools;
use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::fs;
use std::fs::File;
use tracing_subscriber::{Layer, filter::EnvFilter, filter::FilterFn, fmt, layer::SubscriberExt};
use tree_morph::{helpers::select_panic, prelude::*};

#[cfg(feature = "smt")]
use conjure_cp::solver::adaptors::smt::TheoryConfig;

use std::sync::Arc;
use std::sync::RwLock;

use conjure_cp::ast::{Literal, Name};
use conjure_cp::context::Context;
use conjure_cp::parse::tree_sitter::parse_essence_file;
use conjure_cp::rule_engine::resolve_rule_sets;
use conjure_cp::settings::{
    Parser, QuantifiedExpander, Rewriter, SolverFamily, set_comprehension_expander,
    set_current_parser, set_current_rewriter, set_current_solver_family,
};
use conjure_cp_cli::utils::conjure::solutions_to_json;
use conjure_cp_cli::utils::conjure::{get_solutions, get_solutions_from_conjure};
use conjure_cp_cli::utils::testing::save_stats_json;
use conjure_cp_cli::utils::testing::{read_solutions_json, save_solutions_json};
#[allow(clippy::single_component_path_imports, unused_imports)]
use conjure_cp_rules;
use pretty_assertions::assert_eq;
use tests_integration::TestConfig;

#[derive(Clone, Copy, Debug)]
struct RunCase<'a> {
    parser: Parser,
    rewriter: Rewriter,
    comprehension_expander: QuantifiedExpander,
    solver: SolverFamily,
    case_name: &'a str,
}

fn integration_test(path: &str, essence_base: &str, extension: &str) -> Result<(), Box<dyn Error>> {
    let accept = env::var("ACCEPT").unwrap_or("false".to_string()) == "true";

    if accept {
        clean_test_dir_for_accept(path, essence_base, extension)?;
    }

    let file_config: TestConfig =
        if let Ok(config_contents) = fs::read_to_string(format!("{path}/config.toml")) {
            toml::from_str(&config_contents).unwrap()
        } else {
            Default::default()
        };

    let config = file_config;

    let parsers = config
        .configured_parsers()
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?;
    let rewriters = config
        .configured_rewriters()
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?;
    let comprehension_expanders = config
        .configured_comprehension_expanders()
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?;
    let solvers = config
        .configured_solvers()
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?;
    // Conjure output depends only on the input model, so cache it once per test case.
    let conjure_solutions = if accept {
        Some(Arc::new(get_solutions_from_conjure(
            &format!("{path}/{essence_base}.{extension}"),
            Default::default(),
        )?))
    } else {
        None
    };

    for parser in parsers {
        for rewriter in rewriters.clone() {
            for comprehension_expander in comprehension_expanders.clone() {
                for solver in solvers.clone() {
                    let case_name = run_case_name(parser, rewriter, comprehension_expander);
                    let run_case = RunCase {
                        parser,
                        rewriter,
                        comprehension_expander,
                        solver,
                        case_name: case_name.as_str(),
                    };
                    let file = File::create(format!(
                        "{path}/{}-{}-generated-rule-trace.txt",
                        run_case.case_name,
                        run_case.solver.as_str()
                    ))?;
                    let subscriber = Arc::new(
                        tracing_subscriber::registry().with(
                            fmt::layer()
                                .with_writer(file)
                                .with_level(false)
                                .without_time()
                                .with_target(false)
                                .with_filter(EnvFilter::new("rule_engine_human=trace"))
                                .with_filter(FilterFn::new(|meta| {
                                    meta.target() == "rule_engine_human"
                                })),
                        ),
                    )
                        as Arc<dyn tracing::Subscriber + Send + Sync>;
                    tracing::subscriber::with_default(subscriber, || {
                        integration_test_inner(
                            path,
                            essence_base,
                            extension,
                            run_case,
                            conjure_solutions.clone(),
                            accept,
                        )
                    })?;
                }
            }
        }
    }

    Ok(())
}

/// Runs an integration test for a given Conjure model by:
/// 1. Parsing the model from an Essence file.
/// 2. Rewriting the model according to predefined rule sets.
/// 3. Solving the model using the Minion solver and validating the solutions.
/// 4. Comparing generated rule traces with expected outputs.
///
/// This function operates in multiple stages:
///
/// - **Parsing Stage**
///   - **Stage 1a (Default)**: Reads the Essence model file and verifies that it parses correctly using `parser = "tree-sitter"`.
///   - **Stage 1b (Optional)**: Reads the Essence model file and verifies that it parses correctly using `parser = "via-conjure"`.
///
/// - **Rewrite Stage**
///   - **Stage 2a**: Applies a set of rules to the parsed model and validates the result.
///
/// - **Solution Stage**
///   - **Stage 3a (Default)**: Uses Minion to solve the model and save the solutions.
///   - **Stage 3b (ACCEPT only)**: Compares solutions against Conjure-generated solutions.
///
/// - **Rule Trace Validation Stage**
///   - **Stage 4a (Default)**: Checks that the generated rules match expected traces.
///
/// # Arguments
///
/// * `path` - The file path where the Essence model and other resources are located.
/// * `essence_base` - The base name of the Essence model file.
/// * `extension` - The file extension for the Essence model.
///
/// # Errors
///
/// Returns an error if any stage fails due to a mismatch with expected results or file I/O issues.
#[allow(clippy::unwrap_used)]
fn integration_test_inner(
    path: &str,
    essence_base: &str,
    extension: &str,
    run_case: RunCase<'_>,
    conjure_solutions: Option<Arc<Vec<BTreeMap<Name, Literal>>>>,
    accept: bool,
) -> Result<(), Box<dyn Error>> {
    let parser = run_case.parser;
    let rewriter = run_case.rewriter;
    let comprehension_expander = run_case.comprehension_expander;
    let solver_fam = run_case.solver;
    let case_name = run_case.case_name;

    let context: Arc<RwLock<Context<'static>>> = Default::default();

    set_current_parser(parser);
    set_current_rewriter(rewriter);
    set_comprehension_expander(comprehension_expander);
    set_current_solver_family(solver_fam);

    // File path
    let file_path = format!("{path}/{essence_base}.{extension}");

    // Stage 1a/1b: Parse the model using the selected parser.
    let parsed_model = match parser {
        Parser::TreeSitter => {
            let mut ctx = context.as_ref().write().unwrap();
            ctx.file_name = Some(format!("{path}/{essence_base}.{extension}"));
            parse_essence_file_native(&file_path, context.clone())?
        }
        Parser::ViaConjure => parse_essence_file(&file_path, context.clone())?,
    };
    // Stage 2a: Rewrite the model using the rule engine
    let mut extra_rules = vec![];

    if let SolverFamily::Sat(sat_encoding) = solver_fam {
        extra_rules.push(sat_encoding.as_rule_set());
    }

    let mut rules_to_load = DEFAULT_RULE_SETS.to_vec();
    rules_to_load.extend(extra_rules);

    let rule_sets = resolve_rule_sets(solver_fam, &rules_to_load)?;

    let mut model = parsed_model;

    let rewritten_model = match rewriter {
        Rewriter::Naive => rewrite_naive(&model, &rule_sets, false)?,
        Rewriter::Morph => {
            let submodel = model.as_submodel_mut();
            let rules_grouped = get_rules_grouped(&rule_sets)
                .unwrap_or_else(|_| bug!("get_rule_priorities() failed!"))
                .into_iter()
                .map(|(_, rule)| rule.into_iter().map(|f| f.rule).collect_vec())
                .collect_vec();

            let engine = EngineBuilder::new()
                .set_selector(select_panic)
                .append_rule_groups(rules_grouped)
                .build();
            let (expr, symbol_table) =
                engine.morph(submodel.root().clone(), submodel.symbols().clone());

            *submodel.symbols_mut() = symbol_table;
            submodel.replace_root(expr);
            model.clone()
        }
    };
    let solver_input_file = None;

    let solver = match solver_fam {
        SolverFamily::Minion => Solver::new(Minion::default()),
        SolverFamily::Sat(_) => Solver::new(Sat::default()),
        #[cfg(feature = "smt")]
        SolverFamily::Smt(_) => Solver::new(Smt::default()),
    };

    let solutions = {
        let solved = get_solutions(solver, rewritten_model, 0, &solver_input_file)?;
        save_solutions_json(&solved, path, case_name, solver_fam)?;
        solved
    };

    // Stage 3b: Check solutions against Conjure when ACCEPT=true
    if accept {
        let conjure_solutions = conjure_solutions
            .as_deref()
            .expect("conjure solutions should be cached when ACCEPT=true");

        let username_solutions = normalize_solutions_for_comparison(&solutions);
        let conjure_solutions = normalize_solutions_for_comparison(conjure_solutions);

        let mut conjure_solutions_json = solutions_to_json(&conjure_solutions);
        let mut username_solutions_json = solutions_to_json(&username_solutions);

        conjure_solutions_json.sort_all_objects();
        username_solutions_json.sort_all_objects();

        assert_eq!(
            username_solutions_json, conjure_solutions_json,
            "Solutions (<) do not match conjure (>)!"
        );
    }

    // When ACCEPT=true, copy all generated files to expected
    if accept {
        // Always overwrite these ones. Unlike the rest, we don't need to selectively do these
        // based on the test results, so they don't get done later.

        copy_generated_to_expected(path, case_name, "solutions", "json", Some(solver_fam))?;

        copy_human_trace_generated_to_expected(path, case_name, solver_fam)?;
    }

    // Check Stage 3a (solutions)
    match solver_fam {
        SolverFamily::Minion => {
            let expected_solutions_json =
                read_solutions_json(path, case_name, "expected", SolverFamily::Minion)?;
            let username_solutions_json = solutions_to_json(&solutions);
            assert_eq!(username_solutions_json, expected_solutions_json);
        }
        SolverFamily::Sat(_) => {
            let expected_solutions_json = read_solutions_json(
                path,
                case_name,
                "expected",
                SolverFamily::Sat(Default::default()),
            )?;
            let username_solutions_json = solutions_to_json(&solutions);
            assert_eq!(username_solutions_json, expected_solutions_json);
        }
        #[cfg(feature = "smt")]
        SolverFamily::Smt(_) => {
            let expected_solutions_json = read_solutions_json(
                path,
                case_name,
                "expected",
                SolverFamily::Smt(TheoryConfig::default()),
            )?;
            let username_solutions_json = solutions_to_json(&solutions);
            assert_eq!(username_solutions_json, expected_solutions_json);
        }
    }

    // TODO: Implement rule trace validation for morph
    match rewriter {
        Rewriter::Morph => {}
        Rewriter::Naive => {
            let generated = read_human_rule_trace(path, case_name, "generated", &solver_fam)?;
            let expected = read_human_rule_trace(path, case_name, "expected", &solver_fam)?;

            assert_eq!(
                expected, generated,
                "Generated rule trace does not match the expected trace!"
            );
        }
    }

    save_stats_json(context, path, case_name, solver_fam)?;

    Ok(())
}

fn run_case_name(
    parser: Parser,
    rewriter: Rewriter,
    comprehension_expander: QuantifiedExpander,
) -> String {
    format!("{parser}-{rewriter}-{comprehension_expander}")
}

fn clean_test_dir_for_accept(
    path: &str,
    essence_base: &str,
    extension: &str,
) -> Result<(), std::io::Error> {
    let input_filename = format!("{essence_base}.{extension}");

    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let entry_path = entry.path();

        if file_name == input_filename || file_name == "config.toml" {
            continue;
        }

        if entry_path.is_dir() {
            std::fs::remove_dir_all(entry_path)?;
        } else {
            std::fs::remove_file(entry_path)?;
        }
    }

    Ok(())
}

fn copy_human_trace_generated_to_expected(
    path: &str,
    test_name: &str,
    solver: SolverFamily,
) -> Result<(), std::io::Error> {
    let solver_name = solver.as_str();
    std::fs::copy(
        format!("{path}/{test_name}-{solver_name}-generated-rule-trace.txt"),
        format!("{path}/{test_name}-{solver_name}-expected-rule-trace.txt"),
    )?;
    Ok(())
}

fn copy_generated_to_expected(
    path: &str,
    test_name: &str,
    stage: &str,
    extension: &str,
    solver: Option<SolverFamily>,
) -> Result<(), std::io::Error> {
    let marker = solver.map_or("agnostic", |s| s.as_str());

    std::fs::copy(
        format!("{path}/{test_name}-{marker}.generated-{stage}.{extension}"),
        format!("{path}/{test_name}-{marker}.expected-{stage}.{extension}"),
    )?;
    Ok(())
}

#[test]
fn assert_conjure_present() {
    conjure_cp_cli::find_conjure::conjure_executable().unwrap();
}

include!(concat!(env!("OUT_DIR"), "/gen_tests.rs"));
