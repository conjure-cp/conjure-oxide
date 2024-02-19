use conjure_oxide::ast::Model;
use conjure_oxide::parse::model_from_json;
use conjure_oxide::solvers::minion::MinionModel;
use conjure_oxide::solvers::FromConjureModel;
use conjure_oxide::utils::{sort_json_object, sort_json_variables};
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::prelude::*;

use conjure_oxide::rule_engine::resolve_rules::resolve_rule_sets;
use conjure_oxide::rule_engine::rewrite::rewrite_model;
use conjure_oxide::utils::conjure::parse_essence_file;
use conjure_oxide::utils::json::sort_json_object;
use std::path::Path;
use std::process::exit;

fn main() {
    let file_path = Path::new("/path/to/your/file.txt");
    let base_name = file_path.file_stem().and_then(|stem| stem.to_str());

    match base_name {
        Some(name) => println!("Base name: {}", name),
        None => println!("Could not extract the base name"),
    }
}

fn integration_test(path: &str, essence_base: &str) -> Result<(), Box<dyn Error>> {}

fn dummy_callback(_: HashMap<minion_rs::ast::VarName, minion_rs::ast::Constant>) -> bool {
    true
}

#[test]
fn assert_conjure_present() {
    conjure_oxide::find_conjure::conjure_executable().unwrap();
}

include!(concat!(env!("OUT_DIR"), "/gen_tests.rs"));
