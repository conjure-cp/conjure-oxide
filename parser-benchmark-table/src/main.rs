mod discovery;
mod html;
mod model;
mod parser_exec;

use crate::discovery::discover_input_groups;
use crate::html::{build_html, derive_test_name};
use crate::model::{DEFAULT_OUTPUT_HTML, ParserSelection, RepoSelection, RowResult};

use crate::parser_exec::{read_input_file, run_parser_on_group, summarize_results};
use std::fs;
use std::path::PathBuf;

fn main() {
    // Set config values to default values
    let output_file = DEFAULT_OUTPUT_HTML.to_string();
    let mut parser_selection = ParserSelection::Both;
    let mut repo_selection = RepoSelection {
        conjure_oxide: true,
        conjure: true,
        essence_catalog: true,
    };

    // Update config values based on command line arguments
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--parser" => {
                let Some(value) = args.next() else {
                    eprintln!("--parser requires one of: native, via-conjure");
                    std::process::exit(2);
                };
                parser_selection = match value.as_str() {
                    "native" => ParserSelection::NativeOnly,
                    "via-conjure" => ParserSelection::ViaConjureOnly,
                    _ => {
                        eprintln!(
                            "Unknown --parser value '{}'. Use: native, via-conjure",
                            value
                        );
                        std::process::exit(2);
                    }
                };
            }
            "--repos" => {
                let Some(value) = args.next() else {
                    eprintln!(
                        "--repos requires a comma list of repositories to include: conjure-oxide,conjure,essencecatalog"
                    );
                    std::process::exit(2);
                };
                repo_selection = parse_repo_selection(&value);
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            _ => {
                eprintln!("Unknown argument: {}", arg);
                print_usage();
                std::process::exit(2);
            }
        }
    }

    // Discover input groups by scanning the selected repositories
    let groups = discover_input_groups(parser_selection, repo_selection);
    let total = groups.len();
    println!("Found {} input groups to parse.", total);

    let mut rows = Vec::new();
    for (idx, group) in groups.into_iter().enumerate() {
        // Run the selected parser(s) on the group, capturing results and outputs for each
        let native = if matches!(
            parser_selection,
            ParserSelection::NativeOnly | ParserSelection::Both
        ) {
            Some(run_parser_on_group(&group, true))
        } else {
            None
        };

        let via_conjure = if matches!(
            parser_selection,
            ParserSelection::ViaConjureOnly | ParserSelection::Both
        ) {
            Some(run_parser_on_group(&group, false))
        } else {
            None
        };

        // Get the display names from the file paths
        let primary_relative = group
            .primary_file
            .strip_prefix(&group.repo_root)
            .unwrap_or(&group.primary_file)
            .display()
            .to_string();

        let param_relative = group
            .param_file
            .as_ref()
            .map(|p| {
                p.strip_prefix(&group.repo_root)
                    .unwrap_or(p)
                    .display()
                    .to_string()
            })
            .unwrap_or_default();

        let test_name = derive_test_name(&group.repo_name, &primary_relative);

        // Read the contents of the primary and parameter files for display in the table
        let primary_contents = read_input_file(&group.primary_file);
        let param_contents = group
            .param_file
            .as_ref()
            .map_or_else(String::new, |p| read_input_file(p));

        rows.push(RowResult {
            repo_name: group.repo_name,
            kind: group.group_kind,
            test_name,
            primary_relative,
            param_relative,
            primary_contents,
            param_contents,
            native,
            via_conjure,
        });

        print_progress(idx + 1, total);
    }

    if total > 0 {
        println!();
    }

    // Build the HTML report from the results and write it to the output file
    let html = build_html(&rows, &parser_selection, &repo_selection);
    fs::write(&output_file, html).expect("failed to write parser benchmark html");

    // Print summary to terminal
    let (native_pass, native_fail, via_pass, via_fail) = summarize_results(&rows);

    let output_path = PathBuf::from(&output_file);
    let output_abs = if output_path.is_absolute() {
        output_path
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(output_path)
    };

    println!("\nSummary");
    if let (Some(pass), Some(fail)) = (native_pass, native_fail) {
        println!("- Native parser: {} pass, {} fail", pass, fail);
    }
    if let (Some(pass), Some(fail)) = (via_pass, via_fail) {
        println!("- Via-conjure parser: {} pass, {} fail", pass, fail);
    }
    println!(
        "\nWrote parser benchmark report to {}",
        output_abs.display()
    );
    println!("View report with: open {}", output_abs.display());
}

fn print_progress(done: usize, total: usize) {
    if total == 0 {
        return;
    }

    let width = 32usize;
    let filled = (done * width) / total;
    let percent = (done * 100) / total;
    let bar = format!(
        "{}{}",
        "#".repeat(filled),
        "-".repeat(width.saturating_sub(filled))
    );

    print!("\rProgress [{}] {:>3}% ({}/{})", bar, percent, done, total);
    let _ = std::io::Write::flush(&mut std::io::stdout());
}

fn parse_repo_selection(value: &str) -> RepoSelection {
    let mut selection = RepoSelection {
        conjure_oxide: false,
        conjure: false,
        essence_catalog: false,
    };

    for token in value.split(',').map(|s| s.trim().to_ascii_lowercase()) {
        match token.as_str() {
            "conjure-oxide" => selection.conjure_oxide = true,
            "conjure" => selection.conjure = true,
            "essencecatalog" => selection.essence_catalog = true,
            other => {
                eprintln!(
                    "Unknown repo '{}' in --repos. Options: conjure-oxide, conjure, essencecatalog",
                    other
                );
                std::process::exit(2);
            }
        }
    }

    selection
}

fn print_usage() {
    println!("Usage: parser-benchmark [--parser native|via-conjure] [--repos LIST]");
    println!("  --parser MODE    Which parser(s) to run: native, via-conjure");
    println!("  --repos LIST     Comma list: conjure-oxide,conjure,essencecatalog");
}
