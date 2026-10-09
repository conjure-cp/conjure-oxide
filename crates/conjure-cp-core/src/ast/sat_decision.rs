//! Semantic SAT encoding decisions. Clauses and solver literals belong to the adaptor.
use crate::ast::Expression;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use uniplate::Uniplate;

// One listing defines each family's labels, parsing and heuristic iteration order.
macro_rules! encoding_labels {
    ($type:ident, $family:literal, $len:literal, [$($variant:ident => $label:literal),+ $(,)?]) => {
        impl $type {
            /// Algorithms in heuristic and command-line order.
            pub const ALL: [Self; $len] = [$(Self::$variant),+];
            /// Canonical command-line labels in the same order as `ALL`.
            pub const LABELS: [&'static str; $len] = [$($label),+];
        }
        impl Display for $type {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                f.write_str(match self { $(Self::$variant => $label),+ })
            }
        }
        impl std::str::FromStr for $type {
            type Err = String;
            fn from_str(value: &str) -> Result<Self, String> {
                match value {
                    $($label => Ok(Self::$variant),)+
                    _ => Err(format!(concat!("unknown ", $family, " '{}'; expected {}"), value, Self::LABELS.join(", "))),
                }
            }
        }
    };
}

/// A Boolean operation whose truth is asserted or bound to a named output.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Uniplate)]
#[biplate(to=Expression)]
#[biplate(to=crate::ast::DeclarationPtr)]
pub enum SatEncodingDecision {
    /// Assert the semantic Boolean expression; the adaptor chooses clauses and witnesses.
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
    /// Define the truth of a Boolean count comparison using cardinality and AMO providers.
    CountRelation {
        output: Expression,
        inputs: Vec<Expression>,
        relation: IntegerRelation,
        bound: i64,
        encoding: Option<EncodingSelection<CardinalityEncoding>>,
        /// Selected when either implication includes an at-most-one threshold.
        amo_encoding: Option<EncodingSelection<AmoEncoding>>,
    },
    /// Compare a signed weighted Boolean sum with an integer bound.
    PseudoBoolean {
        terms: Vec<(i64, Expression)>,
        /// Relationships already guaranteed by the integer representation constraints.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        groups: Vec<PbTermGroup>,
        relation: CardinalityRelation,
        bound: i64,
        encoding: Option<EncodingSelection<PbEncoding>>,
    },
    /// Define an integer relation over a signed weighted view of the numeric operands.
    IntegerRelation {
        output: Expression,
        terms: Vec<(i64, Expression)>,
        groups: Vec<PbTermGroup>,
        relation: IntegerRelation,
        bound: i64,
        encoding: Option<EncodingSelection<PbEncoding>>,
    },
    /// Minimise or maximise an actual-value integer view using the selected PB provider.
    Objective {
        minimise: bool,
        value: SatIntegerView,
        encoding: Option<EncodingSelection<PbEncoding>>,
    },
    /// Define scalar or whole-value distinctness, with an optional repeated exception value.
    AllDifferent {
        output: Expression,
        inputs: Vec<SatIntegerView>,
        /// Pair conditions for compound values, lowered through the existing equality rules.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        comparisons: Option<Vec<Expression>>,
        /// This value may occur repeatedly.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        except: Option<SatIntegerView>,
        encoding: Option<EncodingSelection<AllDifferentEncoding>>,
        amo_encoding: Option<EncodingSelection<AmoEncoding>>,
        pb_encoding: Option<EncodingSelection<PbEncoding>>,
    },
    /// Define membership in an integer/Boolean relation.
    Table {
        output: Expression,
        inputs: Vec<SatIntegerView>,
        rows: Vec<Vec<i64>>,
        /// Non-constant relation cells retain their numeric representation views.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        row_views: Option<Vec<Vec<SatIntegerView>>>,
        negative: bool,
        encoding: Option<EncodingSelection<TableEncoding>>,
        pb_encoding: Option<EncodingSelection<PbEncoding>>,
    },
    /// Define a selected scalar value; out-of-domain indices leave it unconstrained.
    Element {
        index_view: SatIntegerView,
        value: SatIntegerView,
        entries: Vec<(i64, SatIntegerView)>,
        encoding: Option<EncodingSelection<ElementEncoding>>,
        pb_encoding: Option<EncodingSelection<PbEncoding>>,
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
            Self::CountRelation {
                output,
                inputs,
                relation,
                bound,
                encoding,
                amo_encoding,
            } => write!(
                f,
                "define({output} <-> count({}) {relation:?} {bound}) using {encoding:?}, AMO {amo_encoding:?}",
                inputs
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
            Self::PseudoBoolean {
                terms,
                groups,
                relation,
                bound,
                encoding,
            } => {
                write!(
                    f,
                    "weighted({}) {relation:?} {bound} using {encoding:?}",
                    terms
                        .iter()
                        .map(|(weight, input)| format!("{weight}*{input}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )?;
                if !groups.is_empty() {
                    write!(f, " with {groups:?}")?;
                }
                Ok(())
            }
            Self::IntegerRelation {
                output,
                terms,
                groups,
                relation,
                bound,
                encoding,
            } => write!(
                f,
                "define({output} <-> integer-weighted({}) {relation:?} {bound}) using {encoding:?} with {groups:?}",
                terms
                    .iter()
                    .map(|(weight, input)| format!("{weight}*{input}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::AllDifferent {
                output,
                inputs,
                except,
                comparisons,
                encoding,
                amo_encoding,
                pb_encoding,
            } => {
                let inputs = inputs
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                let constraint = if let Some(comparisons) = comparisons {
                    format!(
                        "allDifferentComparisons({})",
                        comparisons
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                } else {
                    match except {
                        Some(value) if value.terms.is_empty() => {
                            format!("allDifferentExcept({inputs}, {})", value.constant)
                        }
                        Some(value) => format!("allDifferentExcept({inputs}, {value})"),
                        None => format!("allDifferent({inputs})"),
                    }
                };
                write!(
                    f,
                    "define({output} <-> {constraint}) using {encoding:?}, AMO {amo_encoding:?}, PB {pb_encoding:?}"
                )
            }
            Self::Table {
                output,
                inputs,
                rows,
                row_views,
                negative,
                encoding,
                pb_encoding,
            } => write!(
                f,
                "define({output} <-> table({}, {}, negative={negative})) using {encoding:?}, PB {pb_encoding:?}",
                inputs
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", "),
                row_views
                    .as_ref()
                    .map_or_else(|| format!("{rows:?}"), |views| format!("{views:?}"))
            ),
            Self::Element {
                index_view,
                value,
                entries,
                encoding,
                pb_encoding,
            } => write!(
                f,
                "element(index={index_view}, value={value}, entries=[{}]) using {encoding:?}, PB {pb_encoding:?}",
                entries
                    .iter()
                    .map(|(label, view)| format!("{label}: {view}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Objective {
                minimise,
                value,
                encoding,
            } => write!(
                f,
                "optimise(minimise={minimise}, value={value}) using {encoding:?}"
            ),
            Self::Assert(expression) => write!(f, "assert({expression}) using tseitin"),
            Self::Boolean { output, expression } => {
                write!(f, "define({output} <-> {expression}) using tseitin")
            }
        }
    }
}

/// Numeric relation between a weighted integer view and its bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Uniplate)]
pub enum IntegerRelation {
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

/// A contiguous group of weighted terms with an existing representation invariant.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Uniplate)]
pub struct PbTermGroup {
    /// First term index, inclusive.
    pub start: usize,
    /// Last term index, exclusive.
    pub end: usize,
    /// The relationship between these terms, independent of library literals.
    pub structure: PbTermStructure,
}

/// Representation structure available to a pseudo-Boolean encoder.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Uniplate)]
pub enum PbTermStructure {
    /// At most one Boolean input in the group is true.
    Choice,
    /// Each Boolean input is implied by the next input in the group.
    Chain,
    /// Binary terms bounded by the representation's inclusive domain.
    BoundedBinary { lower: i64, upper: i64 },
}

impl SatEncodingDecision {
    /// Semantic expressions held by this decision.
    pub fn expressions(&self) -> Vec<&Expression> {
        match self {
            Self::AtMostOne { inputs, .. } | Self::Cardinality { inputs, .. } => {
                inputs.iter().collect()
            }
            Self::Objective { value, .. } => value
                .terms
                .iter()
                .map(|(_, expression)| expression)
                .collect(),
            Self::CountRelation { output, inputs, .. } => {
                std::iter::once(output).chain(inputs).collect()
            }
            Self::PseudoBoolean { terms, .. } => terms.iter().map(|(_, input)| input).collect(),
            Self::IntegerRelation { output, terms, .. } => std::iter::once(output)
                .chain(terms.iter().map(|(_, input)| input))
                .collect(),
            Self::AllDifferent {
                output,
                inputs,
                except,
                comparisons,
                ..
            } => std::iter::once(output)
                .chain(comparisons.iter().flatten())
                .chain(inputs.iter().chain(except.iter()).flat_map(|input| {
                    input
                        .terms
                        .iter()
                        .map(|(_, term)| term)
                        .chain(input.choices.iter().flatten().map(|(_, term)| term))
                }))
                .collect(),
            Self::Table {
                output,
                inputs,
                row_views,
                ..
            } => std::iter::once(output)
                .chain(
                    inputs
                        .iter()
                        .chain(row_views.iter().flatten().flatten())
                        .flat_map(|input| {
                            input
                                .terms
                                .iter()
                                .map(|(_, term)| term)
                                .chain(input.choices.iter().flatten().map(|(_, term)| term))
                        }),
                )
                .collect(),
            Self::Element {
                index_view,
                value,
                entries,
                ..
            } => std::iter::once(index_view)
                .chain(std::iter::once(value))
                .chain(entries.iter().map(|(_, entry)| entry))
                .flat_map(|view| {
                    view.terms
                        .iter()
                        .map(|(_, term)| term)
                        .chain(view.choices.iter().flatten().map(|(_, term)| term))
                })
                .collect(),
            Self::Assert(expression) => vec![expression],
            Self::Boolean { output, expression } => vec![output, expression],
        }
    }
}

/// A numeric view whose representation constraints remain independently enforced.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Uniplate)]
#[biplate(to=Expression)]
#[biplate(to=crate::ast::DeclarationPtr)]
pub struct SatIntegerView {
    /// Constant contribution to the actual numeric value.
    pub constant: i64,
    /// Signed weights over Boolean representation operands.
    pub terms: Vec<(i64, Expression)>,
    /// Independently guaranteed representation structure.
    pub groups: Vec<PbTermGroup>,
    /// Actual values and their exactly-one indicators, when available.
    pub choices: Option<Vec<(i64, Expression)>>,
}

impl Display for SatIntegerView {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}+weighted({}) with {:?}",
            self.constant,
            self.terms
                .iter()
                .map(|(weight, input)| format!("{weight}*{input}"))
                .collect::<Vec<_>>()
                .join(", "),
            self.groups
        )?;
        if let Some(choices) = &self.choices {
            write!(
                f,
                " choices({})",
                choices
                    .iter()
                    .map(|(value, input)| format!("{value}:{input}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )?;
        }
        Ok(())
    }
}

/// Library compositions available for element.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ElementEncoding {
    /// Each valid index selector implies its scalar equality.
    Implication,
    /// One support disjunction, with an explicit out-of-domain alternative.
    Support,
}
encoding_labels!(ElementEncoding, "element encoding", 2, [
    Implication => "implication",
    Support => "support",
]);

/// Library compositions available for table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TableEncoding {
    /// Disjunction of matching rows, sharing each numeric cell comparison.
    Tuple,
    /// Reduced layered decision diagram, sharing equal suffix relations.
    Mdd,
    /// Bidirectional value supports for constant binary relations.
    BinarySupport,
}
encoding_labels!(TableEncoding, "table encoding", 3, [
    Tuple => "tuple",
    Mdd => "mdd",
    BinarySupport => "binary-support",
]);

/// Library compositions available for allDifferent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AllDifferentEncoding {
    /// Pairwise numeric disequalities, for every numeric representation.
    Pairwise,
    /// AMO over equal-value indicators; requires choice views for all operands.
    ValueAmo,
}
encoding_labels!(AllDifferentEncoding, "allDifferent encoding", 2, [
    Pairwise => "pairwise",
    ValueAmo => "value-amo",
]);

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

/// Library algorithms available for asserted and reified at-most-one constraints.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AmoEncoding {
    Pairwise,
    Ladder,
    Bitwise,
    Commander,
    Bimander,
    TwoProduct,
    PindakaasPairwise,
    PindakaasLadder,
    PindakaasBitwise,
}
encoding_labels!(AmoEncoding, "AMO encoding", 9, [
    Pairwise => "pairwise",
    Ladder => "ladder",
    Bitwise => "bitwise",
    Commander => "commander",
    Bimander => "bimander",
    TwoProduct => "two-product",
    PindakaasPairwise => "pindakaas-pairwise",
    PindakaasLadder => "pindakaas-ladder",
    PindakaasBitwise => "pindakaas-bitwise",
]);

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
encoding_labels!(CardinalityEncoding, "cardinality encoder", 2, [
    RustsatTotalizer => "rustsat-totalizer",
    PindakaasSortingNetwork => "pindakaas-sorting-network",
]);
/// Provider and algorithm for signed weighted pseudo-Boolean constraints.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PbEncoding {
    RustsatGeneralizedTotalizer,
    RustsatBinaryAdder,
    PindakaasBdd,
    RustsatDynamicPolyWatchdog,
    PindakaasSwc,
}
encoding_labels!(PbEncoding, "pseudo-Boolean encoder", 5, [
    RustsatGeneralizedTotalizer => "rustsat-generalized-totalizer",
    RustsatBinaryAdder => "rustsat-binary-adder",
    PindakaasBdd => "pindakaas-bdd",
    RustsatDynamicPolyWatchdog => "rustsat-dynamic-poly-watchdog",
    PindakaasSwc => "pindakaas-swc",
]);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn term_groups_round_trip_and_old_flat_decisions_remain_readable() {
        let mut decision = SatEncodingDecision::PseudoBoolean {
            terms: vec![(1, true.into()), (2, false.into())],
            groups: vec![PbTermGroup {
                start: 0,
                end: 2,
                structure: PbTermStructure::BoundedBinary { lower: 0, upper: 2 },
            }],
            relation: CardinalityRelation::AtMost,
            bound: 2,
            encoding: None,
        };
        let mut value = serde_json::to_value(&decision).unwrap();
        assert_eq!(
            serde_json::from_value::<SatEncodingDecision>(value.clone()).unwrap(),
            decision
        );
        value["PseudoBoolean"]
            .as_object_mut()
            .unwrap()
            .remove("groups");
        if let SatEncodingDecision::PseudoBoolean { groups, .. } = &mut decision {
            groups.clear();
        }
        assert_eq!(
            serde_json::from_value::<SatEncodingDecision>(value).unwrap(),
            decision
        );
    }
    #[test]
    fn alldifferent_display_is_independent_of_internal_declaration_identity() {
        use crate::ast::{DeclarationPtr, Domain, Name, Reference};
        let view = || {
            let input: Expression = Reference::new(DeclarationPtr::new_find(
                Name::User("same".into()),
                Domain::bool(),
            ))
            .into();
            SatIntegerView {
                constant: 0,
                terms: vec![(1, input.clone())],
                groups: vec![],
                choices: Some(vec![(1, input)]),
            }
        };
        assert_eq!(view().to_string(), view().to_string());
    }
}
