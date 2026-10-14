use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io;
use std::path::Path;

use conjure_cp::ast::{AbstractLiteral, GroundDomain, Literal, Name};
use conjure_cp_cli::utils::conjure::solutions_to_essence;
use uniplate::Uniplate;

/// Use the same value spelling across solvers without discarding matrix index domains.
pub fn render_shared_solutions(solutions: &[BTreeMap<Name, Literal>]) -> String {
    let mut canonical: Vec<BTreeMap<Name, Literal>> = solutions
        .iter()
        .map(|solution| {
            solution
                .iter()
                .map(|(name, value)| {
                    let value = value.transform(&|literal| match literal {
                        Literal::Bool(value) => Literal::Int(i32::from(value)),
                        Literal::AbstractLiteral(AbstractLiteral::Matrix(values, domain)) => {
                            // Some adaptors attach the full matrix domain, others only its index
                            // domain. Each nested matrix stores its own remaining index domains.
                            let domain = match domain.as_ref() {
                                GroundDomain::Matrix(_, indices) => {
                                    indices.first().cloned().unwrap_or(domain)
                                }
                                _ => domain,
                            };
                            Literal::AbstractLiteral(AbstractLiteral::Matrix(values, domain))
                        }
                        Literal::AbstractLiteral(AbstractLiteral::Set(mut values)) => {
                            values.sort_by(Literal::essence_cmp);
                            Literal::AbstractLiteral(AbstractLiteral::Set(values))
                        }
                        Literal::AbstractLiteral(AbstractLiteral::MSet(mut values)) => {
                            values.sort_by(Literal::essence_cmp);
                            Literal::AbstractLiteral(AbstractLiteral::MSet(values))
                        }
                        Literal::AbstractLiteral(AbstractLiteral::Record(mut fields)) => {
                            fields.sort_by_key(|field| field.name.to_string());
                            Literal::AbstractLiteral(AbstractLiteral::Record(fields))
                        }
                        Literal::AbstractLiteral(AbstractLiteral::Function(mut pairs)) => {
                            pairs.sort_by(|(ak, av), (bk, bv)| {
                                ak.essence_cmp(bk).then_with(|| av.essence_cmp(bv))
                            });
                            Literal::AbstractLiteral(AbstractLiteral::Function(pairs))
                        }
                        Literal::AbstractLiteral(AbstractLiteral::Relation(mut rows)) => {
                            rows.sort_by(|a, b| compare_rows(a, b));
                            Literal::AbstractLiteral(AbstractLiteral::Relation(rows))
                        }
                        Literal::AbstractLiteral(AbstractLiteral::Partition(mut parts)) => {
                            for part in &mut parts {
                                part.sort_by(Literal::essence_cmp);
                            }
                            parts.sort_by(|a, b| compare_rows(a, b));
                            Literal::AbstractLiteral(AbstractLiteral::Partition(parts))
                        }
                        other => other,
                    });
                    (name.clone(), value)
                })
                .collect()
        })
        .collect();
    // Different representations can produce the same assignment more than once.
    // Compare sets of problem assignments, independently of auxiliary solver variables.
    let mut seen = HashSet::new();
    canonical.retain(|solution| seen.insert(solution.clone()));
    let rendered = solutions_to_essence(&canonical);
    if rendered.is_empty() {
        rendered
    } else {
        format!("{}\n", rendered.trim_end())
    }
}

fn compare_rows(a: &[Literal], b: &[Literal]) -> std::cmp::Ordering {
    a.iter()
        .zip(b)
        .map(|(a, b)| a.essence_cmp(b))
        .find(|ordering| *ordering != std::cmp::Ordering::Equal)
        .unwrap_or_else(|| a.len().cmp(&b.len()))
}

/// Holds one complete solution set while all configurations of a fixture are checked.
#[derive(Default)]
pub struct SharedSolutionSnapshot {
    first: Option<(String, String)>,
    disagreement: bool,
}

impl SharedSolutionSnapshot {
    pub fn record(&mut self, run_label: &str, rendered: String) -> io::Result<()> {
        if let Some((first_label, first)) = &self.first {
            if first != &rendered {
                self.disagreement = true;
                return Err(io::Error::other(format!(
                    "Complete solution sets disagree between {first_label} and {run_label}; \
                     shared solution snapshot was not accepted. See the generated solution files."
                )));
            }
        } else {
            self.first = Some((run_label.to_owned(), rendered));
        }
        Ok(())
    }

    /// Called only after every run succeeds, including trace and reference validation.
    pub fn finish(&self, expected_path: &Path, accept: bool) -> io::Result<()> {
        if self.disagreement {
            return Err(io::Error::other(
                "Cannot publish a shared snapshot after configurations disagreed",
            ));
        }
        let Some((run_label, rendered)) = &self.first else {
            return Ok(());
        };
        if accept {
            fs::write(expected_path, rendered)
        } else {
            let expected = fs::read_to_string(expected_path)?;
            if &expected != rendered {
                return Err(io::Error::other(format!(
                    "Complete solution set from {run_label} does not match {}. \
                     See the generated solution files.",
                    expected_path.display()
                )));
            }
            Ok(())
        }
    }
}
