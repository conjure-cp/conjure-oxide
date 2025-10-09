#![allow(clippy::expect_used)]
use conjure_cp::ast::SymbolTable;
use conjure_cp::bug;
use conjure_cp::rule_engine::get_rules_grouped;

use conjure_cp::defaults::DEFAULT_RULE_SETS;
use conjure_cp::parse::tree_sitter::parse_essence_file_native;
use conjure_cp::rule_engine::rewrite_naive;
use conjure_cp_cli::utils::testing::{normalize_solutions_for_comparison, read_human_rule_trace};
use glob::glob;
use itertools::Itertools;
use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::fs;
use std::fs::File;
use tracing::{Level, Metadata as OtherMetadata, span};
use tracing_subscriber::{
    Layer, Registry, filter::EnvFilter, filter::FilterFn, fmt, layer::SubscriberExt,
};
use tree_morph::{helpers::select_panic, prelude::*};

use uniplate::Biplate;

use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;

use conjure_cp::ast::Atom;
use conjure_cp::ast::{Expression, Literal, Name};
use conjure_cp::context::Context;
use conjure_cp::parse::tree_sitter::parse_essence_file;
use conjure_cp::rule_engine::resolve_rule_sets;
use conjure_cp::solver::SolverFamily;
use conjure_cp_cli::utils::conjure::solutions_to_json;
use conjure_cp_cli::utils::conjure::{
    get_minion_solutions, get_sat_solutions, get_solutions_from_conjure,
};
use conjure_cp_cli::utils::testing::save_stats_json;
use conjure_cp_cli::utils::testing::{
    read_model_json, read_solutions_json, save_model_json, save_solutions_json,
};
#[allow(clippy::single_component_path_imports, unused_imports)]
use conjure_cp_rules;
use pretty_assertions::assert_eq;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(default)]
#[serde(deny_unknown_fields)]
struct TestConfig {
    extra_rewriter_asserts: Vec<String>,

    parse_model_default: bool, // Stage 1a: Reads and verifies the Essence model file
    enable_native_parser: bool, // Stage 1b: Runs the native parser if enabled
    apply_rewrite_rules: bool, // Stage 2a: Applies predefined rules to the model
    enable_extra_validation: bool, // Stage 2b: Runs additional validation checks
    solve_with_minion: bool,   // Stage 3a: Solves the model using Minion
    solve_with_sat: bool,      // TODO - add stage mark
    compare_solver_solutions: bool, // Stage 3b: Compares Minion and Conjure solutions
    validate_rule_traces: bool, // Stage 4a: Checks rule traces against expected outputs

    enable_morph_impl: bool,
    enable_naive_impl: bool,
    enable_rewriter_impl: bool,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            extra_rewriter_asserts: vec!["vector_operators_have_partially_evaluated".into()],
            enable_naive_impl: true,
            solve_with_sat: false,
            enable_morph_impl: false,
            enable_rewriter_impl: true,
            parse_model_default: true,
            enable_native_parser: true,
            apply_rewrite_rules: true,
            enable_extra_validation: false,
            solve_with_minion: true,
            compare_solver_solutions: false,
            validate_rule_traces: true,
        }
    }
}

fn env_var_override_bool(key: &str, default: bool) -> bool {
    env::var(key).ok().map(|s| s == "true").unwrap_or(default)
}
impl TestConfig {
    fn merge_env(self) -> Self {
        Self {
            parse_model_default: env_var_override_bool(
                "PARSE_MODEL_DEFAULT",
                self.parse_model_default,
            ),
            enable_morph_impl: env_var_override_bool("ENABLE_MORPH_IMPL", self.enable_morph_impl),
            enable_naive_impl: env_var_override_bool("ENABLE_NAIVE_IMPL", self.enable_naive_impl),
            enable_native_parser: env_var_override_bool(
                "ENABLE_NATIVE_PARSER",
                self.enable_native_parser,
            ),
            apply_rewrite_rules: env_var_override_bool(
                "APPLY_REWRITE_RULES",
                self.apply_rewrite_rules,
            ),
            enable_extra_validation: env_var_override_bool(
                "ENABLE_EXTRA_VALIDATION",
                self.enable_extra_validation,
            ),
            solve_with_minion: env_var_override_bool("SOLVE_WITH_MINION", self.solve_with_minion),
            solve_with_sat: env_var_override_bool("SOLVE_WITH_SAT", self.solve_with_sat),
            compare_solver_solutions: env_var_override_bool(
                "COMPARE_SOLVER_SOLUTIONS",
                self.compare_solver_solutions,
            ),
            validate_rule_traces: env_var_override_bool(
                "VALIDATE_RULE_TRACES",
                self.validate_rule_traces,
            ),
            enable_rewriter_impl: env_var_override_bool(
                "ENABLE_REWRITER_IMPL",
                self.enable_rewriter_impl,
            ),
            extra_rewriter_asserts: self.extra_rewriter_asserts, // Not overridden by env vars
        }
    }
}

fn main() {
    let _guard = create_scoped_subscriber("./logs", "test_log");

    // creating a span and log a message
    let test_span = span!(Level::TRACE, "test_span");
    let _enter: span::Entered<'_> = test_span.enter();

    for entry in glob("conjure_cp_cli/tests/integration/*").expect("Failed to read glob pattern") {
        match entry {
            Ok(path) => println!("File: {path:?}"),
            Err(e) => println!("Error: {e:?}"),
        }
    }

    let file_path = Path::new("conjure_cp_cli/tests/integration/*"); // using relative path

    let base_name = file_path.file_stem().and_then(|stem| stem.to_str());

    match base_name {
        Some(name) => println!("Base name: {name}"),
        None => println!("Could not extract the base name"),
    }
}

// run tests in sequence not parallel when verbose logging, to ensure the logs are ordered
// correctly
static GUARD: Mutex<()> = Mutex::new(());

// wrapper to conditionally enforce sequential execution
fn integration_test(path: &str, essence_base: &str, extension: &str) -> Result<(), Box<dyn Error>> {
    let verbose = env::var("VERBOSE").unwrap_or("false".to_string()) == "true";
    let accept = env::var("ACCEPT").unwrap_or("false".to_string()) == "true";

    let subscriber = create_scoped_subscriber(path, essence_base);
    // run tests in sequence not parallel when verbose logging, to ensure the logs are ordered
    // correctly
    //
    // also with ACCEPT=true, as the conjure checking seems to get confused when ran too much at
    // once.
    if verbose || accept {
        let _guard = GUARD.lock().unwrap_or_else(|e| e.into_inner());

        // set the subscriber as default
        tracing::subscriber::with_default(subscriber, || {
            integration_test_inner(path, essence_base, extension)
        })
    } else {
        let subscriber = create_scoped_subscriber(path, essence_base);
        tracing::subscriber::with_default(subscriber, || {
            integration_test_inner(path, essence_base, extension)
        })
    }
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
///   - **Stage 1a (Default)**: Reads the Essence model file and verifies that it parses correctly.
///   - **Stage 1b (Optional)**: Runs the native parser if explicitly enabled.
///
/// - **Rewrite Stage**
///   - **Stage 2a (Default)**: Applies a set of rules to the parsed model and validates the result.
///   - **Stage 2b (Optional)**: Runs additional validation checks on the rewritten model if enabled.
///
/// - **Solution Stage**
///   - **Stage 3a (Default)**: Uses Minion to solve the model and save the solutions.
///   - **Stage 3b (Optional)**: Compares the Minion solutions against Conjure-generated solutions if enabled.
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
) -> Result<(), Box<dyn Error>> {
    let context: Arc<RwLock<Context<'static>>> = Default::default();
    let accept = env::var("ACCEPT").unwrap_or("false".to_string()) == "true";
    let verbose = env::var("VERBOSE").unwrap_or("false".to_string()) == "true";

    // When running accept=true, only regenerate the expected files for these tests if the test
    // fails.
    //
    // This reduces unnecessary git diffs when only the id of items in a model changes. These
    // change every run of the tester, but do not change the correctness of the model.
    let mut parsed_model_dirty = false;
    let mut parsed_native_model_dirty = false;
    let mut rewritten_model_dirty = false;

    if verbose {
        println!("Running integration test for {path}/{essence_base}, ACCEPT={accept}");
    }

    let file_config: TestConfig =
        if let Ok(config_contents) = fs::read_to_string(format!("{path}/config.toml")) {
            toml::from_str(&config_contents).unwrap()
        } else {
            Default::default()
        };

    let config = file_config.merge_env();

    // TODO: allow either Minion or SAT but not both; eventually allow both sovlers to be tested

    if config.solve_with_sat && config.solve_with_minion {
        todo!("Not yet implemented simultaneous testing of both solvers")
    }

    // File path
    let file_path = format!("{path}/{essence_base}.{extension}");

    // Stage 1a: Parse the model using the normal parser (run unless explicitly disabled)
    let parsed_model = if config.parse_model_default {
        let parsed = parse_essence_file(&file_path, context.clone())?;
        if verbose {
            println!("Parsed model: {parsed:#?}");
        }
        save_model_json(&parsed, path, essence_base, "parse")?;
        Some(parsed)
    } else {
        None
    };

    // Stage 1b: Run native parser (only if explicitly enabled)
    let mut model_native = None;
    if config.enable_native_parser {
        let mn = parse_essence_file_native(&file_path, context.clone())?;
        save_model_json(&mn, path, essence_base, "parse")?;
        model_native = Some(mn);

        {
            let mut ctx = context.as_ref().write().unwrap();
            ctx.file_name = Some(format!("{path}/{essence_base}.{extension}"));
        }
    }

    // Stage 2a: Rewrite the model using the rule engine (run unless explicitly disabled)
    let rewritten_model = if config.apply_rewrite_rules {
        // rule set selection based on solver

        let solver_fam = if config.solve_with_sat {
            SolverFamily::Sat
        } else {
            SolverFamily::Minion
        };

        let rule_sets = resolve_rule_sets(solver_fam, DEFAULT_RULE_SETS)?;

        let mut model = parsed_model.expect("Model must be parsed in 1a");

        let rewritten = if config.enable_naive_impl {
            rewrite_naive(&model, &rule_sets, false, false)?
        } else if config.enable_morph_impl {
            let submodel = model.as_submodel_mut();
            let rules_grouped = get_rules_grouped(&rule_sets)
                .unwrap_or_else(|_| bug!("get_rule_priorities() failed!"))
                .into_iter()
                .map(|(_, rule)| rule.into_iter().map(|f| f.rule).collect_vec())
                .collect_vec();

            let (expr, symbol_table): (Expression, SymbolTable) = morph(
                rules_grouped,
                select_panic,
                submodel.root().clone(),
                submodel.symbols().clone(),
            );

            *submodel.symbols_mut() = symbol_table;
            submodel.replace_root(expr);
            model.clone()
        } else {
            panic!("No rewriter implementation specified")
        };
        if verbose {
            println!("Rewritten model: {rewritten:#?}");
        }

        save_model_json(&rewritten, path, essence_base, "rewrite")?;
        Some(rewritten)
    } else {
        None
    };

    // Stage 2b: Check model properties (extra_asserts) (Verify additional model properties)
    // (e.g., ensure vector operators are evaluated). (only if explicitly enabled)
    if config.enable_extra_validation {
        for extra_assert in config.extra_rewriter_asserts.clone() {
            match extra_assert.as_str() {
                "vector_operators_have_partially_evaluated" => {
                    assert_vector_operators_have_partially_evaluated(
                        rewritten_model.as_ref().expect("Rewritten model required"),
                    );
                }
                x => println!("Unrecognised extra assert: {x}"),
            };
        }
    }

    // Stage 3a: Run the model through the Minion solver (run unless explicitly disabled)
    let solutions = if config.solve_with_minion {
        let solved = get_minion_solutions(
            rewritten_model
                .as_ref()
                .expect("Rewritten model must be present in 2a")
                .clone(),
            0,
            &None,
        )?;
        let solutions_json =
            save_solutions_json(&solved, path, essence_base, SolverFamily::Minion)?;
        if verbose {
            println!("Minion solutions: {solutions_json:#?}");
        }
        Some(solved)
    } else if config.solve_with_sat {
        let solved = get_sat_solutions(
            rewritten_model
                .as_ref()
                .expect("Rewritten model must be present in 2a")
                .clone(),
            0,
            &None,
        )?;
        let solutions_json = save_solutions_json(&solved, path, essence_base, SolverFamily::Sat)?;
        if verbose {
            println!("Minion solutions: {solutions_json:#?}");
        }
        Some(solved)
    } else {
        None
    };

    // Stage 3b: Check solutions against Conjure (only if explicitly enabled)
    if config.compare_solver_solutions
        || accept && (config.solve_with_minion || config.solve_with_sat)
    {
        let conjure_solutions: Vec<BTreeMap<Name, Literal>> = get_solutions_from_conjure(
            &format!("{path}/{essence_base}.{extension}"),
            Arc::clone(&context),
        )?;

        let username_solutions = normalize_solutions_for_comparison(
            solutions.as_ref().expect("Minion solutions required"),
        );
        let conjure_solutions = normalize_solutions_for_comparison(&conjure_solutions);

        let mut conjure_solutions_json = solutions_to_json(&conjure_solutions);
        let mut username_solutions_json = solutions_to_json(&username_solutions);

        conjure_solutions_json.sort_all_objects();
        username_solutions_json.sort_all_objects();

        assert_eq!(
            username_solutions_json, conjure_solutions_json,
            "Solutions (<) do not match conjure (>)!"
        );
    }

    // Before testing against the generated tests, if the generated tests don't exist, make them.
    //
    // We rewrite out-of-date tests (if necessary) after testing them.
    if accept {
        // Overwrite expected parse and rewrite models if enabled
        if config.enable_native_parser
            && !expected_exists_for(path, essence_base, "parse", "serialised.json")
        {
            model_native.clone().expect("model_native should exist");
            copy_generated_to_expected(path, essence_base, "parse", "serialised.json")?;
        }
        if config.parse_model_default
            && !expected_exists_for(path, essence_base, "parse", "serialised.json")
        {
            copy_generated_to_expected(path, essence_base, "parse", "serialised.json")?;
        }
        if config.apply_rewrite_rules
            && !expected_exists_for(path, essence_base, "rewrite", "serialised.json")
        {
            copy_generated_to_expected(path, essence_base, "rewrite", "serialised.json")?;
        }

        // Always overwrite these ones. Unlike the rest, we don't need to selectively do these
        // based on the test results, so they don't get done later.
        if config.solve_with_minion {
            copy_generated_to_expected(path, essence_base, "minion", "solutions.json")?;
        } else if config.solve_with_sat {
            copy_generated_to_expected(path, essence_base, "sat", "solutions.json")?;
        }

        if config.validate_rule_traces {
            copy_human_trace_generated_to_expected(path, essence_base)?;
            save_stats_json(context.clone(), path, essence_base)?;
        }
    }

    // Check Stage 1b (native parser)
    if config.enable_native_parser {
        let expected_model = read_model_json(&context, path, essence_base, "expected", "parse");

        // A JSON reading error could just mean that the ast has changed since the file was
        // generated.
        //
        // When ACCEPT=true, regenerate the json instead of failing the test.
        match expected_model {
            Err(_) if accept => {
                parsed_native_model_dirty = true;
            }
            Err(e) => {
                return Err(Box::new(e));
            }
            Ok(expected_model) => {
                let model_native = model_native
                    .clone()
                    .expect("model_native should exist here");
                if accept {
                    parsed_native_model_dirty = model_native != expected_model;
                } else {
                    assert_eq!(model_native, expected_model);
                }
            }
        }
    }

    // Check Stage 1a (parsed model)
    if config.parse_model_default {
        let expected_model = read_model_json(&context, path, essence_base, "expected", "parse");
        let model_from_file = read_model_json(&context, path, essence_base, "generated", "parse");

        // A JSON reading error could just mean that the ast has changed since the file was
        // generated.
        //
        // When ACCEPT=true, regenerate the json instead of failing the test.
        match (expected_model, model_from_file) {
            (Err(_), _) | (_, Err(_)) if accept => {
                parsed_model_dirty = true;
            }

            (Err(e), _) => {
                return Err(Box::new(e));
            }

            (_, Err(e)) => {
                return Err(Box::new(e));
            }

            (Ok(expected_model), Ok(model_from_file)) if accept => {
                parsed_model_dirty = model_from_file != expected_model;
            }

            (Ok(expected_model), Ok(model_from_file)) => {
                assert_eq!(model_from_file, expected_model);
            }
        }
    }

    // Check Stage 2a (rewritten model)
    if config.apply_rewrite_rules {
        let expected_model = read_model_json(&context, path, essence_base, "expected", "rewrite");
        // A JSON reading error could just mean that the ast has changed since the file was
        // generated.
        //
        // When ACCEPT=true, regenerate the json instead of failing the test.
        match expected_model {
            Err(_) if accept => {
                rewritten_model_dirty = true;
            }
            Err(e) => {
                return Err(Box::new(e));
            }
            Ok(expected_model) => {
                let rewritten_model =
                    rewritten_model.expect("Rewritten model must be present in 2a");

                if accept {
                    rewritten_model_dirty = rewritten_model != expected_model;
                } else {
                    assert_eq!(rewritten_model, expected_model);
                }
            }
        }
    }

    // Check Stage 3a (solutions)
    if config.solve_with_minion {
        let expected_solutions_json =
            read_solutions_json(path, essence_base, "expected", SolverFamily::Minion)?;
        let username_solutions_json = solutions_to_json(solutions.as_ref().unwrap_or(&vec![]));
        assert_eq!(username_solutions_json, expected_solutions_json);
    } else if config.solve_with_sat {
        let expected_solutions_json =
            read_solutions_json(path, essence_base, "expected", SolverFamily::Sat)?;
        let username_solutions_json = solutions_to_json(solutions.as_ref().unwrap_or(&vec![]));
        assert_eq!(username_solutions_json, expected_solutions_json);
    }

    // Stage 4a: Check that the generated rules trace matches the expected.
    // We don't check rule trace when morph is enabled.
    // TODO: Implement rule trace validation for morph
    if config.validate_rule_traces && !config.enable_morph_impl {
        let generated = read_human_rule_trace(path, essence_base, "generated")?;
        let expected = read_human_rule_trace(path, essence_base, "expected")?;

        assert_eq!(
            expected, generated,
            "Generated rule trace does not match the expected trace!"
        );
    };

    if accept {
        // Overwrite expected parse and rewrite models if needed
        if config.enable_native_parser && parsed_native_model_dirty {
            model_native.expect("model_native should exist");
            copy_generated_to_expected(path, essence_base, "parse", "serialised.json")?;
        }
        if config.parse_model_default && parsed_model_dirty {
            copy_generated_to_expected(path, essence_base, "parse", "serialised.json")?;
        }
        if config.apply_rewrite_rules && rewritten_model_dirty {
            copy_generated_to_expected(path, essence_base, "rewrite", "serialised.json")?;
        }
    }
    save_stats_json(context, path, essence_base)?;

    Ok(())
}

fn copy_human_trace_generated_to_expected(
    path: &str,
    test_name: &str,
) -> Result<(), std::io::Error> {
    std::fs::copy(
        format!("{path}/{test_name}-generated-rule-trace-human.txt"),
        format!("{path}/{test_name}-expected-rule-trace-human.txt"),
    )?;
    Ok(())
}

fn copy_generated_to_expected(
    path: &str,
    test_name: &str,
    stage: &str,
    extension: &str,
) -> Result<(), std::io::Error> {
    std::fs::copy(
        format!("{path}/{test_name}.generated-{stage}.{extension}"),
        format!("{path}/{test_name}.expected-{stage}.{extension}"),
    )?;
    Ok(())
}

fn expected_exists_for(path: &str, test_name: &str, stage: &str, extension: &str) -> bool {
    Path::new(&format!("{path}/{test_name}.expected-{stage}.{extension}")).exists()
}

fn assert_vector_operators_have_partially_evaluated(model: &conjure_cp::Model) {
    for node in model.universe_bi() {
        use conjure_cp::ast::Expression::*;
        match node {
            Sum(_, ref vec) => assert_constants_leq_one_vec_lit(&node, vec),
            Min(_, ref vec) => assert_constants_leq_one_vec_lit(&node, vec),
            Max(_, ref vec) => assert_constants_leq_one_vec_lit(&node, vec),
            Or(_, ref vec) => assert_constants_leq_one_vec_lit(&node, vec),
            And(_, ref vec) => assert_constants_leq_one_vec_lit(&node, vec),
            _ => (),
        };
    }
}

fn assert_constants_leq_one_vec_lit(parent_expr: &Expression, expr: &Expression) {
    if let Some(exprs) = expr.clone().unwrap_list() {
        assert_constants_leq_one(parent_expr, &exprs);
    };
}

fn assert_constants_leq_one(parent_expr: &Expression, exprs: &[Expression]) {
    let count = exprs.iter().fold(0, |i, x| match x {
        Expression::Atomic(_, Atom::Literal(_)) => i + 1,
        _ => i,
    });

    assert!(
        count <= 1,
        "assert_vector_operators_have_partially_evaluated: expression {parent_expr} is not partially evaluated"
    );
}

pub fn create_scoped_subscriber(
    path: &str,
    test_name: &str,
) -> (impl tracing::Subscriber + Send + Sync) {
    let target1_layer = create_file_layer_json(path, test_name);
    let target2_layer = create_file_layer_human(path, test_name);
    let layered = target1_layer.and_then(target2_layer);

    let subscriber = Arc::new(tracing_subscriber::registry().with(layered))
        as Arc<dyn tracing::Subscriber + Send + Sync>;
    // setting this subscriber as the default
    let _default = tracing::subscriber::set_default(subscriber.clone());

    subscriber
}

fn create_file_layer_json(path: &str, test_name: &str) -> impl Layer<Registry> + Send + Sync {
    let file = File::create(format!("{path}/{test_name}-generated-rule-trace.json"))
        .expect("Unable to create log file");

    fmt::layer()
        .with_writer(file)
        .with_level(false)
        .with_target(false)
        .without_time()
        .with_filter(FilterFn::new(|meta: &OtherMetadata| {
            meta.target() == "rule_engine"
        }))
}

fn create_file_layer_human(path: &str, test_name: &str) -> (impl Layer<Registry> + Send + Sync) {
    let file = File::create(format!("{path}/{test_name}-generated-rule-trace-human.txt"))
        .expect("Unable to create log file");

    fmt::layer()
        .with_writer(file)
        .with_level(false)
        .without_time()
        .with_target(false)
        .with_filter(EnvFilter::new("rule_engine_human=trace"))
        .with_filter(FilterFn::new(|meta| meta.target() == "rule_engine_human"))
}

#[test]
fn assert_conjure_present() {
    conjure_cp_cli::find_conjure::conjure_executable().unwrap();
}

include!(concat!(env!("OUT_DIR"), "/gen_tests.rs"));
