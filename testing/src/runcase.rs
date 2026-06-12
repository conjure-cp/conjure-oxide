use conjure_cp::settings::{Parser, QuantifiedExpander, Rewriter, SolverFamily};
use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

#[derive(Clone, Debug)]
pub struct RunCase {
    pub parser: Parser,
    pub rewriter: Rewriter,
    pub comprehension_expander: QuantifiedExpander,
    pub solver: SolverFamily,
}

impl fmt::Display for RunCase {
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

impl FromStr for RunCase {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let mut parser = None;
        let mut rewriter = None;
        let mut comprehension_expander = None;
        let mut solver = None;

        for part in s.split(", ") {
            let (key, value) = part.split_once('=').ok_or_else(|| {
                format!("invalid RunCase format: expected 'key=value', got '{part}'")
            })?;
            match key {
                "parser" => parser = Some(value.parse::<Parser>()?),
                "rewriter" => rewriter = Some(value.parse::<Rewriter>()?),
                "comprehension_expander" => {
                    comprehension_expander = Some(value.parse::<QuantifiedExpander>()?);
                }
                "solver" => solver = Some(value.parse::<SolverFamily>()?),
                other => return Err(format!("unknown RunCase key '{other}'")),
            }
        }

        let parser = parser.ok_or_else(|| format!("missing 'parser' in '{s}'"))?;
        let rewriter = rewriter.ok_or_else(|| format!("missing 'rewriter' in '{s}'"))?;
        let comprehension_expander = comprehension_expander
            .ok_or_else(|| format!("missing 'comprehension_expander' in '{s}'"))?;
        let solver = solver.ok_or_else(|| format!("missing 'solver' in '{s}'"))?;

        Ok(RunCase {
            parser,
            rewriter,
            comprehension_expander,
            solver,
        })
    }
}

impl RunCase {
    pub fn run_case_label(self: &RunCase) -> String {
        format!(
            "{}-{}-{}-{}",
            self.parser,
            self.rewriter,
            self.comprehension_expander,
            self.solver.as_str()
        )
    }
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
