use std::env;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;

use conjure_core::ast::Expression;
use conjure_core::context::Context;
use conjure_oxide::rule_engine::resolve_rule_sets;
use conjure_oxide::rule_engine::rewrite_model;
use conjure_oxide::utils::conjure::{get_minion_solutions, parse_essence_file};
use conjure_oxide::utils::testing::save_stats_json;
use conjure_oxide::utils::testing::{
    read_minion_solutions_json, read_model_json, save_minion_solutions_json, save_model_json,
};
use conjure_oxide::SolverFamily;

use uniplate::Uniplate;

use serde::Deserialize;

#[derive(Deserialize, Default)]
struct TestConfig {
    extra_rewriter_asserts: Vec<String>,
}

fn main() {
    let file_path = Path::new("/path/to/your/file.txt");
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

    // run tests in sequence not parallel when verbose logging, to ensure the logs are ordered
    // correctly
    if verbose {
        #[allow(clippy::unwrap_used)]
        #[allow(unused_variables)]
        let guard = GUARD.lock().unwrap();
        integration_test_inner(path, essence_base, extension)
    } else {
        integration_test_inner(path, essence_base, extension)
    }
}

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
    let rule_sets = resolve_rule_sets(
        SolverFamily::Minion,
        &vec!["Constant".to_string(), "Bubble".to_string()],
    )?;
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

    let expected_solutions_json = read_minion_solutions_json(path, essence_base, "expected")?;
    if verbose {
        println!("Expected solutions: {:#?}", expected_solutions_json)
    }

    assert_eq!(solutions_json, expected_solutions_json);

    save_stats_json(context, path, essence_base)?;

    Ok(())
}

fn assert_vector_operators_have_partially_evaluated(model: &conjure_core::Model) {
    model.constraints.descend(Arc::new(|x| {
        use conjure_core::ast::Expression::*;
        match &x {
            Nothing => (),
            Bubble(_, _, _) => (),
            Constant(_, _) => (),
            Reference(_, _) => (),
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
            DivEq(_, _, _, _) => (),
            Ineq(_, _, _, _) => (),
            AllDiff(_, vec) => assert_constants_leq_one(&x, vec),
        };
        x.clone()
    }));
}

fn assert_constants_leq_one(parent_expr: &Expression, exprs: &[Expression]) {
    let count = exprs.iter().fold(0, |i, x| match x {
        Expression::Constant(_, _) => i + 1,
        _ => i,
    });

    assert!(count <= 1, "assert_vector_operators_have_partially_evaluated: expression {} is not partially evaluated",parent_expr)
}

#[test]
fn assert_conjure_present() {
    conjure_oxide::find_conjure::conjure_executable().unwrap();
}

include!(concat!(env!("OUT_DIR"), "/gen_tests.rs"));
