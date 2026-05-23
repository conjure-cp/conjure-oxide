mod discovery;
mod html;
mod model;
mod parser_exec;

use crate::discovery::discover_input_groups;
use crate::html::{build_html, derive_test_name};
use crate::model::{DEFAULT_OUTPUT_HTML, ParserSelection, RepoSelection, RowResult};

use crate::parser_exec::{read_input_file, run_parser_on_group, summarize_results};
use clap::{Parser, ValueEnum};
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "parser-benchmark-table",
    about = "Benchmark Essence parsers and generate a report"
)]
struct Cli {
    #[arg(long, value_enum)]
    parser: Option<ParserArg>,

    #[arg(long, value_enum, value_delimiter = ',')]
    repos: Option<Vec<RepoArg>>,

    #[arg(long, default_value = DEFAULT_OUTPUT_HTML)]
    output_html: PathBuf,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ParserArg {
    Native,
    #[value(name = "via-conjure")]
    ViaConjure,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum RepoArg {
    #[value(name = "conjure-oxide")]
    ConjureOxide,
    Conjure,
    #[value(name = "essencecatalog")]
    EssenceCatalog,
}

fn main() {
    let cli = Cli::parse();

    let parser_selection = match cli.parser {
        Some(ParserArg::Native) => ParserSelection::NativeOnly,
        Some(ParserArg::ViaConjure) => ParserSelection::ViaConjureOnly,
        None => ParserSelection::Both,
    };

    let repo_selection = repo_selection_from_cli(cli.repos.as_deref());
    let output_file = cli.output_html;

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

fn repo_selection_from_cli(repos: Option<&[RepoArg]>) -> RepoSelection {
    match repos {
        None => RepoSelection {
            conjure_oxide: true,
            conjure: true,
            essence_catalog: true,
        },
        Some(repos) => {
            let mut selection = RepoSelection {
                conjure_oxide: false,
                conjure: false,
                essence_catalog: false,
            };

            for repo in repos {
                match repo {
                    RepoArg::ConjureOxide => selection.conjure_oxide = true,
                    RepoArg::Conjure => selection.conjure = true,
                    RepoArg::EssenceCatalog => selection.essence_catalog = true,
                }
            }

            selection
        }
    }
}
