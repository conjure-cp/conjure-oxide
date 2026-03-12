use std::{fs, sync::{Arc, RwLock}};

use conjure_cp::{Model, ast::ExprInfo, context::Context, parse::tree_sitter::errors::ParseErrorCollection};

/// Parser function used by expression domain tests.
type ParseFn = fn(&str, Arc<RwLock<Context<'static>>>) -> Result<Model, Box<ParseErrorCollection>>;

/// Runs a single test
fn expression_domain_test() {
    unimplemented!()
}

/// Returns the saved expression JSON path
fn expression_domains_json_path(path: &str, case_name: &str, file_type: &str) -> String {
    format!("{path}/{case_name}.{file_type}.serialised.json")
}

/// Reads and initialises a saved expression domains snapshot.
fn read_expression_domains_json(
    context: &Arc<RwLock<Context<'static>>>,
    path: &str,
    case_name: &str,
    file_type: &str,
) -> Result<Vec<ExprInfo>, std::io::Error> {
    let serialised = fs::read_to_string(expression_domains_json_path(path, case_name, file_type))?;
    let exprs: Vec<ExprInfo> =
        serde_json::from_str(&serialised).map_err(std::io::Error::other)?;

    Ok(exprs)
}


// include!(concat!(env!("OUT_DIR"), "/gen_tests_domain_tightening.rs"));