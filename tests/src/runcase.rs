use conjure_cp::settings::{Parser, QuantifiedExpander, Rewriter, SolverFamily};
use std::collections::BTreeSet;
use std::fmt;

#[derive(Clone, Copy, Debug)]
pub struct RunCase<'a> {
    pub parser: Parser,
    pub rewriter: Rewriter,
    pub comprehension_expander: QuantifiedExpander,
    pub solver: SolverFamily,
    pub case_name: &'a str,
}

impl fmt::Display for RunCase<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "parser={}, rewriter={}, comprehension_expander={}, solver={}",
            self.parser,
            self.rewriter,
            self.comprehension_expander,
            self.solver.as_str()
        )
    }
}

pub fn run_case_label(
    path: &str,
    essence_base: &str,
    extension: &str,
    run_case: RunCase<'_>,
) -> String {
    format!(
        "test_dir={path}, model={essence_base}.{extension}, parser={}, rewriter={}, comprehension_expander={}, solver={}",
        run_case.parser,
        run_case.rewriter,
        run_case.comprehension_expander,
        run_case.solver.as_str()
    )
}

pub fn run_case_name(
    parser: Parser,
    rewriter: Rewriter,
    comprehension_expander: QuantifiedExpander,
) -> String {
    format!("{parser}-{rewriter}-{comprehension_expander}")
}

/// Returns the expected snapshot files for an executed integration run case.
pub fn expected_integration_files_for_case(
    case_name: &str,
    solver: SolverFamily,
) -> BTreeSet<String> {
    let solver_name = solver.as_str();
    BTreeSet::from([
        format!("{case_name}-{solver_name}.expected-solutions.json"),
        format!("{case_name}-{solver_name}-expected-rule-trace.txt"),
    ])
}

pub fn clean_test_dir_for_accept(
    path: &str,
    essence_base: &str,
    extension: &str,
) -> Result<(), std::io::Error> {
    let input_filename = format!("{essence_base}.{extension}");

    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let entry_path = entry.path();

        if file_name == input_filename || file_name == "config.toml" {
            continue;
        }

        if entry_path.is_dir() {
            std::fs::remove_dir_all(entry_path)?;
        } else {
            std::fs::remove_file(entry_path)?;
        }
    }

    Ok(())
}
