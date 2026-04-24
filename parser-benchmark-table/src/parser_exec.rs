use crate::model::{InputGroup, ParseResult, RowResult};
use conjure_cp::context::Context;
use conjure_cp::parse::tree_sitter::errors::ParseErrorCollection;
use conjure_cp::parse::tree_sitter::{parse_essence_file, parse_essence_file_native};
use std::fs;
use std::io::Write;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn run_parser_on_group(group: &InputGroup, native: bool) -> ParseResult {
    let temp_path = write_combined_group_file(group);
    let parse_result = run_one_file(&temp_path, native);

    let _ = fs::remove_file(&temp_path);

    match parse_result {
        ParseResult { pass: true, .. } => ParseResult {
            pass: true,
            summary: "pass",
            output_or_error: format!(
                "{}\nparsed combined group successfully",
                group.primary_file.display()
            ),
        },
        ParseResult {
            pass: false,
            output_or_error,
            ..
        } => ParseResult {
            pass: false,
            summary: "fail",
            output_or_error,
        },
    }
}

pub fn read_input_file(path: &Path) -> String {
    match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(err) => format!("Could not read file contents: {}", err),
    }
}

pub fn summarize_results(
    rows: &[RowResult],
) -> (Option<usize>, Option<usize>, Option<usize>, Option<usize>) {
    let native_total = rows.iter().filter(|r| r.native.is_some()).count();
    let via_total = rows.iter().filter(|r| r.via_conjure.is_some()).count();

    let native = if native_total > 0 {
        let native_pass = rows
            .iter()
            .filter(|r| r.native.as_ref().is_some_and(|p| p.pass))
            .count();
        Some((native_pass, native_total.saturating_sub(native_pass)))
    } else {
        None
    };

    let via = if via_total > 0 {
        let via_pass = rows
            .iter()
            .filter(|r| r.via_conjure.as_ref().is_some_and(|p| p.pass))
            .count();
        Some((via_pass, via_total.saturating_sub(via_pass)))
    } else {
        None
    };

    (
        native.map(|x| x.0),
        native.map(|x| x.1),
        via.map(|x| x.0),
        via.map(|x| x.1),
    )
}

fn run_one_file(path: &Path, native: bool) -> ParseResult {
    let context: Arc<RwLock<Context<'static>>> = Default::default();
    let path_str = path.to_string_lossy().to_string();

    // Run the parser in a catch_unwind block to catch any panics and convert them into test failures
    let parse_result = catch_unwind_silent(AssertUnwindSafe(|| {
        if native {
            parse_essence_file_native(&path_str, context)
        } else {
            parse_essence_file(&path_str, context)
        }
    }));

    // Handle any panics that occured and convert them into a ParseResult
    match parse_result {
        Ok(parser_outcome) => match parser_outcome {
            Ok(_) => ParseResult {
                pass: true,
                summary: "pass",
                output_or_error: format!("{}\nparsed successfully", path.display()),
            },
            Err(err_box) => match err_box.as_ref() {
                ParseErrorCollection::Fatal(_) => ParseResult {
                    pass: false,
                    summary: "fail",
                    output_or_error: format!("{}\n{}", path.display(), err_box),
                },
                _ => ParseResult {
                    pass: true,
                    summary: "pass",
                    output_or_error: format!("{}\n{}", path.display(), err_box),
                },
            },
        },
        Err(payload) => {
            let panic_message = if let Some(msg) = payload.downcast_ref::<&str>() {
                (*msg).to_string()
            } else if let Some(msg) = payload.downcast_ref::<String>() {
                msg.clone()
            } else {
                "unknown panic payload".to_string()
            };

            ParseResult {
                pass: false,
                summary: "fail",
                output_or_error: format!(
                    "{}\nPANIC while parsing: {}",
                    path.display(),
                    panic_message
                ),
            }
        }
    }
}

fn write_combined_group_file(group: &InputGroup) -> PathBuf {
    // Combine the contents of the param file (if any) and the primary file into a single source string for parsing
    let mut source = String::new();

    if let Some(param) = &group.param_file {
        source.push_str(&read_input_file(param));
        source.push_str("\n\n");
    }

    source.push_str(&read_input_file(&group.primary_file));
    source.push('\n');

    // Write the combined source to a temporary file and return the path to that file
    let mut temp_path = std::env::temp_dir();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();

    temp_path.push(format!(
        "parser-benchmark-{}-{}-{}.essence",
        std::process::id(),
        sanitize_for_filename(&group.repo_name),
        stamp
    ));

    let mut file = fs::File::create(&temp_path)
        .unwrap_or_else(|err| panic!("failed to create temporary combined input file: {err}"));
    file.write_all(source.as_bytes())
        .unwrap_or_else(|err| panic!("failed to write temporary combined input file: {err}"));

    temp_path
}

fn sanitize_for_filename(input: &str) -> String {
    input
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect()
}

fn catch_unwind_silent<F, R>(f: F) -> std::thread::Result<R>
where
    F: FnOnce() -> R + std::panic::UnwindSafe,
{
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = catch_unwind(f);
    std::panic::set_hook(previous_hook);
    result
}
