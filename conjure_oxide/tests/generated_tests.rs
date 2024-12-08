use conjure_oxide::utils::essence_parser::parse_essence_file_native;
use conjure_oxide::utils::testing::read_human_rule_trace;
use glob::glob;
use itertools::Itertools;
use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::fs;
use std::fs::File;
use tracing::{span, Level};
use tracing_subscriber::{
    filter::EnvFilter, filter::FilterFn, fmt, layer::SubscriberExt, Layer, Registry,
};
use uniplate::Biplate;

use tracing_appender::non_blocking::WorkerGuard;

use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;

use conjure_core::ast::Atom;
use conjure_core::ast::{Expression, Literal, Name};
use conjure_core::context::Context;
use conjure_oxide::defaults::get_default_rule_sets;
use conjure_oxide::rule_engine::resolve_rule_sets;
use conjure_oxide::rule_engine::rewrite_model;
use conjure_oxide::utils::conjure::minion_solutions_to_json;
use conjure_oxide::utils::conjure::{
    get_minion_solutions, get_solutions_from_conjure, parse_essence_file,
};
use conjure_oxide::utils::testing::save_stats_json;
use conjure_oxide::utils::testing::{
    read_minion_solutions_json, read_model_json, save_minion_solutions_json, save_model_json,
};
use conjure_oxide::SolverFamily;
use serde::Deserialize;

use pretty_assertions::assert_eq;

#[derive(Deserialize, Default)]
struct TestConfig {
    extra_rewriter_asserts: Option<Vec<String>>,
    skip_native_parser: Option<bool>,
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

    // Lock here to ensure sequential execution
    // Tests should still run if a previous test panics while holding this mutex
    let _guard = GUARD.lock().unwrap_or_else(|e| e.into_inner());

    // run tests in sequence not parallel when verbose logging, to ensure the logs are ordered
    // correctly

    let (subscriber, _guard) = create_scoped_subscriber(path, essence_base);

    // set the subscriber as default
    tracing::subscriber::with_default(subscriber, || {
        // create a span for the trace
        // let test_span = span!(target: "rule_engine", Level::TRACE, "test_span");
        // let _enter = test_span.enter();

        // execute tests based on verbosity
        if verbose {
            #[allow(clippy::unwrap_used)]
            let _guard = GUARD.lock().unwrap_or_else(|e| e.into_inner());
            integration_test_inner(path, essence_base, extension)?
        } else {
            integration_test_inner(path, essence_base, extension)?
        }

        Ok(())
    })
}

/// Runs an integration test that:
/// - **Parsing**: Parses the Essence model and optionally compares with expected output.
/// - **Rewriting**: Applies rules to the parsed model, then compares with expected rewritten output.
/// - **Solving**: Runs Minion on the rewritten model, checks solutions against expected outcomes.
/// - **Tracing**: Validates the generated human-readable rule traces.
///
/// **ACCEPT=false**: Compares all generated artifacts against expected files.
/// **ACCEPT=true**: Only ensures final solutions match Conjure. If they do, overwrites expected files; otherwise, fails without overwriting.
///
/// **VERBOSE=true**: Prints detailed information during each stage.
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

    // Stage 0: Run native parser (unless skipped)
    let mut model_native = None;
    if config.skip_native_parser != Some(true) {
        let mn = parse_essence_file_native(path, essence_base, extension, context.clone())?;
        save_model_json(&mn, path, essence_base, "parse")?;
        model_native = Some(mn);
    }

    // Stage 1: Parse the model using the Conjure JSON parser
    let model = parse_essence_file(path, essence_base, extension, context.clone())?;
    if verbose {
        println!("Parsed model: {:#?}", model);
    }

    {
        let mut ctx = context.as_ref().write().unwrap();
        ctx.file_name = Some(format!("{path}/{essence_base}.{extension}"));
    }

    save_model_json(&model, path, essence_base, "parse")?;

    // Stage 2: Rewrite the model using the rule engine
    let rule_sets = resolve_rule_sets(SolverFamily::Minion, &get_default_rule_sets())?;
    let rewritten_model = rewrite_model(&model, &rule_sets)?;

    if verbose {
        println!("Rewritten model: {:#?}", rewritten_model);
    }

    save_model_json(&rewritten_model, path, essence_base, "rewrite")?;

    if let Some(extra_asserts) = config.extra_rewriter_asserts.clone() {
        for extra_assert in extra_asserts {
            match extra_assert.as_str() {
                "vector_operators_have_partially_evaluated" => {
                    assert_vector_operators_have_partially_evaluated(&rewritten_model)
                }
                x => println!("Unrecognised extra assert: {}", x),
            };
        }
    }

    // Stage 3: Run the model through the Minion solver
    let solutions = get_minion_solutions(rewritten_model.clone(), 0)?;
    let solutions_json = save_minion_solutions_json(&solutions, path, essence_base)?;
    if verbose {
        println!("Minion solutions: {:#?}", solutions_json);
    }

    // Stage 4: Check that the generated rules match expected traces
    let generated_rule_trace_human = read_human_rule_trace(path, essence_base, "generated")?;
    let expected_rule_trace_human = read_human_rule_trace(path, essence_base, "expected")?;

    // If ACCEPT = true, we skip intermediate assertions and only check conjure solutions
    if accept {
        // Check solutions against Conjure
        let conjure_solutions: Vec<BTreeMap<Name, Literal>> =
            get_solutions_from_conjure(&format!("{}/{}.{}", path, essence_base, extension))?;

        // Normalize solutions to allow comparison
        let username_solutions = normalize_solutions_for_comparison(&solutions);
        let conjure_solutions = normalize_solutions_for_comparison(&conjure_solutions);

        let mut conjure_solutions_json = minion_solutions_to_json(&conjure_solutions);
        let mut username_solutions_json = minion_solutions_to_json(&username_solutions);

        conjure_solutions_json.sort_all_objects();
        username_solutions_json.sort_all_objects();

        assert_eq!(
            username_solutions_json, conjure_solutions_json,
            "Solutions do not match conjure!"
        );

        // If we reach here, solutions match Conjure. We now overwrite the expected files.

        // Overwrite expected parse and rewrite models
        if config.skip_native_parser != Some(true) {
            model_native.clone().expect("model_native should exist");
            copy_generated_to_expected(path, essence_base, "parse", "serialised.json")?;
        }
        copy_generated_to_expected(path, essence_base, "parse", "serialised.json")?;
        copy_generated_to_expected(path, essence_base, "rewrite", "serialised.json")?;

        // Overwrite expected solutions
        copy_generated_to_expected(path, essence_base, "minion", "solutions.json")?;

        // Overwrite the expected human rule trace
        copy_human_trace_generated_to_expected(path, essence_base)?;

        save_stats_json(context.clone(), path, essence_base)?;
    }

    // Check Stage 0 (native parser) if not skipped
    if config.skip_native_parser != Some(true) {
        let expected_model = read_model_json(path, essence_base, "expected", "parse")?;
        let model_native = model_native.expect("model_native should exist here");
        assert_eq!(model_native, expected_model);
    }

    // Check Stage 1 (parsed model)
    let expected_model = read_model_json(path, essence_base, "expected", "parse")?;
    assert_eq!(model, expected_model);

    // Check Stage 2 (rewritten model)
    let expected_model = read_model_json(path, essence_base, "expected", "rewrite")?;
    assert_eq!(rewritten_model, expected_model);

    // Check Stage 3 (solutions)
    let expected_solutions_json = read_minion_solutions_json(path, essence_base, "expected")?;

    // Check Stage 4 (human-readable rule trace)
    assert_eq!(expected_rule_trace_human, generated_rule_trace_human);

    if verbose {
        println!("Expected solutions: {:#?}", expected_solutions_json);
    }
    assert_eq!(solutions_json, expected_solutions_json);

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
    for node in <_ as Biplate<Expression>>::universe_bi(&model.constraints) {
        use conjure_core::ast::Expression::*;
        match node {
            Sum(_, ref vec) => assert_constants_leq_one(&node, vec),
            Min(_, ref vec) => assert_constants_leq_one(&node, vec),
            Max(_, ref vec) => assert_constants_leq_one(&node, vec),
            Or(_, ref vec) => assert_constants_leq_one(&node, vec),
            And(_, ref vec) => assert_constants_leq_one(&node, vec),
            SumEq(_, ref vec, _) => assert_constants_leq_one(&node, vec),
            SumGeq(_, ref vec, _) => assert_constants_leq_one(&node, vec),
            SumLeq(_, ref vec, _) => assert_constants_leq_one(&node, vec),
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
) -> (
    impl tracing::Subscriber + Send + Sync,
    Vec<tracing_appender::non_blocking::WorkerGuard>,
) {
    //let (target1_layer, guard1) = create_file_layer_json(path, test_name);
    let (target2_layer, guard2) = create_file_layer_human(path, test_name);
    let layered = target2_layer;

    let subscriber = Arc::new(tracing_subscriber::registry().with(layered))
        as Arc<dyn tracing::Subscriber + Send + Sync>;
    // setting this subscriber as the default
    let _default = tracing::subscriber::set_default(subscriber.clone());

    (subscriber, vec![guard2])
}

fn create_file_layer_json(
    path: &str,
    test_name: &str,
) -> (impl Layer<Registry> + Send + Sync, WorkerGuard) {
    let file = File::create(format!("{path}/{test_name}-generated-rule-trace.json"))
        .expect("Unable to create log file");
    let (non_blocking, guard1) = tracing_appender::non_blocking(file);

    let layer1 = fmt::layer()
        .with_writer(non_blocking)
        .json()
        .with_level(false)
        .without_time()
        .with_target(false)
        .with_filter(EnvFilter::new("rule_engine=trace"))
        .with_filter(FilterFn::new(|meta| meta.target() == "rule_engine"));

    (layer1, guard1)
}

fn create_file_layer_human(
    path: &str,
    test_name: &str,
) -> (impl Layer<Registry> + Send + Sync, WorkerGuard) {
    let file = File::create(format!("{path}/{test_name}-generated-rule-trace-human.txt"))
        .expect("Unable to create log file");
    let (non_blocking, guard2) = tracing_appender::non_blocking(file);

    let layer2 = fmt::layer()
        .with_writer(non_blocking)
        .with_level(false)
        .without_time()
        .with_target(false)
        .with_filter(EnvFilter::new("rule_engine_human=trace"))
        .with_filter(FilterFn::new(|meta| meta.target() == "rule_engine_human"));

    (layer2, guard2)
}

#[test]
fn assert_conjure_present() {
    conjure_oxide::find_conjure::conjure_executable().unwrap();
}

include!(concat!(env!("OUT_DIR"), "/gen_tests.rs"));
