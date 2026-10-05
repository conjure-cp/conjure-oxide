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

/// Structure available to a pseudo-Boolean encoder, guaranteed elsewhere in the model.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Uniplate)]
pub enum PbTermStructure {
    /// At most one Boolean input in the group is true.
    Choice,
    /// Each Boolean input is implied by the next input in the group.
    Chain,
    /// Binary terms whose weighted contribution is within these inclusive bounds.
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
impl ElementEncoding {
    pub const ALL: [Self; 2] = [Self::Implication, Self::Support];
    pub const LABELS: [&'static str; 2] = ["implication", "support"];
}
impl Display for ElementEncoding {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(Self::LABELS[Self::ALL.iter().position(|value| value == self).unwrap()])
    }
}
impl std::str::FromStr for ElementEncoding {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, String> {
        Self::LABELS
            .iter()
            .position(|label| *label == value)
            .map(|index| Self::ALL[index])
            .ok_or_else(|| {
                format!(
                    "unknown element encoding '{value}'; expected {}",
                    Self::LABELS.join(", ")
                )
            })
    }
}

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
impl TableEncoding {
    pub const ALL: [Self; 3] = [Self::Tuple, Self::Mdd, Self::BinarySupport];
    pub const LABELS: [&'static str; 3] = ["tuple", "mdd", "binary-support"];
}
impl Display for TableEncoding {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(Self::LABELS[Self::ALL.iter().position(|value| value == self).unwrap()])
    }
}
impl std::str::FromStr for TableEncoding {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, String> {
        Self::LABELS
            .iter()
            .position(|label| *label == value)
            .map(|index| Self::ALL[index])
            .ok_or_else(|| {
                format!(
                    "unknown table encoding '{value}'; expected {}",
                    Self::LABELS.join(", ")
                )
            })
    }
}

/// Library compositions available for allDifferent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AllDifferentEncoding {
    /// Pairwise numeric disequalities, for every numeric representation.
    Pairwise,
    /// AMO over equal-value indicators; requires choice views for all operands.
    ValueAmo,
}
impl AllDifferentEncoding {
    pub const ALL: [Self; 2] = [Self::Pairwise, Self::ValueAmo];
    pub const LABELS: [&'static str; 2] = ["pairwise", "value-amo"];
}
impl Display for AllDifferentEncoding {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(Self::LABELS[Self::ALL.iter().position(|value| value == self).unwrap()])
    }
}
impl std::str::FromStr for AllDifferentEncoding {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, String> {
        Self::LABELS
            .iter()
            .position(|label| *label == value)
            .map(|index| Self::ALL[index])
            .ok_or_else(|| {
                format!(
                    "unknown allDifferent encoding '{value}'; expected {}",
                    Self::LABELS.join(", ")
                )
            })
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
    resolve_element_choices(decisions);
    resolve_table_choices(decisions);
    resolve_alldifferent_choices(decisions);
    resolve_pb_choices(decisions);
    resolve_cardinality_choices(decisions);
    resolve_amo_choices(decisions);
}
fn resolve_element_choices(decisions: &mut [SatEncodingDecision]) {
    use crate::settings::{self, Heuristic};
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
    let (algorithm, provenance) = if let Some(algorithm) = settings::element_encoding() {
        (algorithm, SelectionProvenance::ExplicitConfiguration)
    } else {
        let index = if algorithms.len() == 1 {
            0
        } else {
            match settings::heuristic() {
                Heuristic::First => 0,
                Heuristic::Compact => usize::from(sizes.iter().any(|size| *size > 4)),
                Heuristic::Random => settings::next_heuristic_random_index(algorithms.len()),
                Heuristic::Interactive => {
                    settings::next_heuristic_interactive_index(&ElementEncoding::LABELS)
                }
                Heuristic::All => settings::next_heuristic_all_index(&ElementEncoding::LABELS),
            }
        };
        (algorithms[index], SelectionProvenance::Heuristic)
    };
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
    let (algorithm, provenance) = if let Some(algorithm) = settings::table_encoding() {
        (algorithm, SelectionProvenance::ExplicitConfiguration)
    } else {
        let index = if algorithms.len() == 1 {
            0
        } else {
            match settings::heuristic() {
                Heuristic::First => 0,
                Heuristic::Compact => usize::from(rows.iter().any(|size| *size > 4)),
                Heuristic::Random => settings::next_heuristic_random_index(algorithms.len()),
                Heuristic::Interactive => settings::next_heuristic_interactive_index(labels),
                Heuristic::All => settings::next_heuristic_all_index(labels),
            }
        };
        (algorithms[index], SelectionProvenance::Heuristic)
    };
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
fn resolve_alldifferent_choices(decisions: &mut [SatEncodingDecision]) {
    use crate::settings::{self, Heuristic};
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
    let (algorithm, provenance) = if let Some(algorithm) = settings::alldifferent_encoding() {
        (algorithm, SelectionProvenance::ExplicitConfiguration)
    } else {
        let index = if algorithms.len() == 1 {
            0
        } else {
            match settings::heuristic() {
                Heuristic::First => 0,
                Heuristic::Compact => 1,
                Heuristic::Random => settings::next_heuristic_random_index(algorithms.len()),
                Heuristic::Interactive => {
                    settings::next_heuristic_interactive_index(&AllDifferentEncoding::LABELS)
                }
                Heuristic::All => settings::next_heuristic_all_index(&AllDifferentEncoding::LABELS),
            }
        };
        (algorithms[index], SelectionProvenance::Heuristic)
    };
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
                | SatEncodingDecision::CountRelation { encoding: None, .. }
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
    fn table_choices_are_shared_and_explicit_requests_have_provenance() {
        let build = |size| SatEncodingDecision::Table {
            output: true.into(),
            inputs: vec![],
            rows: vec![vec![]; size],
            row_views: None,
            negative: false,
            encoding: None,
            pb_encoding: None,
        };
        settings::set_table_encoding(None);
        settings::set_heuristic(Heuristic::Compact);
        let mut decisions = vec![build(1), build(5)];
        resolve_encoding_choices(&mut decisions);
        assert!(decisions.iter().all(|d| matches!(
            d,
            SatEncodingDecision::Table {
                encoding: Some(EncodingSelection {
                    algorithm: TableEncoding::Mdd,
                    provenance: SelectionProvenance::Heuristic
                }),
                ..
            }
        )));
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
