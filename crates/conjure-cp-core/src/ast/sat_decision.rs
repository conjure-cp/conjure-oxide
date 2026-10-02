//! Semantic SAT encoding decisions. Clauses and solver literals belong to the adaptor.
use crate::ast::Expression;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use uniplate::Uniplate;

/// A Boolean operation whose truth is asserted or bound to a named output.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Uniplate)]
#[biplate(to=Expression)]
#[biplate(to=crate::ast::DeclarationPtr)]
pub enum SatEncodingDecision {
    /// Assert the semantic Boolean expression using a Tseitin encoding.
    Assert(Expression),
    /// At most one input may be true; the selected library algorithm is recorded explicitly.
    AtMostOne {
        inputs: Vec<Expression>,
        encoding: Option<EncodingSelection<AmoEncoding>>,
    },
    /// Compare the number of true operands with a mathematical integer bound.
    Cardinality {
        inputs: Vec<Expression>,
        relation: CardinalityRelation,
        bound: i64,
        encoding: Option<EncodingSelection<CardinalityEncoding>>,
    },
    /// Compare a signed weighted Boolean sum with an integer bound.
    PseudoBoolean {
        terms: Vec<(i64, Expression)>,
        relation: CardinalityRelation,
        bound: i64,
        encoding: Option<EncodingSelection<PbEncoding>>,
    },
    /// Define an output as equivalent to a semantic Boolean expression.
    Boolean {
        output: Expression,
        expression: Expression,
    },
}

impl Display for SatEncodingDecision {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AtMostOne { inputs, encoding } => write!(
                f,
                "at-most-one({}) using {:?}",
                inputs
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", "),
                encoding
            ),
            Self::Cardinality {
                inputs,
                relation,
                bound,
                encoding,
            } => write!(
                f,
                "count({}) {:?} {bound} using {encoding:?}",
                inputs
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", "),
                relation
            ),
            Self::PseudoBoolean {
                terms,
                relation,
                bound,
                encoding,
            } => write!(
                f,
                "weighted({}) {relation:?} {bound} using {encoding:?}",
                terms
                    .iter()
                    .map(|(weight, input)| format!("{weight}*{input}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Assert(expression) => write!(f, "assert({expression}) using tseitin"),
            Self::Boolean { output, expression } => {
                write!(f, "define({output} <-> {expression}) using tseitin")
            }
        }
    }
}

impl SatEncodingDecision {
    /// Semantic expressions held by this decision.
    pub fn expressions(&self) -> Vec<&Expression> {
        match self {
            Self::AtMostOne { inputs, .. } | Self::Cardinality { inputs, .. } => {
                inputs.iter().collect()
            }
            Self::PseudoBoolean { terms, .. } => terms.iter().map(|(_, input)| input).collect(),
            Self::Assert(expression) => vec![expression],
            Self::Boolean { output, expression } => vec![output, expression],
        }
    }
}

/// The origin of a resolved compilation choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SelectionProvenance {
    ExplicitConfiguration,
    Heuristic,
}

/// A resolved algorithm and the source of that choice, reusable across encoding classes.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EncodingSelection<T> {
    pub algorithm: T,
    pub provenance: SelectionProvenance,
}

/// RustSAT algorithms available for asserted at-most-one constraints.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AmoEncoding {
    Pairwise,
    Ladder,
    Bitwise,
    Commander,
    Bimander,
    TwoProduct,
}
impl AmoEncoding {
    pub const ALL: [Self; 6] = [
        Self::Pairwise,
        Self::Ladder,
        Self::Bitwise,
        Self::Commander,
        Self::Bimander,
        Self::TwoProduct,
    ];
    pub const LABELS: [&'static str; 6] = [
        "pairwise",
        "ladder",
        "bitwise",
        "commander",
        "bimander",
        "two-product",
    ];
}
impl Display for AmoEncoding {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            Self::LABELS[Self::ALL
                .iter()
                .position(|algorithm| algorithm == self)
                .unwrap()],
        )
    }
}
impl std::str::FromStr for AmoEncoding {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, String> {
        Self::LABELS
            .iter()
            .position(|label| *label == value)
            .map(|index| Self::ALL[index])
            .ok_or_else(|| {
                format!(
                    "unknown AMO encoding '{value}'; expected {}",
                    Self::LABELS.join(", ")
                )
            })
    }
}

/// Resolve unpinned encoding classes once per model through the configured heuristic.
pub fn resolve_encoding_choices(decisions: &mut [SatEncodingDecision]) {
    resolve_pb_choices(decisions);
    resolve_cardinality_choices(decisions);
    resolve_amo_choices(decisions);
}
fn resolve_amo_choices(decisions: &mut [SatEncodingDecision]) {
    use crate::settings::{self, Heuristic};
    let largest = decisions
        .iter()
        .filter_map(|decision| match decision {
            SatEncodingDecision::AtMostOne {
                inputs,
                encoding: None,
            } => Some(inputs.len()),
            _ => None,
        })
        .max();
    let Some(largest) = largest else {
        return;
    };
    let (algorithm, provenance) = if let Some(algorithm) = settings::amo_encoding() {
        (algorithm, SelectionProvenance::ExplicitConfiguration)
    } else {
        let index = match settings::heuristic() {
            Heuristic::First => 0,
            Heuristic::Compact => {
                if largest <= 5 {
                    0
                } else {
                    1
                }
            }
            Heuristic::Random => settings::next_heuristic_random_index(AmoEncoding::ALL.len()),
            Heuristic::Interactive => {
                settings::next_heuristic_interactive_index(&AmoEncoding::LABELS)
            }
            Heuristic::All => settings::next_heuristic_all_index(&AmoEncoding::LABELS),
        };
        (AmoEncoding::ALL[index], SelectionProvenance::Heuristic)
    };
    for decision in decisions {
        if let SatEncodingDecision::AtMostOne { encoding, .. } = decision
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
mod tests {
    use super::*;
    use crate::settings::{self, Heuristic};
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

/// Bound direction for a semantic cardinality constraint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CardinalityRelation {
    AtMost,
    AtLeast,
    Exactly,
}
/// Provider and algorithm for a cardinality constraint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CardinalityEncoding {
    RustsatTotalizer,
    PindakaasSortingNetwork,
}
impl CardinalityEncoding {
    pub const ALL: [Self; 2] = [Self::RustsatTotalizer, Self::PindakaasSortingNetwork];
    pub const LABELS: [&'static str; 2] = ["rustsat-totalizer", "pindakaas-sorting-network"];
}
impl Display for CardinalityEncoding {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(Self::LABELS[Self::ALL.iter().position(|value| value == self).unwrap()])
    }
}
impl std::str::FromStr for CardinalityEncoding {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, String> {
        Self::LABELS
            .iter()
            .position(|label| *label == value)
            .map(|index| Self::ALL[index])
            .ok_or_else(|| {
                format!(
                    "unknown cardinality encoder '{value}'; expected {}",
                    Self::LABELS.join(", ")
                )
            })
    }
}
fn resolve_cardinality_choices(decisions: &mut [SatEncodingDecision]) {
    use crate::settings::{self, Heuristic};
    if !decisions.iter().any(|decision| {
        matches!(
            decision,
            SatEncodingDecision::Cardinality { encoding: None, .. }
        )
    }) {
        return;
    }
    let (algorithm, provenance) = if let Some(algorithm) = settings::cardinality_encoding() {
        (algorithm, SelectionProvenance::ExplicitConfiguration)
    } else {
        let index = match settings::heuristic() {
            Heuristic::First | Heuristic::Compact => 0,
            Heuristic::Random => settings::next_heuristic_random_index(2),
            Heuristic::Interactive => {
                settings::next_heuristic_interactive_index(&CardinalityEncoding::LABELS)
            }
            Heuristic::All => settings::next_heuristic_all_index(&CardinalityEncoding::LABELS),
        };
        (
            CardinalityEncoding::ALL[index],
            SelectionProvenance::Heuristic,
        )
    };
    for decision in decisions {
        if let SatEncodingDecision::Cardinality { encoding, .. } = decision
            && encoding.is_none()
        {
            *encoding = Some(EncodingSelection {
                algorithm,
                provenance,
            });
        }
    }
}

/// Provider and algorithm for signed weighted pseudo-Boolean constraints.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PbEncoding {
    RustsatGeneralizedTotalizer,
    RustsatBinaryAdder,
    PindakaasBdd,
    RustsatDynamicPolyWatchdog,
    PindakaasSwc,
}
impl PbEncoding {
    pub const ALL: [Self; 5] = [
        Self::RustsatGeneralizedTotalizer,
        Self::RustsatBinaryAdder,
        Self::PindakaasBdd,
        Self::RustsatDynamicPolyWatchdog,
        Self::PindakaasSwc,
    ];
    pub const LABELS: [&'static str; 5] = [
        "rustsat-generalized-totalizer",
        "rustsat-binary-adder",
        "pindakaas-bdd",
        "rustsat-dynamic-poly-watchdog",
        "pindakaas-swc",
    ];
}
impl Display for PbEncoding {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(Self::LABELS[Self::ALL.iter().position(|value| value == self).unwrap()])
    }
}
impl std::str::FromStr for PbEncoding {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, String> {
        Self::LABELS
            .iter()
            .position(|label| *label == value)
            .map(|index| Self::ALL[index])
            .ok_or_else(|| {
                format!(
                    "unknown pseudo-Boolean encoder '{value}'; expected {}",
                    Self::LABELS.join(", ")
                )
            })
    }
}
fn resolve_pb_choices(decisions: &mut [SatEncodingDecision]) {
    use crate::settings::{self, Heuristic};
    let unresolved: Vec<_> = decisions
        .iter()
        .filter_map(|decision| match decision {
            SatEncodingDecision::PseudoBoolean {
                terms,
                encoding: None,
                ..
            } => Some(terms),
            _ => None,
        })
        .collect();
    if unresolved.is_empty() {
        return;
    }
    let (algorithm, provenance) = if let Some(algorithm) = settings::pb_encoding() {
        (algorithm, SelectionProvenance::ExplicitConfiguration)
    } else {
        let index = match settings::heuristic() {
            Heuristic::First => 0,
            // Large coefficient sums are better handled by a binary adder.
            Heuristic::Compact => usize::from(unresolved.iter().any(|terms| {
                terms
                    .iter()
                    .map(|(weight, _)| i128::from(*weight).abs())
                    .sum::<i128>()
                    > 4096
            })),
            Heuristic::Random => settings::next_heuristic_random_index(PbEncoding::ALL.len()),
            Heuristic::Interactive => {
                settings::next_heuristic_interactive_index(&PbEncoding::LABELS)
            }
            Heuristic::All => settings::next_heuristic_all_index(&PbEncoding::LABELS),
        };
        (PbEncoding::ALL[index], SelectionProvenance::Heuristic)
    };
    for decision in decisions {
        if let SatEncodingDecision::PseudoBoolean { encoding, .. } = decision
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
