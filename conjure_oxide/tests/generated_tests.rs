#![allow(clippy::expect_used)]

use conjure_core::rule_engine::rewrite_model;
use conjure_core::rule_engine::rewrite_naive;
use conjure_core::Model;
use conjure_oxide::utils::essence_parser::parse_essence_file_native;
use conjure_oxide::utils::testing::{read_human_rule_trace, read_rule_trace};
use glob::glob;
use itertools::Itertools;
use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::fs;
use std::fs::File;
use tracing::{span, Level, Metadata as OtherMetadata};
use tracing_subscriber::{
    filter::EnvFilter, filter::FilterFn, fmt, layer::SubscriberExt, Layer, Registry,
};

use uniplate::Biplate;

use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;

use conjure_core::ast::Atom;
use conjure_core::ast::{Expression, Literal, Name};
use conjure_core::context::Context;
use conjure_oxide::defaults::get_default_rule_sets;
use conjure_oxide::rule_engine::resolve_rule_sets;
use conjure_oxide::utils::conjure::minion_solutions_to_json;
use conjure_oxide::utils::conjure::{
    get_minion_solutions, get_solutions_from_conjure, parse_essence_file,
};
use conjure_oxide::utils::testing::save_stats_json;
use conjure_oxide::utils::testing::{
    read_minion_solutions_json, read_model_json, save_minion_solutions_json, save_model_json,
};
use conjure_oxide::SolverFamily;
use pretty_assertions::assert_eq;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(default)]
struct TestConfig {
    extra_rewriter_asserts: Vec<String>,
    use_native_parser: bool,
    use_naive_rewriter: bool,
    run_solver: bool,

    disable_1a: bool,
    enable_1b: bool,
    disable_2a: bool,
    enable_2b: bool,
    disable_3a: bool,
    enable_3b: bool,
    disable_4a: bool,

    enable_native_impl: bool,
    enable_rewriter_impl: bool,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            extra_rewriter_asserts: vec!["vector_operators_have_partially_evaluated".into()],
            use_native_parser: true,
            use_naive_rewriter: true,
            enable_native_impl: true,
            enable_rewriter_impl: true,
            run_solver: true,
            disable_1a: false,
            enable_1b: true,
            disable_2a: false,
            enable_2b: true,
            disable_3a: false,
            enable_3b: true,
            disable_4a: false,
        }
    }
}

fn main() {
    let _guard = create_scoped_subscriber("./logs", "test_log");

    // creating a span and log a message
    let test_span = span!(Level::TRACE, "test_span");
    let _enter: span::Entered<'_> = test_span.enter();

    for entry in glob("conjure_oxide/tests/integration/*").expect("Failed to read glob pattern") {
        match entry {
            Ok(path) => println!("File: {:?}", path),
            Err(e) => println!("Error: {:?}", e),
        }
    }

    let file_path = Path::new("conjure_oxide/tests/integration/*"); // using relative path

    let base_name = file_path.file_stem().and_then(|stem| stem.to_str());

    match base_name {
        Some(name) => println!("Base name: {}", name),
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

    if verbose {
        println!(
            "Running integration test for {}/{}, ACCEPT={}",
            path, essence_base, accept
        );
    }

    let config: TestConfig =
        if let Ok(config_contents) = fs::read_to_string(format!("{}/config.toml", path)) {
            toml::from_str(&config_contents).unwrap()
        } else {
            Default::default()
        };

    // Stage 1a: Parse the model using the normal parser (run unless explicitly disabled)
    let model = if !config.disable_1a.unwrap_or(false) {
        let parsed = parse_essence_file(path, essence_base, extension, context.clone())?;
        if verbose {
            println!("Parsed model: {:#?}", parsed);
        }
        save_model_json(&parsed, path, essence_base, "parse")?;
        Some(parsed)
    } else {
        None
    };

    // Stage 1b: Run native parser (only if explicitly enabled)
    let mut model_native = None;
    if config.enable_1b {
        let mn = parse_essence_file_native(path, essence_base, extension, context.clone())?;
        save_model_json(&mn, path, essence_base, "parse")?;
        model_native = Some(mn);

        {
            let mut ctx = context.as_ref().write().unwrap();
            ctx.file_name = Some(format!("{path}/{essence_base}.{extension}"));
        }
    }

    // Stage 2a: Rewrite the model using the rule engine (run unless explicitly disabled)
    let rewritten_model = if !config.disable_2a {
        let rule_sets = resolve_rule_sets(SolverFamily::Minion, &get_default_rule_sets())?;

        let rewritten = if config.enable_native_impl.unwrap_or(false) {
            rewrite_model(
                model.as_ref().expect("Model must be parsed in 1a"),
                &rule_sets,
            )?
        } else {
            rewrite_naive(
                model.as_ref().expect("Model must be parsed in 1a"),
                &rule_sets,
                false,
            )?
        };

        if verbose {
            println!("Rewritten model: {:#?}", rewritten);
        }

        save_model_json(&rewritten, path, essence_base, "rewrite")?;
        Some(rewritten)
    } else {
        None
    };

    // Stage 2b: Check model properties (extra_asserts) (Verify additional model properties
    // (e.g., ensure vector operators are evaluated). (only if explicitly enabled)
    if config.enable_2b {
        if let Some(extra_asserts) = config.extra_rewriter_asserts.clone() {
            for extra_assert in extra_asserts {
                match extra_assert.as_str() {
                    "vector_operators_have_partially_evaluated" => {
                        assert_vector_operators_have_partially_evaluated(
                            rewritten_model.as_ref().expect("Rewritten model required"),
                        );
                    }
                    x => println!("Unrecognised extra assert: {}", x),
                };
            }
        }
    }

    // Stage 3a: Run the model through the Minion solver (run unless explicitly disabled)
    let solutions = if !config.disable_3a {
        let solved = get_minion_solutions(
            rewritten_model
                .as_ref()
                .expect("Rewritten model must be present in 2a")
                .clone(),
            0,
        )?;
        let solutions_json = save_minion_solutions_json(&solved, path, essence_base)?;
        if verbose {
            println!("Minion solutions: {:#?}", solutions_json);
        }
        Some(solved)
    } else {
        None
    };

    // Stage 3b: Check solutions against Conjure (only if explicitly enabled)
    if config.enable_3b.unwrap_or(false) {
        let conjure_solutions: Vec<BTreeMap<Name, Literal>> =
            get_solutions_from_conjure(&format!("{}/{}.{}", path, essence_base, extension))?;

        let username_solutions = normalize_solutions_for_comparison(
            solutions.as_ref().expect("Minion solutions required"),
        );
        let conjure_solutions = normalize_solutions_for_comparison(&conjure_solutions);

        let mut conjure_solutions_json = minion_solutions_to_json(&conjure_solutions);
        let mut username_solutions_json = minion_solutions_to_json(&username_solutions);

        conjure_solutions_json.sort_all_objects();
        username_solutions_json.sort_all_objects();

        assert_eq!(
            username_solutions_json, conjure_solutions_json,
            "Solutions do not match conjure!"
        );
    }

    // Stage 4a: Check that the generated rules match expected traces (run unless explicitly disabled)
    let (generated_rule_trace_human, expected_rule_trace_human) =
        if !config.disable_4a.unwrap_or(false) {
            let generated = read_human_rule_trace(path, essence_base, "generated")?;
            let expected = read_human_rule_trace(path, essence_base, "expected")?;

            // Perform the assertion immediately
            assert_eq!(
                expected, generated,
                "Generated rule trace does not match the expected trace!"
            );

            (Some(generated), Some(expected))
        } else {
            (None, None) // Avoid uninitialized variables when 4a is disabled
        };

    if accept {
        // Overwrite expected parse and rewrite models if enabled
        if config.enable_1b {
            model_native.clone().expect("model_native should exist");
            copy_generated_to_expected(path, essence_base, "parse", "serialised.json")?;
        }
        if !config.disable_1a {
            copy_generated_to_expected(path, essence_base, "parse", "serialised.json")?;
        }
        if !config.disable_2a {
            copy_generated_to_expected(path, essence_base, "rewrite", "serialised.json")?;
        }

        if !config.disable_3a {
            copy_generated_to_expected(path, essence_base, "minion", "solutions.json")?;
        }

        if !config.disable_4a {
            copy_human_trace_generated_to_expected(path, essence_base)?;
            save_stats_json(context.clone(), path, essence_base)?;
        }
    }

    // Check Stage 1b (native parser)
    if config.enable_1b {
        let expected_model = read_model_json(path, essence_base, "expected", "parse")?;
        let model_native = model_native.expect("model_native should exist here");
        assert_eq!(model_native, expected_model);
    }

    // Check Stage 1a (parsed model)
    if !config.disable_1a {
        let expected_model = read_model_json(path, essence_base, "expected", "parse")?;
        assert_eq!(model.expect("Model must be present in 1a"), expected_model);
    }

    // Check Stage 2a (rewritten model)
    if !config.disable_2a {
        let expected_model = read_model_json(path, essence_base, "expected", "rewrite")?;
        assert_eq!(
            rewritten_model.expect("Rewritten model must be present in 2a"),
            expected_model
        );
    }

    // Check Stage 3a (solutions)
    if !config.disable_3a {
        let expected_solutions_json = read_minion_solutions_json(path, essence_base, "expected")?;
        let username_solutions_json =
            minion_solutions_to_json(solutions.as_ref().unwrap_or(&vec![]));
        assert_eq!(username_solutions_json, expected_solutions_json);
    }

    // Final assertion for rule trace (only if 4a was enabled)
    if let (Some(expected), Some(generated)) =
        (expected_rule_trace_human, generated_rule_trace_human)
    {
        assert_eq!(
            expected, generated,
            "Generated rule trace does not match the expected trace!"
        );
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

fn normalize_solutions_for_comparison(
    input_solutions: &Vec<BTreeMap<Name, Literal>>,
) -> Vec<BTreeMap<Name, Literal>> {
    let mut normalized = input_solutions.clone();

    for solset in &mut normalized {
        // remove machine names
        let keys_to_remove: Vec<Name> = solset
            .keys()
            .filter(|k| matches!(k, Name::MachineName(_)))
            .cloned()
            .collect();
        for k in keys_to_remove {
            solset.remove(&k);
        }

        let mut updates = vec![];
        for (k, v) in solset.clone() {
            if let Name::UserName(_) = k {
                match v {
                    Literal::Bool(true) => updates.push((k, Literal::Int(1))),
                    Literal::Bool(false) => updates.push((k, Literal::Int(0))),
                    _ => {}
                }
            }
        }

        for (k, v) in updates {
            solset.insert(k, v);
        }
    }

    // Remove duplicates
    normalized = normalized.into_iter().unique().collect();
    normalized
}

fn assert_vector_operators_have_partially_evaluated(model: &conjure_core::Model) {
    for node in model.universe_bi() {
        use conjure_core::ast::Expression::*;
        match node {
            Sum(_, ref vec) => assert_constants_leq_one(&node, vec),
            Min(_, ref vec) => assert_constants_leq_one(&node, vec),
            Max(_, ref vec) => assert_constants_leq_one(&node, vec),
            Or(_, ref vec) => assert_constants_leq_one(&node, vec),
            And(_, ref vec) => assert_constants_leq_one(&node, vec),
            _ => (),
        };
    }
}

fn assert_constants_leq_one(parent_expr: &Expression, exprs: &[Expression]) {
    let count = exprs.iter().fold(0, |i, x| match x {
        Expression::Atomic(_, Atom::Literal(_)) => i + 1,
        _ => i,
    });

    assert!(
        count <= 1,
        "assert_vector_operators_have_partially_evaluated: expression {} is not partially evaluated",
        parent_expr
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

    let layer1 = fmt::layer()
        .with_writer(file)
        .with_level(false)
        .with_target(false)
        .without_time()
        .with_filter(FilterFn::new(|meta: &OtherMetadata| {
            meta.target() == "rule_engine"
        }));

    layer1
}

fn create_file_layer_human(path: &str, test_name: &str) -> (impl Layer<Registry> + Send + Sync) {
    let file = File::create(format!("{path}/{test_name}-generated-rule-trace-human.txt"))
        .expect("Unable to create log file");

    let layer2 = fmt::layer()
        .with_writer(file)
        .with_level(false)
        .without_time()
        .with_target(false)
        .with_filter(EnvFilter::new("rule_engine_human=trace"))
        .with_filter(FilterFn::new(|meta| meta.target() == "rule_engine_human"));

    layer2
}

#[test]
fn assert_conjure_present() {
    conjure_oxide::find_conjure::conjure_executable().unwrap();
}

include!(concat!(env!("OUT_DIR"), "/gen_tests.rs"));
