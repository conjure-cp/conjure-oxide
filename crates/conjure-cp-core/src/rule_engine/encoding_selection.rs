//! Resolve semantic SAT encoding decisions through pins and modelling heuristics.
use crate::ast::Expression;
use crate::ast::sat_decision::*;

/// Pins take precedence; a singleton family consumes no portfolio choice.
fn select_encoding<T: Copy>(
    pinned: Option<T>,
    algorithms: &[T],
    labels: &[&str],
    compact: impl FnOnce() -> usize,
) -> (T, SelectionProvenance) {
    use crate::settings::{self, Heuristic};
    if let Some(algorithm) = pinned {
        return (algorithm, SelectionProvenance::ExplicitConfiguration);
    }
    let index = if algorithms.len() == 1 {
        0
    } else {
        match settings::heuristic() {
            Heuristic::First => 0,
            Heuristic::Compact => compact(),
            Heuristic::Random => settings::next_heuristic_random_index(algorithms.len()),
            Heuristic::Interactive => settings::next_heuristic_interactive_index(labels),
            Heuristic::All => settings::next_heuristic_all_index(labels),
        }
    };
    (algorithms[index], SelectionProvenance::Heuristic)
}

/// Resolve unpinned encoding classes through the configured heuristic.
/// Compact chooses AMO and table algorithms per constraint; portfolio heuristics share families.
pub(crate) fn resolve_encoding_choices(decisions: &mut [SatEncodingDecision]) {
    resolve_element_choices(decisions);
    resolve_table_choices(decisions);
    resolve_alldifferent_choices(decisions);
    resolve_pb_choices(decisions);
    resolve_cardinality_choices(decisions);
    resolve_amo_choices(decisions);
}
fn resolve_element_choices(decisions: &mut [SatEncodingDecision]) {
    use crate::settings;
    let sizes = decisions
        .iter()
        .filter_map(|decision| match decision {
            SatEncodingDecision::Element {
                entries,
                encoding: None,
                ..
            } => Some(entries.len()),
            _ => None,
        })
        .collect::<Vec<_>>();
    if sizes.is_empty() {
        return;
    }
    let algorithms = &ElementEncoding::ALL;
    let (algorithm, provenance) = select_encoding(
        settings::element_encoding(),
        algorithms,
        &ElementEncoding::LABELS,
        || usize::from(sizes.iter().any(|size| *size > 4)),
    );
    for decision in decisions {
        if let SatEncodingDecision::Element { encoding, .. } = decision
            && encoding.is_none()
        {
            *encoding = Some(EncodingSelection {
                algorithm,
                provenance,
            });
        }
    }
}
fn resolve_table_choices(decisions: &mut [SatEncodingDecision]) {
    use crate::settings::{self, Heuristic};
    if settings::table_encoding().is_none() && settings::heuristic() == Heuristic::Compact {
        for decision in decisions {
            if let SatEncodingDecision::Table {
                rows,
                row_views,
                encoding,
                ..
            } = decision
                && encoding.is_none()
            {
                let algorithm = compact_table_encoding(rows, row_views.as_ref().map(Vec::len));
                *encoding = Some(EncodingSelection {
                    algorithm,
                    provenance: SelectionProvenance::Heuristic,
                });
            }
        }
        return;
    }
    let rows = decisions
        .iter()
        .filter_map(|decision| match decision {
            SatEncodingDecision::Table {
                rows,
                encoding: None,
                ..
            } => Some(rows.len()),
            _ => None,
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return;
    }
    let binary = decisions.iter().all(|decision| match decision {
        SatEncodingDecision::Table {
            inputs, row_views, ..
        } => inputs.len() == 2 && row_views.is_none(),
        _ => true,
    });
    let algorithms = &TableEncoding::ALL[..if binary { 3 } else { 2 }];
    let labels = &TableEncoding::LABELS[..algorithms.len()];
    let (algorithm, provenance) =
        select_encoding(settings::table_encoding(), algorithms, labels, || {
            usize::from(rows.iter().any(|size| *size > 4))
        });
    for decision in decisions {
        if let SatEncodingDecision::Table { encoding, .. } = decision
            && encoding.is_none()
        {
            *encoding = Some(EncodingSelection {
                algorithm,
                provenance,
            });
        }
    }
}

/// Count real variable rows or distinct constant rows before choosing the composition.
fn compact_table_encoding(rows: &[Vec<i64>], variable_row_count: Option<usize>) -> TableEncoding {
    let count = variable_row_count
        .unwrap_or_else(|| rows.iter().collect::<std::collections::BTreeSet<_>>().len());
    if count > 4 {
        TableEncoding::Mdd
    } else {
        TableEncoding::Tuple
    }
}
fn resolve_alldifferent_choices(decisions: &mut [SatEncodingDecision]) {
    use crate::settings;
    let eligible: Vec<_> = decisions
        .iter()
        .filter_map(|decision| match decision {
            SatEncodingDecision::AllDifferent {
                inputs,
                comparisons,
                encoding: None,
                ..
            } => Some(comparisons.is_none() && inputs.iter().all(|input| input.choices.is_some())),
            _ => None,
        })
        .collect();
    if eligible.is_empty() {
        return;
    }
    let algorithms = if eligible.iter().all(|eligible| *eligible) {
        &AllDifferentEncoding::ALL[..]
    } else {
        &AllDifferentEncoding::ALL[..1]
    };
    let (algorithm, provenance) = select_encoding(
        settings::alldifferent_encoding(),
        algorithms,
        &AllDifferentEncoding::LABELS[..algorithms.len()],
        || 1,
    );
    for decision in decisions {
        if let SatEncodingDecision::AllDifferent { encoding, .. } = decision
            && encoding.is_none()
        {
            *encoding = Some(EncodingSelection {
                algorithm,
                provenance,
            });
        }
    }
}
/// Whether a count comparison needs an upper threshold of one after constant folding.
pub(crate) fn count_uses_amo(inputs: &[Expression], relation: IntegerRelation, bound: i64) -> bool {
    let (constants, variables) =
        inputs
            .iter()
            .fold(
                (0i128, 0usize),
                |(constants, variables), input| match crate::ast::eval_constant(input) {
                    Some(crate::ast::Literal::Bool(true)) => (constants + 1, variables),
                    Some(crate::ast::Literal::Bool(false)) => (constants, variables),
                    _ => (constants, variables + 1),
                },
            );
    if variables <= 1 {
        return false;
    }
    let bound = i128::from(bound) - constants;
    match relation {
        IntegerRelation::Less | IntegerRelation::GreaterEqual => bound - 1 == 1,
        IntegerRelation::LessEqual | IntegerRelation::Greater => bound == 1,
        IntegerRelation::Equal | IntegerRelation::NotEqual => bound == 1 || bound - 1 == 1,
    }
}

fn resolve_amo_choices(decisions: &mut [SatEncodingDecision]) {
    use crate::settings::{self, Heuristic};
    let compact = settings::amo_encoding().is_none() && settings::heuristic() == Heuristic::Compact;
    let largest = decisions
        .iter()
        .filter_map(|decision| match decision {
            SatEncodingDecision::AtMostOne {
                inputs,
                encoding: None,
            } => Some(inputs.len()),
            SatEncodingDecision::CountRelation {
                inputs,
                relation,
                bound,
                amo_encoding: None,
                ..
            } if count_uses_amo(inputs, *relation, *bound) => Some(inputs.len()),
            SatEncodingDecision::AllDifferent {
                inputs,
                encoding: Some(selection),
                amo_encoding: None,
                ..
            } if selection.algorithm == AllDifferentEncoding::ValueAmo => Some(inputs.len()),
            _ => None,
        })
        .max();
    let Some(largest) = largest else {
        return;
    };
    let (algorithm, provenance) = select_encoding(
        settings::amo_encoding(),
        &AmoEncoding::ALL,
        &AmoEncoding::LABELS,
        || usize::from(largest > 5),
    );
    for decision in decisions {
        let algorithm = if compact {
            let size = match &*decision {
                SatEncodingDecision::AtMostOne { inputs, .. }
                | SatEncodingDecision::CountRelation { inputs, .. } => inputs.len(),
                SatEncodingDecision::AllDifferent { inputs, .. } => inputs.len(),
                _ => 0,
            };
            compact_amo_encoding(size)
        } else {
            algorithm
        };
        if let SatEncodingDecision::CountRelation {
            inputs,
            relation,
            bound,
            amo_encoding,
            ..
        } = decision
            && amo_encoding.is_none()
            && count_uses_amo(inputs, *relation, *bound)
        {
            *amo_encoding = Some(EncodingSelection {
                algorithm,
                provenance,
            });
        }
        if let SatEncodingDecision::AtMostOne { encoding, .. }
        | SatEncodingDecision::AllDifferent {
            amo_encoding: encoding,
            encoding:
                Some(EncodingSelection {
                    algorithm: AllDifferentEncoding::ValueAmo,
                    ..
                }),
            ..
        } = decision
            && encoding.is_none()
        {
            *encoding = Some(EncodingSelection {
                algorithm,
                provenance,
            });
        }
    }
}

fn compact_amo_encoding(size: usize) -> AmoEncoding {
    if size <= 5 {
        AmoEncoding::Pairwise
    } else {
        AmoEncoding::Ladder
    }
}

fn resolve_cardinality_choices(decisions: &mut [SatEncodingDecision]) {
    use crate::settings;
    if !decisions.iter().any(|decision| {
        matches!(
            decision,
            SatEncodingDecision::Cardinality { encoding: None, .. }
                | SatEncodingDecision::CountRelation { encoding: None, .. }
        )
    }) {
        return;
    }
    let (algorithm, provenance) = select_encoding(
        settings::cardinality_encoding(),
        &CardinalityEncoding::ALL,
        &CardinalityEncoding::LABELS,
        || 0,
    );
    for decision in decisions {
        if let SatEncodingDecision::Cardinality { encoding, .. }
        | SatEncodingDecision::CountRelation { encoding, .. } = decision
            && encoding.is_none()
        {
            *encoding = Some(EncodingSelection {
                algorithm,
                provenance,
            });
        }
    }
}

fn resolve_pb_choices(decisions: &mut [SatEncodingDecision]) {
    use crate::settings;
    let unresolved: Vec<_> = decisions
        .iter()
        .filter_map(|decision| match decision {
            SatEncodingDecision::PseudoBoolean {
                terms,
                encoding: None,
                ..
            }
            | SatEncodingDecision::IntegerRelation {
                terms,
                encoding: None,
                ..
            } => Some(terms),
            SatEncodingDecision::Objective {
                value,
                encoding: None,
                ..
            } => Some(&value.terms),
            SatEncodingDecision::AllDifferent {
                inputs,
                except,
                pb_encoding: None,
                ..
            } => inputs
                .iter()
                .chain(except.iter())
                .max_by_key(|input| {
                    input
                        .terms
                        .iter()
                        .map(|(weight, _)| i128::from(*weight).abs())
                        .sum::<i128>()
                })
                .map(|input| &input.terms),
            SatEncodingDecision::Table {
                inputs,
                row_views,
                pb_encoding: None,
                ..
            } => inputs
                .iter()
                .chain(row_views.iter().flatten().flatten())
                .max_by_key(|input| {
                    input
                        .terms
                        .iter()
                        .map(|(weight, _)| i128::from(*weight).abs())
                        .sum::<i128>()
                })
                .map(|input| &input.terms),
            SatEncodingDecision::Element {
                index_view,
                value,
                entries,
                pb_encoding: None,
                ..
            } => std::iter::once(index_view)
                .chain(std::iter::once(value))
                .chain(entries.iter().map(|(_, entry)| entry))
                .max_by_key(|view| {
                    view.terms
                        .iter()
                        .map(|(weight, _)| i128::from(*weight).abs())
                        .sum::<i128>()
                })
                .map(|view| &view.terms),
            _ => None,
        })
        .collect();
    if unresolved.is_empty() {
        return;
    }
    let (algorithm, provenance) = select_encoding(
        settings::pb_encoding(),
        &PbEncoding::ALL,
        &PbEncoding::LABELS,
        || {
            usize::from(unresolved.iter().any(|terms| {
                terms
                    .iter()
                    .map(|(weight, _)| i128::from(*weight).abs())
                    .sum::<i128>()
                    > 4096
            }))
        },
    );
    for decision in decisions {
        if let SatEncodingDecision::PseudoBoolean { encoding, .. }
        | SatEncodingDecision::IntegerRelation { encoding, .. }
        | SatEncodingDecision::Objective { encoding, .. }
        | SatEncodingDecision::AllDifferent {
            pb_encoding: encoding,
            ..
        }
        | SatEncodingDecision::Table {
            pb_encoding: encoding,
            ..
        }
        | SatEncodingDecision::Element {
            pb_encoding: encoding,
            ..
        } = decision
            && encoding.is_none()
        {
            *encoding = Some(EncodingSelection {
                algorithm,
                provenance,
            });
        }
    }
}
