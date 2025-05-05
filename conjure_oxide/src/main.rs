#![allow(clippy::unwrap_used)]
mod cli;
mod print_info_schema;
mod solve;
mod test_solve;
use clap::{CommandFactory, Parser};
use clap_complete::generate;
use cli::Cli;
use git_version::git_version;
use print_info_schema::run_print_info_schema_command;
use solve::run_solve_command;
use std::io;
use std::process::exit;
use test_solve::run_test_solve_command;

pub fn main() {
    // exit with 2 instead of 1 on failure,like grep
    match run() {
        Ok(_) => {
            exit(0);
        }
        Err(e) => {
            eprintln!("{:?}", e);
            exit(2);
        }
    }
}

pub fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.version {
        println!("Version: {}", git_version!());
        return Ok(());
    }

    run_subcommand(cli)
}

fn run_completion_command(completion_args: cli::CompletionArgs) -> anyhow::Result<()> {
    let mut cmd = Cli::command();
    let shell = completion_args.shell;
    let name = cmd.get_name().to_string();

    eprintln!("Generating completion for {}...", shell);

    generate(shell, &mut cmd, name, &mut io::stdout());
    Ok(())
}

/// Runs the selected subcommand
fn run_subcommand(cli: Cli) -> anyhow::Result<()> {
    let global_args = cli.global_args;
    match cli.subcommand {
        cli::Command::Solve(solve_args) => run_solve_command(global_args, solve_args),
        cli::Command::TestSolve(local_args) => run_test_solve_command(global_args, local_args),
        cli::Command::PrintJsonSchema => run_print_info_schema_command(),
        cli::Command::Completion(completion_args) => run_completion_command(completion_args),
    }
}

#[cfg(test)]
mod tests {
    use conjure_oxide::{get_example_model, get_example_model_by_path};

    #[test]
    fn test_get_example_model_success() {
        let filename = "input";
        get_example_model(filename).unwrap();
    }

    #[test]
    fn test_get_example_model_by_filepath() {
        let filepath = "tests/integration/xyz/input.essence";
        get_example_model_by_path(filepath).unwrap();
    }

    #[test]
    fn test_get_example_model_fail_empty_filename() {
        let filename = "";
        get_example_model(filename).unwrap_err();
    }

    #[test]
    fn test_get_example_model_fail_empty_filepath() {
        let filepath = "";
        get_example_model_by_path(filepath).unwrap_err();
    }
}
