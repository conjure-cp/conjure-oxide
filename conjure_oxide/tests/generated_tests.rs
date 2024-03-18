use conjure_oxide::rule_engine::resolve_rules::resolve_rule_sets;
use conjure_oxide::rule_engine::rewrite::rewrite_model;
use conjure_oxide::utils::conjure::{get_minion_solutions, parse_essence_file};
use conjure_oxide::utils::testing::{
    read_minion_solutions_json, read_model_json, save_minion_solutions_json, save_model_json,
};
use conjure_oxide::SolverFamily;
use std::env;
use std::error::Error;
use std::path::Path;

fn main() {
    let file_path = Path::new("/path/to/your/file.txt");
    let base_name = file_path.file_stem().and_then(|stem| stem.to_str());

    match base_name {
        Some(name) => println!("Base name: {}", name),
        None => println!("Could not extract the base name"),
    }
}

fn integration_test(path: &str, essence_base: &str) -> Result<(), Box<dyn Error>> {
    let accept = env::var("ACCEPT").unwrap_or("false".to_string()) == "true";
    let verbose = env::var("VERBOSE").unwrap_or("false".to_string()) == "true";

    if verbose {
        println!(
            "Running integration test for {}/{}, ACCEPT={}",
            path, essence_base, accept
        );
    }

    // Stage 1: Read the essence file and check that the model is parsed correctly
    let model = parse_essence_file(path, essence_base)?;
    if verbose {
        println!("Parsed model: {:#?}", model)
    }

    save_model_json(&model, path, essence_base, "parse", accept)?;
    let expected_model = read_model_json(path, essence_base, "expected", "parse")?;
    if verbose {
        println!("Expected model: {:#?}", expected_model)
    }

    assert_eq!(model, expected_model);

    // Stage 2: Rewrite the model using the rule engine and check that the result is as expected
    let rule_sets = resolve_rule_sets(SolverFamily::Minion, vec!["Constant"])?;
    let model = rewrite_model(&model, &rule_sets)?;
    if verbose {
        println!("Rewritten model: {:#?}", model)
    }

    save_model_json(&model, path, essence_base, "rewrite", accept)?;
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

    Ok(())
}

#[test]
fn assert_conjure_present() {
    conjure_oxide::find_conjure::conjure_executable().unwrap();
}

include!(concat!(env!("OUT_DIR"), "/gen_tests.rs"));
