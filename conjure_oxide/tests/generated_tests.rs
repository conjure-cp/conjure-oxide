use conjure_core::Model;
use conjure_oxide::utils::testing::read_rule_trace;
use glob::glob;
use serde_json::json;
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs;
use std::fs::File;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tracing::field::Field;
use tracing::field::Visit;
use tracing::Subscriber;
use tracing::{span, Level};
use tracing_subscriber::{filter::EnvFilter, fmt, fmt::FmtContext, layer::SubscriberExt, Registry};

use tracing_appender::non_blocking::WorkerGuard;

use std::fmt::Write;
use std::io::BufWriter;
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
use uniplate::Uniplate;

use pretty_assertions::assert_eq;
use tracing::Event;
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::FormatEvent;
use tracing_subscriber::registry::LookupSpan;

#[derive(Deserialize, Default)]
struct TestConfig {
    extra_rewriter_asserts: Vec<String>,
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
    // let file_path = Path::new("/path/to/your/file.txt");
    // let base_name = file_path.file_stem().and_then(|stem| stem.to_str());

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
    let _guard = GUARD.lock().unwrap();

    // run tests in sequence not parallel when verbose logging, to ensure the logs are ordered
    // correctly

    let (subscriber, _guard) = create_scoped_subscriber(path, essence_base);

    // set the subscriber as default
    tracing::subscriber::with_default(subscriber, || {
        // create a span for the trace
        let test_span = span!(target: "rule_engine", Level::TRACE, "test_span");
        let _enter = test_span.enter();

        // execute tests based on verbosity
        if verbose {
            #[allow(clippy::unwrap_used)]
            let _guard = GUARD.lock().unwrap();
            integration_test_inner(path, essence_base, extension)?
        } else {
            integration_test_inner(path, essence_base, extension)?
        }

        Ok(())
    })
}

/// Runs an integration test for a given Conjure model by:
/// 1. Parsing the model from an Essence file.
/// 2. Rewriting the model according to predefined rule sets.
/// 3. Solving the model using the Minion solver and validating the solutions.
///
/// This function operates in three main stages:
/// - **Parsing Stage**: Reads the Essence model file and verifies that it parses correctly.
/// - **Rewrite Stage**: Applies a set of rules to the parsed model and validates the result.
/// - **Solution Stage**: Uses Minion to solve the model and compares solutions with expected results.
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

    // Stage 1: Read the essence file and check that the model is parsed correctly
    let model = parse_essence_file(path, essence_base, extension, context.clone())?;
    if verbose {
        println!("Parsed model: {:#?}", model)
    }

    context.as_ref().write().unwrap().file_name =
        Some(format!("{path}/{essence_base}.{extension}"));

    save_model_json(&model, path, essence_base, "parse", accept)?;
    let expected_model = read_model_json(path, essence_base, "expected", "parse")?;
    if verbose {
        println!("Expected model: {:#?}", expected_model)
    }

    assert_eq!(model, expected_model);

    // Stage 2: Rewrite the model using the rule engine and check that the result is as expected
    let rule_sets = resolve_rule_sets(SolverFamily::Minion, &get_default_rule_sets())?;
    let model = rewrite_model(&model, &rule_sets)?;

    if verbose {
        println!("Rewritten model: {:#?}", model)
    }

    save_model_json(&model, path, essence_base, "rewrite", accept)?;

    for extra_assert in config.extra_rewriter_asserts {
        match extra_assert.as_str() {
            "vector_operators_have_partially_evaluated" => {
                assert_vector_operators_have_partially_evaluated(&model)
            }
            x => println!("Unrecognised extra assert: {}", x),
        };
    }

    let expected_model = read_model_json(path, essence_base, "expected", "rewrite")?;
    if verbose {
        println!("Expected model: {:#?}", expected_model)
    }

    assert_eq!(model, expected_model);

    // Stage 3: Run the model through the Minion solver and check that the solutions are as expected
    let solutions = get_minion_solutions(model)?;

    let solutions_json = save_minion_solutions_json(&solutions, path, essence_base, accept)?;
    if verbose {
        println!("Minion solutions: {:#?}", solutions_json)
    }

    let expected_rule_trace = read_rule_trace(path, essence_base, "expected")?;
    let generated_rule_trace = read_rule_trace(path, essence_base, "generated")?;

    assert_eq!(expected_rule_trace, generated_rule_trace);

    // test solutions against conjure before writing
    if accept {
        let mut conjure_solutions: Vec<HashMap<Name, Literal>> =
            get_solutions_from_conjure(&format!("{}/{}.{}", path, essence_base, extension))?;

        // Change bools to nums in both outputs, as we currently don't convert 0,1 back to
        // booleans for Minion.

        // remove machine names from Minion solutions, as the conjure solutions won't have these.
        let mut username_solutions = solutions.clone();
        for solset in &mut username_solutions {
            for (k, v) in solset.clone().into_iter() {
                match k {
                    conjure_core::ast::Name::MachineName(_) => {
                        solset.remove(&k);
                    }
                    conjure_core::ast::Name::UserName(_) => match v {
                        Literal::Bool(true) => {
                            solset.insert(k, Literal::Int(1));
                        }
                        Literal::Bool(false) => {
                            solset.insert(k, Literal::Int(0));
                        }
                        _ => {}
                    },
                }
            }
        }

        for solset in &mut conjure_solutions {
            for (k, v) in solset.clone().into_iter() {
                match v {
                    Literal::Bool(true) => {
                        solset.insert(k, Literal::Int(1));
                    }
                    Literal::Bool(false) => {
                        solset.insert(k, Literal::Int(0));
                    }
                    _ => {}
                }
            }
        }

        // I can't make these sets of hashmaps due to hashmaps not implementing hash; so, to
        // compare these, I make them both json and compare that.

        let mut conjure_solutions_json: serde_json::Value =
            minion_solutions_to_json(&conjure_solutions);
        let mut username_solutions_json: serde_json::Value =
            minion_solutions_to_json(&username_solutions);
        conjure_solutions_json.sort_all_objects();
        username_solutions_json.sort_all_objects();

        assert_eq!(
            username_solutions_json, conjure_solutions_json,
            "Solutions do not match conjure!"
        );
    }

    let expected_solutions_json = read_minion_solutions_json(path, essence_base, "expected")?;
    if verbose {
        println!("Expected solutions: {:#?}", expected_solutions_json)
    }

    assert_eq!(solutions_json, expected_solutions_json);

    save_stats_json(context, path, essence_base)?;

    Ok(())
}

fn assert_vector_operators_have_partially_evaluated(model: &conjure_core::Model) {
    model.constraints.transform(Arc::new(|x| {
        use conjure_core::ast::Expression::*;
        match &x {
            Bubble(_, _, _) => (),
            Atomic(_, _) => (),
            Sum(_, vec) => assert_constants_leq_one(&x, vec),
            Min(_, vec) => assert_constants_leq_one(&x, vec),
            Max(_, vec) => assert_constants_leq_one(&x, vec),
            Not(_, _) => (),
            Or(_, vec) => assert_constants_leq_one(&x, vec),
            And(_, vec) => assert_constants_leq_one(&x, vec),
            Eq(_, _, _) => (),
            Neq(_, _, _) => (),
            Geq(_, _, _) => (),
            Leq(_, _, _) => (),
            Gt(_, _, _) => (),
            Lt(_, _, _) => (),
            SafeDiv(_, _, _) => (),
            UnsafeDiv(_, _, _) => (),
            SumEq(_, vec, _) => assert_constants_leq_one(&x, vec),
            SumGeq(_, vec, _) => assert_constants_leq_one(&x, vec),
            SumLeq(_, vec, _) => assert_constants_leq_one(&x, vec),
            DivEqUndefZero(_, _, _, _) => (),
            Ineq(_, _, _, _) => (),
            // this is a vector operation, but we don't want to fold values into each-other in this
            // one
            AllDiff(_, _) => (),
            WatchedLiteral(_, _, _) => (),
            Reify(_, _, _) => (),
            AuxDeclaration(_, _, _) => (),
            UnsafeMod(_, _, _) => (),
            SafeMod(_, _, _) => (),
            ModuloEqUndefZero(_, _, _, _) => (),
        };
        x.clone()
    }));
}

fn assert_constants_leq_one(parent_expr: &Expression, exprs: &[Expression]) {
    let count = exprs.iter().fold(0, |i, x| match x {
        Expression::Atomic(_, Atom::Literal(_)) => i + 1,
        _ => i,
    });

    assert!(count <= 1, "assert_vector_operators_have_partially_evaluated: expression {} is not partially evaluated",parent_expr)
}

// using a custom formatter to omit the span name in the log
// and removing the identifier and application fields for assertions
struct JsonFormatter;

impl<S, N> FormatEvent<S, N> for JsonFormatter
where
    S: Subscriber + for<'span> LookupSpan<'span>,
    N: for<'a> tracing_subscriber::fmt::FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        _ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        // initialising the log object with level and target
        let mut log = json!({
            //"level": event.metadata().level().to_string(),
            "target": event.metadata().target(),
        });

        // creating a visitor to capture fields
        struct JsonVisitor {
            log: Value,
        }

        impl Visit for JsonVisitor {
            fn record_str(&mut self, field: &Field, value: &str) {
                self.log
                    .as_object_mut()
                    .map(|obj| obj.insert(field.name().to_string(), json!(value)));
            }

            fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
                self.log
                    .as_object_mut()
                    .map(|obj| obj.insert(field.name().to_string(), json!(format!("{:?}", value))));
            }
        }

        // using the visitor to record fields
        let mut visitor = JsonVisitor { log: log.clone() };
        event.record(&mut visitor);

        // merging the visitor's log into the main log object
        log.as_object_mut().map(|obj| {
            if let Some(visitor_obj) = visitor.log.as_object() {
                for (key, value) in visitor_obj {
                    obj.insert(key.clone(), value.clone());
                }
            }
        });

        // Write the JSON log
        write!(writer, "{}\n", log)
    }
}

pub fn create_scoped_subscriber(
    path: &str,
    test_name: &str,
) -> (impl tracing::Subscriber + Send + Sync, WorkerGuard) {
    let file = File::create(format!("{path}/{test_name}-expected-rule-trace.json"))
        .expect("Unable to create log file");
    let writer = BufWriter::new(file);
    let (non_blocking, guard) = tracing_appender::non_blocking(writer);

    // subscriber setup with the JSON formatter
    let subscriber = Registry::default()
        .with(EnvFilter::new("rule_engine=trace"))
        .with(
            fmt::layer()
                .with_writer(non_blocking)
                .json()
                .event_format(JsonFormatter),
        );

    // wrapping the subscriber in an Arc to share across multiple threads
    let subscriber = Arc::new(subscriber) as Arc<dyn tracing::Subscriber + Send + Sync>;

    // setting this subscriber as the default
    let _default = tracing::subscriber::set_default(subscriber.clone());

    (subscriber, guard)
}

#[test]
fn assert_conjure_present() {
    conjure_oxide::find_conjure::conjure_executable().unwrap();
}

include!(concat!(env!("OUT_DIR"), "/gen_tests.rs"));
