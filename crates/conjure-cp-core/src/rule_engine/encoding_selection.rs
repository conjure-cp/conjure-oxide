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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{self, Heuristic};
    #[test]
    fn compact_amo_choices_are_local_but_pins_cover_the_family() {
        let build = |size| SatEncodingDecision::AtMostOne {
            inputs: vec![true.into(); size],
            encoding: None,
        };
        settings::set_heuristic(Heuristic::Compact);
        settings::set_amo_encoding(None);
        let mut decisions = vec![build(3), build(20)];
        resolve_encoding_choices(&mut decisions);
        for (decision, algorithm) in decisions
            .iter()
            .zip([AmoEncoding::Pairwise, AmoEncoding::Ladder])
        {
            assert!(
                matches!(decision, SatEncodingDecision::AtMostOne { encoding: Some(EncodingSelection { algorithm: actual, provenance: SelectionProvenance::Heuristic }), .. } if *actual == algorithm)
            );
        }
        settings::set_amo_encoding(Some(AmoEncoding::Bitwise));
        let mut decisions = vec![build(3), build(20)];
        resolve_encoding_choices(&mut decisions);
        assert!(decisions.iter().all(|decision| matches!(
            decision,
            SatEncodingDecision::AtMostOne {
                encoding: Some(EncodingSelection {
                    algorithm: AmoEncoding::Bitwise,
                    provenance: SelectionProvenance::ExplicitConfiguration
                }),
                ..
            }
        )));
        settings::set_amo_encoding(None);
    }
    #[test]
    fn cardinality_choices_are_shared_and_explicit_pins_take_precedence() {
        let decision = || SatEncodingDecision::Cardinality {
            inputs: vec![true.into(); 4],
            relation: CardinalityRelation::Exactly,
            bound: 2,
            encoding: None,
        };
        settings::set_heuristic(Heuristic::All);
        settings::set_cardinality_encoding(None);
        settings::begin_heuristic_all_choices(vec![1]);
        let mut decisions = vec![decision(), decision()];
        resolve_encoding_choices(&mut decisions);
        assert_eq!(settings::heuristic_all_choices().len(), 1);
        for entry in &decisions {
            assert!(matches!(
                entry,
                SatEncodingDecision::Cardinality {
                    encoding: Some(EncodingSelection {
                        algorithm: CardinalityEncoding::PindakaasSortingNetwork,
                        provenance: SelectionProvenance::Heuristic,
                    }),
                    ..
                }
            ));
        }
        settings::begin_heuristic_all_choices(vec![]);
        settings::set_cardinality_encoding(Some(CardinalityEncoding::RustsatTotalizer));
        let mut pinned = vec![decision(), decisions.remove(0)];
        resolve_encoding_choices(&mut pinned);
        assert!(settings::heuristic_all_choices().is_empty());
        assert!(matches!(
            &pinned[0],
            SatEncodingDecision::Cardinality {
                encoding: Some(EncodingSelection {
                    algorithm: CardinalityEncoding::RustsatTotalizer,
                    provenance: SelectionProvenance::ExplicitConfiguration,
                }),
                ..
            }
        ));
        assert!(matches!(
            &pinned[1],
            SatEncodingDecision::Cardinality {
                encoding: Some(EncodingSelection {
                    algorithm: CardinalityEncoding::PindakaasSortingNetwork,
                    ..
                }),
                ..
            }
        ));
        settings::set_cardinality_encoding(None);
        settings::set_heuristic(Heuristic::Compact);
    }
    #[test]
    fn count_relation_choices_share_providers_and_fold_constant_thresholds() {
        use crate::ast::{DeclarationPtr, Domain, Name, Reference};
        let a: Expression = Reference::new(DeclarationPtr::new_find(
            Name::user("count_choice"),
            Domain::bool(),
        ))
        .into();
        let build = || SatEncodingDecision::CountRelation {
            output: a.clone(),
            inputs: vec![true.into(), a.clone(), a.clone()],
            relation: IntegerRelation::Equal,
            bound: 2,
            encoding: None,
            amo_encoding: None,
        };
        assert!(count_uses_amo(
            &[true.into(), a.clone(), a.clone()],
            IntegerRelation::Equal,
            2
        ));
        assert!(!count_uses_amo(
            &[true.into(), a.clone()],
            IntegerRelation::Equal,
            2
        ));
        assert!(!count_uses_amo(
            &[a.clone(), a.clone()],
            IntegerRelation::LessEqual,
            i64::MIN
        ));
        assert!(!count_uses_amo(
            &[a.clone(), a.clone()],
            IntegerRelation::LessEqual,
            i64::MAX
        ));
        settings::set_heuristic(Heuristic::All);
        settings::set_amo_encoding(None);
        settings::set_cardinality_encoding(None);
        for (amo_index, amo) in AmoEncoding::ALL.into_iter().enumerate() {
            for (card_index, card) in CardinalityEncoding::ALL.into_iter().enumerate() {
                settings::begin_heuristic_all_choices(vec![card_index, amo_index]);
                let mut decisions = vec![build(), build()];
                resolve_encoding_choices(&mut decisions);
                assert_eq!(settings::heuristic_all_choices().len(), 2);
                for decision in decisions {
                    assert!(matches!(decision, SatEncodingDecision::CountRelation {
                        encoding: Some(EncodingSelection { algorithm, provenance: SelectionProvenance::Heuristic }),
                        amo_encoding: Some(EncodingSelection { algorithm: actual_amo, provenance: SelectionProvenance::Heuristic }), ..
                    } if algorithm == card && actual_amo == amo));
                }
            }
        }
        settings::set_amo_encoding(Some(AmoEncoding::Ladder));
        settings::set_cardinality_encoding(Some(CardinalityEncoding::PindakaasSortingNetwork));
        settings::begin_heuristic_all_choices(vec![]);
        let mut decisions = vec![build()];
        resolve_encoding_choices(&mut decisions);
        let retained = decisions[0].clone();
        settings::set_amo_encoding(Some(AmoEncoding::Pairwise));
        settings::set_cardinality_encoding(Some(CardinalityEncoding::RustsatTotalizer));
        resolve_encoding_choices(&mut decisions);
        assert_eq!(decisions[0], retained);
        assert!(settings::heuristic_all_choices().is_empty());
        settings::set_amo_encoding(None);
        settings::set_cardinality_encoding(None);
        settings::set_heuristic(Heuristic::Compact);
    }

    fn decision() -> SatEncodingDecision {
        SatEncodingDecision::AtMostOne {
            inputs: vec![true.into(); 6],
            encoding: None,
        }
    }
    #[test]
    fn heuristic_all_enumerates_one_amo_choice_for_the_whole_model() {
        settings::set_heuristic(Heuristic::All);
        settings::set_amo_encoding(None);
        for (index, algorithm) in AmoEncoding::ALL.into_iter().enumerate() {
            settings::begin_heuristic_all_choices(vec![index]);
            let mut decisions = vec![decision(), decision()];
            resolve_encoding_choices(&mut decisions);
            assert_eq!(settings::heuristic_all_choices().len(), 1);
            for decision in decisions {
                assert!(
                    matches!(decision, SatEncodingDecision::AtMostOne { encoding: Some(EncodingSelection { algorithm: actual, provenance: SelectionProvenance::Heuristic }), .. } if actual == algorithm)
                );
            }
        }
        settings::set_heuristic(Heuristic::Compact);
        settings::begin_heuristic_all_choices(vec![]);
    }
    #[test]
    fn explicit_amo_configuration_overrides_the_heuristic() {
        settings::set_heuristic(Heuristic::All);
        settings::begin_heuristic_all_choices(vec![]);
        settings::set_amo_encoding(Some(AmoEncoding::Bitwise));
        let mut decisions = vec![decision()];
        resolve_encoding_choices(&mut decisions);
        assert!(settings::heuristic_all_choices().is_empty());
        assert!(matches!(
            &decisions[0],
            SatEncodingDecision::AtMostOne {
                encoding: Some(EncodingSelection {
                    algorithm: AmoEncoding::Bitwise,
                    provenance: SelectionProvenance::ExplicitConfiguration
                }),
                ..
            }
        ));
        settings::set_amo_encoding(None);
        settings::set_heuristic(Heuristic::Compact);
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

#[cfg(test)]
mod pb_choice_tests {
    use super::*;
    use crate::settings::{self, Heuristic};
    #[test]
    fn weighted_choices_are_shared_and_explicit_pins_take_precedence() {
        let decision = || SatEncodingDecision::PseudoBoolean {
            terms: vec![(3, true.into()), (-2, false.into())],
            groups: vec![],
            relation: CardinalityRelation::AtMost,
            bound: 1,
            encoding: None,
        };
        settings::set_heuristic(Heuristic::All);
        settings::set_pb_encoding(None);
        for (index, algorithm) in PbEncoding::ALL.into_iter().enumerate() {
            settings::begin_heuristic_all_choices(vec![index]);
            let mut decisions = vec![decision(), decision()];
            resolve_encoding_choices(&mut decisions);
            assert_eq!(settings::heuristic_all_choices().len(), 1);
            for entry in decisions {
                assert!(matches!(entry, SatEncodingDecision::PseudoBoolean {
                    encoding: Some(EncodingSelection { algorithm: selected, provenance: SelectionProvenance::Heuristic }), ..
                } if selected == algorithm));
            }
        }
        settings::begin_heuristic_all_choices(vec![]);
        settings::set_pb_encoding(Some(PbEncoding::RustsatBinaryAdder));
        let retained = EncodingSelection {
            algorithm: PbEncoding::PindakaasBdd,
            provenance: SelectionProvenance::Heuristic,
        };
        let mut decisions = vec![
            decision(),
            SatEncodingDecision::PseudoBoolean {
                terms: vec![],
                groups: vec![],
                relation: CardinalityRelation::Exactly,
                bound: 0,
                encoding: Some(retained.clone()),
            },
        ];
        resolve_encoding_choices(&mut decisions);
        assert!(settings::heuristic_all_choices().is_empty());
        assert!(matches!(
            &decisions[0],
            SatEncodingDecision::PseudoBoolean {
                encoding: Some(EncodingSelection {
                    algorithm: PbEncoding::RustsatBinaryAdder,
                    provenance: SelectionProvenance::ExplicitConfiguration
                }),
                ..
            }
        ));
        assert!(
            matches!(&decisions[1], SatEncodingDecision::PseudoBoolean { encoding: Some(selection), .. } if selection == &retained)
        );
        settings::set_pb_encoding(None);
        settings::set_heuristic(Heuristic::Compact);
    }
}

#[cfg(test)]
mod alldifferent_choice_tests {
    use super::*;
    use crate::settings::{self, Heuristic};

    #[test]
    fn alldifferent_choices_respect_views_and_explicit_provenance() {
        let choice = SatIntegerView {
            constant: 0,
            terms: vec![(1, true.into())],
            groups: vec![],
            choices: Some(vec![(1, true.into())]),
        };
        let build = |input: SatIntegerView| SatEncodingDecision::AllDifferent {
            output: true.into(),
            inputs: vec![input.clone(), input],
            except: None,
            comparisons: None,
            encoding: None,
            amo_encoding: None,
            pb_encoding: None,
        };
        settings::set_heuristic(Heuristic::Compact);
        settings::set_alldifferent_encoding(None);
        settings::set_amo_encoding(None);
        settings::set_pb_encoding(None);
        let mut decisions = vec![build(choice.clone())];
        resolve_encoding_choices(&mut decisions);
        assert!(matches!(
            &decisions[0],
            SatEncodingDecision::AllDifferent {
                encoding: Some(EncodingSelection {
                    algorithm: AllDifferentEncoding::ValueAmo,
                    provenance: SelectionProvenance::Heuristic
                }),
                amo_encoding: Some(_),
                pb_encoding: Some(_),
                ..
            }
        ));
        let mut numeric = choice.clone();
        numeric.choices = None;
        let mut decisions = vec![build(choice.clone()), build(numeric)];
        resolve_encoding_choices(&mut decisions);
        assert!(decisions.iter().all(|decision| matches!(
            decision,
            SatEncodingDecision::AllDifferent {
                encoding: Some(EncodingSelection {
                    algorithm: AllDifferentEncoding::Pairwise,
                    ..
                }),
                ..
            }
        )));
        let mut compound = build(choice.clone());
        if let SatEncodingDecision::AllDifferent { comparisons, .. } = &mut compound {
            *comparisons = Some(vec![true.into()]);
        }
        resolve_alldifferent_choices(std::slice::from_mut(&mut compound));
        assert!(matches!(
            compound,
            SatEncodingDecision::AllDifferent {
                encoding: Some(EncodingSelection {
                    algorithm: AllDifferentEncoding::Pairwise,
                    ..
                }),
                ..
            }
        ));
        settings::set_alldifferent_encoding(Some(AllDifferentEncoding::Pairwise));
        let mut decisions = vec![build(choice)];
        resolve_encoding_choices(&mut decisions);
        assert!(matches!(
            &decisions[0],
            SatEncodingDecision::AllDifferent {
                encoding: Some(EncodingSelection {
                    algorithm: AllDifferentEncoding::Pairwise,
                    provenance: SelectionProvenance::ExplicitConfiguration
                }),
                ..
            }
        ));
        settings::set_alldifferent_encoding(None);
    }
}

#[cfg(test)]
mod table_choice_tests {
    use super::*;
    use crate::settings::{self, Heuristic};
    #[test]
    fn compact_table_choices_are_local_and_explicit_requests_have_provenance() {
        let build = |size| SatEncodingDecision::Table {
            output: true.into(),
            inputs: vec![],
            rows: (0..size).map(|row| vec![row]).collect(),
            row_views: None,
            negative: false,
            encoding: None,
            pb_encoding: None,
        };
        settings::set_table_encoding(None);
        settings::set_heuristic(Heuristic::Compact);
        let mut decisions = vec![build(1), build(5)];
        resolve_encoding_choices(&mut decisions);
        for (decision, algorithm) in decisions
            .iter()
            .zip([TableEncoding::Tuple, TableEncoding::Mdd])
        {
            assert!(matches!(
                decision,
                SatEncodingDecision::Table {
                    encoding: Some(EncodingSelection {
                        algorithm: actual,
                        provenance: SelectionProvenance::Heuristic
                    }),
                    ..
                } if *actual == algorithm
            ));
        }
        settings::set_table_encoding(Some(TableEncoding::Tuple));
        let mut decisions = vec![build(1), build(5)];
        resolve_encoding_choices(&mut decisions);
        assert!(decisions.iter().all(|d| matches!(
            d,
            SatEncodingDecision::Table {
                encoding: Some(EncodingSelection {
                    algorithm: TableEncoding::Tuple,
                    provenance: SelectionProvenance::ExplicitConfiguration
                }),
                ..
            }
        )));
        settings::set_table_encoding(None);
        settings::set_heuristic(Heuristic::First);
    }

    #[test]
    fn dense_binary_tables_use_mdd_and_duplicate_rows_do_not_inflate_cost() {
        let dense = (0..8)
            .flat_map(|x| {
                (0..8)
                    .filter(move |y| (x + y) % 4 != 0)
                    .map(move |y| vec![x, y])
            })
            .collect::<Vec<_>>();
        assert_eq!(compact_table_encoding(&dense, None), TableEncoding::Mdd);
        assert_eq!(compact_table_encoding(&[], Some(48)), TableEncoding::Mdd);
        assert_eq!(compact_table_encoding(&[], Some(2)), TableEncoding::Tuple);
        assert_eq!(
            compact_table_encoding(&vec![vec![1, 2]; 10], None),
            TableEncoding::Tuple
        );
        let sparse = (0..8).map(|x| vec![x, x]).collect::<Vec<_>>();
        assert_eq!(compact_table_encoding(&sparse, None), TableEncoding::Mdd);
    }
}
#[cfg(test)]
mod element_choice_tests {
    use super::*;
    use crate::settings::{self, Heuristic};
    #[test]
    fn element_choices_are_shared_and_explicit_requests_have_provenance() {
        let build = |size| SatEncodingDecision::Element {
            index_view: SatIntegerView {
                constant: 0,
                terms: vec![],
                groups: vec![],
                choices: None,
            },
            value: SatIntegerView {
                constant: 0,
                terms: vec![],
                groups: vec![],
                choices: None,
            },
            entries: (0..size)
                .map(|label| {
                    (
                        label,
                        SatIntegerView {
                            constant: 0,
                            terms: vec![],
                            groups: vec![],
                            choices: None,
                        },
                    )
                })
                .collect(),
            encoding: None,
            pb_encoding: None,
        };
        settings::set_element_encoding(None);
        settings::set_heuristic(Heuristic::Compact);
        let mut decisions = vec![build(1), build(5)];
        resolve_encoding_choices(&mut decisions);
        assert!(decisions.iter().all(|d| matches!(
            d,
            SatEncodingDecision::Element {
                encoding: Some(EncodingSelection {
                    algorithm: ElementEncoding::Support,
                    provenance: SelectionProvenance::Heuristic
                }),
                ..
            }
        )));
        settings::set_element_encoding(Some(ElementEncoding::Implication));
        let mut decisions = vec![build(1), build(5)];
        resolve_encoding_choices(&mut decisions);
        assert!(decisions.iter().all(|d| matches!(
            d,
            SatEncodingDecision::Element {
                encoding: Some(EncodingSelection {
                    algorithm: ElementEncoding::Implication,
                    provenance: SelectionProvenance::ExplicitConfiguration
                }),
                ..
            }
        )));
        settings::set_element_encoding(None);
        settings::set_heuristic(Heuristic::First);
    }
}
