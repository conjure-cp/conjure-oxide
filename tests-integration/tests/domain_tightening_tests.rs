use std::{
    env,
    error::Error,
    fs,
    sync::{Arc, RwLock},
};

use conjure_cp::parse::tree_sitter::{parse_essence_file, parse_essence_file_native};
use conjure_cp::settings::Parser;
use conjure_cp::{
    Model, ast::ExprInfo, context::Context, parse::tree_sitter::errors::ParseErrorCollection,
};
use tests_integration::TestConfig;

/// Parser function used by expression domain tests.
type ParseFn = fn(&str, Arc<RwLock<Context<'static>>>) -> Result<Model, Box<ParseErrorCollection>>;

/// Runs a test for one model using each configured parser
fn expression_domain_test(
    path: &str,
    filename: &str,
    extension: &str,
) -> Result<(), Box<dyn Error>> {
    let accept = env::var("ACCEPT").unwrap_or("false".to_string()) == "true";

    let file_config: TestConfig =
        if let Ok(config_contents) = fs::read_to_string(format!("{path}/config.toml")) {
            toml::from_str(&config_contents).unwrap()
        } else {
            Default::default()
        };

    if accept {
        clean_test_dir_for_accept(path)?;
    }

    let parsers = file_config
        .configured_parsers()
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?;

    for parser in parsers {
        let case_name = parser.to_string();
        let parse = match parser {
            Parser::TreeSitter => parse_essence_file_native,
            Parser::ViaConjure => parse_essence_file,
        };
        expression_domain_test_inner(path, filename, &case_name, extension, parse)?;
    }

    Ok(())
}

fn expression_domain_test_inner(
    path: &str,
    input_filename: &str,
    case_name: &str,
    extension: &str,
    parse: ParseFn,
) -> Result<(), Box<dyn Error>> {
    unimplemented!()
}

fn clean_test_dir_for_accept(path: &str) -> Result<(), std::io::Error> {
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let entry_path = entry.path();

        if entry_path.is_dir() {
            continue;
        }

        let keep = if file_name == "config.toml" {
            true
        } else {
            let is_model_file = entry_path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext == "essence");
            let is_generated_or_expected =
                file_name.contains(".generated") || file_name.contains(".expected");
            is_model_file && !is_generated_or_expected
        };

        if keep {
            continue;
        }

        std::fs::remove_file(entry_path)?;
    }

    Ok(())
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
    let exprs: Vec<ExprInfo> = serde_json::from_str(&serialised).map_err(std::io::Error::other)?;

    Ok(exprs)
}

// include!(concat!(env!("OUT_DIR"), "/gen_tests_domain_tightening.rs"));
