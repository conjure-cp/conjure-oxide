//! Generate solver clauses directly from semantic Boolean encoding decisions.
use crate::{
    ast::{AbstractLiteral, Atom, Expression, Literal, Name, SatEncodingDecision},
    solver::SolverError,
};
use rustsat::{
    encodings::atomics,
    instances::{Cnf, SatInstance},
    types::{Clause, Lit},
};
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Term {
    Constant(bool),
    Literal(Lit),
}
type TableNodeCache = HashMap<(usize, Vec<Vec<i64>>), Term>;
// Wide assertions and GTE predicates retain their original library constructions.
const MAX_SHARED_WEIGHTED_INPUTS: usize = 64;

impl Term {
    fn negated(self) -> Self {
        match self {
            Self::Constant(v) => Self::Constant(!v),
            Self::Literal(l) => Self::Literal(!l),
        }
    }
}

/// Compile semantic decisions using the instance's shared variable allocator.
pub fn compile_decisions(
    decisions: &[SatEncodingDecision],
    instance: &mut SatInstance,
    variables: &mut HashMap<Name, Lit>,
) -> Result<(), SolverError> {
    compile_decisions_with_cache(
        decisions,
        instance,
        variables,
        &mut EncodingCache::default(),
    )
}

/// Compile another decision batch against encodings already emitted into the same solver.
pub(super) fn compile_decisions_with_cache(
    decisions: &[SatEncodingDecision],
    instance: &mut SatInstance,
    variables: &mut HashMap<Name, Lit>,
    cache: &mut EncodingCache,
) -> Result<(), SolverError> {
    let mut compiler = Compiler {
        instance,
        variables,
        counters: Some(cache),
    };
    // Direct assertions determine whether a relation needs an assertion or equivalence.
    // Keep their units in the model so output literals remain correctly constrained.
    let mut asserted = std::collections::HashSet::new();
    for decision in decisions {
        let SatEncodingDecision::Assert(expression) = decision else {
            continue;
        };
        let input = match expression {
            Expression::Not(_, input) => input.as_ref(),
            input => input,
        };
        if matches!(input, Expression::Atomic(_, Atom::Reference(_)))
            && let Term::Literal(literal) = compiler.encode(expression)?
        {
            asserted.insert(literal);
        }
    }
    for decision in decisions {
        match decision {
            SatEncodingDecision::Objective {
                value, encoding, ..
            } => {
                // The adaptor owns solve-time objective state; it has no feasibility clauses.
                if encoding.is_none() {
                    return Err(SolverError::ModelInvalid(
                        "Unresolved objective PB encoding decision".into(),
                    ));
                }
                validate_pb_groups(&value.groups, value.terms.len())?;
            }
            SatEncodingDecision::IntegerRelation {
                output,
                terms,
                groups,
                relation,
                bound,
                encoding,
            } => {
                let algorithm = encoding
                    .as_ref()
                    .ok_or_else(|| {
                        SolverError::ModelInvalid(
                            "Unresolved integer relation encoding decision".into(),
                        )
                    })?
                    .algorithm;
                let output = compiler.encode(output)?;
                let output = match output {
                    Term::Literal(literal) if asserted.contains(&literal) => Term::Constant(true),
                    Term::Literal(literal) if asserted.contains(&!literal) => Term::Constant(false),
                    output => output,
                };
                let terms = terms
                    .iter()
                    .map(|(weight, input)| compiler.encode(input).map(|term| (*weight, term)))
                    .collect::<Result<Vec<_>, _>>()?;
                validate_pb_groups(groups, terms.len())?;
                if !compiler.choice_relation(output, *relation, *bound, &terms, groups) {
                    compiler
                        .integer_relation(algorithm, output, *relation, *bound, &terms, groups)?;
                }
            }
            SatEncodingDecision::Element {
                index_view,
                value,
                entries,
                encoding,
                pb_encoding,
            } => {
                compiler.element(index_view, value, entries, encoding, pb_encoding)?;
            }
            SatEncodingDecision::Table {
                output,
                inputs,
                rows,
                negative,
                encoding,
                pb_encoding,
            } => {
                let output = compiler.encode(output)?;
                compiler.table(output, inputs, rows, *negative, encoding, pb_encoding)?;
            }
            SatEncodingDecision::AllDifferent {
                output,
                inputs,
                except,
                encoding,
                amo_encoding,
                pb_encoding,
            } => {
                let output = compiler.encode(output)?;
                let output = match output {
                    Term::Literal(literal) if asserted.contains(&literal) => Term::Constant(true),
                    Term::Literal(literal) if asserted.contains(&!literal) => Term::Constant(false),
                    output => output,
                };
                compiler.alldifferent(
                    output,
                    inputs,
                    *except,
                    encoding,
                    amo_encoding,
                    pb_encoding,
                )?;
            }
            SatEncodingDecision::PseudoBoolean {
                terms,
                groups,
                relation,
                bound,
                encoding,
            } => {
                let algorithm = encoding
                    .as_ref()
                    .ok_or_else(|| {
                        SolverError::ModelInvalid(
                            "Unresolved pseudo-Boolean encoding decision".into(),
                        )
                    })?
                    .algorithm;
                let terms = terms
                    .iter()
                    .map(|(weight, input)| compiler.encode(input).map(|term| (*weight, term)))
                    .collect::<Result<Vec<_>, _>>()?;
                validate_pb_groups(groups, terms.len())?;
                compiler.pseudo_boolean(algorithm, *relation, *bound, terms, groups)?;
            }
            SatEncodingDecision::CountRelation {
                output,
                inputs,
                relation,
                bound,
                encoding,
                amo_encoding,
            } => {
                let algorithm = encoding
                    .as_ref()
                    .ok_or_else(|| {
                        SolverError::ModelInvalid(
                            "Unresolved count cardinality encoding decision".into(),
                        )
                    })?
                    .algorithm;
                let output = compiler.encode(output)?;
                let output = match output {
                    Term::Literal(literal) if asserted.contains(&literal) => Term::Constant(true),
                    Term::Literal(literal) if asserted.contains(&!literal) => Term::Constant(false),
                    output => output,
                };
                let terms = inputs
                    .iter()
                    .map(|input| compiler.encode(input))
                    .collect::<Result<Vec<_>, _>>()?;
                compiler.count_relation(
                    algorithm,
                    amo_encoding.as_ref().map(|selection| selection.algorithm),
                    output,
                    *relation,
                    *bound,
                    terms,
                )?;
            }
            SatEncodingDecision::Cardinality {
                inputs,
                relation,
                bound,
                encoding,
            } => {
                let algorithm = encoding
                    .as_ref()
                    .ok_or_else(|| {
                        SolverError::ModelInvalid("Unresolved cardinality encoding decision".into())
                    })?
                    .algorithm;
                let terms = inputs
                    .iter()
                    .map(|input| compiler.encode(input))
                    .collect::<Result<Vec<_>, _>>()?;
                compiler.cardinality(algorithm, *relation, *bound, terms)?;
            }
            SatEncodingDecision::AtMostOne { inputs, encoding } => {
                let algorithm = encoding
                    .as_ref()
                    .ok_or_else(|| {
                        SolverError::ModelInvalid("Unresolved AMO encoding decision".into())
                    })?
                    .algorithm;
                let terms = inputs
                    .iter()
                    .map(|input| compiler.encode(input))
                    .collect::<Result<Vec<_>, _>>()?;
                compiler.asserted_amo(algorithm, terms)?;
            }
            SatEncodingDecision::Assert(expression) => {
                let term = compiler.encode(expression)?;
                compiler.assert(term);
            }
            SatEncodingDecision::Boolean { output, expression } => {
                let output = compiler.encode(output)?;
                let value = compiler.encode(expression)?;
                compiler.equate(output, value);
                compiler.retain_boolean_alias(output, value);
            }
        }
    }
    Ok(())
}
// Canonical positive variables retain signed coefficients for structured input handling.
fn canonical_pb_terms(
    bound: i64,
    terms: &[(i64, Term)],
) -> (i128, std::collections::BTreeMap<Lit, i128>) {
    let mut bound = i128::from(bound);
    let mut coefficients = std::collections::BTreeMap::<Lit, i128>::new();
    for &(weight, term) in terms {
        let weight = i128::from(weight);
        match term {
            Term::Constant(true) => bound -= weight,
            Term::Constant(false) => (),
            Term::Literal(literal) => {
                let (literal, weight) = if literal.is_neg() {
                    bound -= weight;
                    (!literal, -weight)
                } else {
                    (literal, weight)
                };
                *coefficients.entry(literal).or_default() += weight;
            }
        }
    }
    (bound, coefficients)
}
fn validate_pb_groups(
    groups: &[crate::ast::sat_decision::PbTermGroup],
    len: usize,
) -> Result<(), SolverError> {
    use crate::ast::sat_decision::PbTermStructure;
    let mut end = 0;
    for group in groups {
        if group.start < end
            || group.start >= group.end
            || group.end > len
            || matches!(group.structure, PbTermStructure::BoundedBinary { lower, upper } if lower > upper)
        {
            return Err(SolverError::ModelInvalid(
                "Invalid pseudo-Boolean term group".into(),
            ));
        }
        end = group.end;
    }
    Ok(())
}
/// Compile and normalise the fixed input set for a minimised objective cost.
pub(super) fn compile_objective_terms(
    value: &crate::ast::sat_decision::SatIntegerView,
    minimise: bool,
    instance: &mut SatInstance,
    variables: &mut HashMap<Name, Lit>,
) -> Result<(i128, Vec<(Lit, usize)>), SolverError> {
    validate_pb_groups(&value.groups, value.terms.len())?;
    let mut compiler = Compiler {
        instance,
        variables,
        counters: None,
    };
    let terms = value
        .terms
        .iter()
        .map(|(weight, expression)| compiler.encode(expression).map(|term| (*weight, term)))
        .collect::<Result<Vec<_>, _>>()?;
    let (adjusted, coefficients) = canonical_pb_terms(0, &terms);
    let sign = if minimise { 1i128 } else { -1i128 };
    let mut constant = sign * (i128::from(value.constant) - adjusted);
    let mut positive = Vec::new();
    let mut total = 0i128;
    for (literal, weight) in coefficients {
        let weight = sign * weight;
        if weight == 0 {
            continue;
        }
        let (literal, weight) = if weight < 0 {
            constant += weight;
            (!literal, -weight)
        } else {
            (literal, weight)
        };
        total += weight;
        if total >= (isize::MAX as i128).min(i128::from(i64::MAX)) {
            return Err(SolverError::ModelInvalid(
                "Objective coefficient sum exceeds the library range".into(),
            ));
        }
        positive.push((literal, weight as usize));
    }
    Ok((constant, positive))
}

/// Shared gates, aliases, native counters and threshold predicates for one loaded solver.
#[derive(Default)]
pub(super) struct EncodingCache {
    totalizers: HashMap<Vec<Lit>, rustsat::encodings::card::Totalizer>,
    thresholds: HashMap<(Vec<Lit>, i64), Lit>,
    occurrences: HashMap<Vec<Lit>, Vec<Lit>>,
    weighted: HashMap<WeightedKey, WeightedEncoder>,
    weighted_thresholds: HashMap<WeightedThresholdKey, Lit>,
    gates: HashMap<(bool, Vec<Lit>), Lit>,
    aliases: HashMap<Lit, Term>,
}

type WeightedKey = (crate::ast::sat_decision::PbEncoding, Vec<(Lit, usize)>);

#[derive(PartialEq, Eq, Hash)]
struct WeightedThresholdKey {
    algorithm: crate::ast::sat_decision::PbEncoding,
    coefficients: Vec<(Lit, i128)>,
    groups: Vec<WeightedGroupKey>,
    bound: i128,
}

type WeightedGroupKey = (u8, i128, i128, Vec<(i128, Term)>);

// These providers expose bound outputs which can be enforced independently.
enum WeightedEncoder {
    Totalizer(rustsat::encodings::pb::GeneralizedTotalizer),
    Adder(rustsat::encodings::pb::BinaryAdder),
}

struct Compiler<'a> {
    instance: &'a mut SatInstance,
    variables: &'a mut HashMap<Name, Lit>,
    counters: Option<&'a mut EncodingCache>,
}
impl Compiler<'_> {
    fn asserted_amo(
        &mut self,
        algorithm: crate::ast::sat_decision::AmoEncoding,
        terms: Vec<Term>,
    ) -> Result<(), SolverError> {
        let true_count = terms
            .iter()
            .filter(|term| matches!(term, Term::Constant(true)))
            .count();
        let literals: Vec<_> = terms
            .into_iter()
            .filter_map(|term| match term {
                Term::Literal(literal) => Some(literal),
                _ => None,
            })
            .collect();
        if true_count >= 2 {
            self.assert(Term::Constant(false));
        } else if true_count == 1 {
            for literal in literals {
                self.assert(Term::Literal(!literal));
            }
        } else {
            self.amo(algorithm, literals)?;
        }
        Ok(())
    }

    fn view_equality(
        &mut self,
        left: &crate::ast::sat_decision::SatIntegerView,
        right: &crate::ast::sat_decision::SatIntegerView,
        algorithm: crate::ast::sat_decision::PbEncoding,
    ) -> Result<Term, SolverError> {
        use crate::ast::sat_decision::{IntegerRelation, PbTermStructure};
        validate_pb_groups(&left.groups, left.terms.len())?;
        validate_pb_groups(&right.groups, right.terms.len())?;
        if left.terms.is_empty() && right.terms.is_empty() {
            return Ok(Term::Constant(left.constant == right.constant));
        }
        let range_error = || {
            SolverError::ModelInvalid("Element numeric difference exceeds the library range".into())
        };
        let bound = i64::try_from(i128::from(right.constant) - i128::from(left.constant))
            .map_err(|_| range_error())?;
        let mut terms = left
            .terms
            .iter()
            .map(|(weight, expression)| self.encode(expression).map(|term| (*weight, term)))
            .collect::<Result<Vec<_>, _>>()?;
        for (weight, expression) in &right.terms {
            terms.push((
                weight.checked_neg().ok_or_else(range_error)?,
                self.encode(expression)?,
            ));
        }
        let mut groups = left.groups.clone();
        for group in &right.groups {
            let mut group = group.clone();
            group.start += left.terms.len();
            group.end += left.terms.len();
            if let PbTermStructure::BoundedBinary { lower, upper } = group.structure {
                group.structure = PbTermStructure::BoundedBinary {
                    lower: upper.checked_neg().ok_or_else(range_error)?,
                    upper: lower.checked_neg().ok_or_else(range_error)?,
                };
            }
            groups.push(group);
        }
        let truth = Term::Literal(self.instance.new_lit());
        if !self.choice_relation(truth, IntegerRelation::Equal, bound, &terms, &groups) {
            self.integer_relation(
                algorithm,
                truth,
                IntegerRelation::Equal,
                bound,
                &terms,
                &groups,
            )?;
        }
        Ok(truth)
    }

    fn element(
        &mut self,
        index: &crate::ast::sat_decision::SatIntegerView,
        value: &crate::ast::sat_decision::SatIntegerView,
        entries: &[(i64, crate::ast::sat_decision::SatIntegerView)],
        encoding: &Option<
            crate::ast::sat_decision::EncodingSelection<crate::ast::sat_decision::ElementEncoding>,
        >,
        pb_encoding: &Option<
            crate::ast::sat_decision::EncodingSelection<crate::ast::sat_decision::PbEncoding>,
        >,
    ) -> Result<(), SolverError> {
        use crate::ast::sat_decision::{ElementEncoding, SatIntegerView};
        let algorithm = encoding
            .as_ref()
            .ok_or_else(|| {
                SolverError::ModelInvalid("Unresolved element encoding decision".into())
            })?
            .algorithm;
        let pb = pb_encoding
            .as_ref()
            .ok_or_else(|| {
                SolverError::ModelInvalid("Unresolved element component PB encoding".into())
            })?
            .algorithm;
        let mut labels = std::collections::HashSet::new();
        if entries.iter().any(|(label, _)| !labels.insert(*label)) {
            return Err(SolverError::ModelInvalid(
                "Element index labels must be unique".into(),
            ));
        }
        let mut selectors = Vec::new();
        let mut supports = Vec::new();
        for (label, entry) in entries {
            let label = SatIntegerView {
                constant: *label,
                terms: vec![],
                groups: vec![],
                choices: None,
            };
            let selector = self.view_equality(index, &label, pb)?;
            let equality = self.view_equality(value, entry, pb)?;
            match algorithm {
                ElementEncoding::Implication => {
                    let implication = self.combine(false, vec![selector.negated(), equality]);
                    self.equate(Term::Constant(true), implication);
                }
                ElementEncoding::Support => {
                    selectors.push(selector);
                    supports.push(self.combine(true, vec![selector, equality]));
                }
            }
        }
        if algorithm == ElementEncoding::Support {
            let valid = self.combine(false, selectors);
            supports.push(valid.negated());
            let definition = self.combine(false, supports);
            self.equate(Term::Constant(true), definition);
        }
        Ok(())
    }

    fn table(
        &mut self,
        output: Term,
        inputs: &[crate::ast::sat_decision::SatIntegerView],
        rows: &[Vec<i64>],
        negative: bool,
        encoding: &Option<
            crate::ast::sat_decision::EncodingSelection<crate::ast::sat_decision::TableEncoding>,
        >,
        pb_encoding: &Option<
            crate::ast::sat_decision::EncodingSelection<crate::ast::sat_decision::PbEncoding>,
        >,
    ) -> Result<(), SolverError> {
        use crate::ast::sat_decision::{IntegerRelation, TableEncoding};
        let algorithm = encoding
            .as_ref()
            .ok_or_else(|| SolverError::ModelInvalid("Unresolved table encoding decision".into()))?
            .algorithm;
        if rows.iter().any(|row| row.len() != inputs.len()) {
            return Err(SolverError::ModelInvalid(
                "Table row width differs from tuple width".into(),
            ));
        }
        if rows.is_empty() || inputs.is_empty() {
            let truth = !rows.is_empty();
            self.equate(output, Term::Constant(truth != negative));
            return Ok(());
        }
        let pb = pb_encoding
            .as_ref()
            .ok_or_else(|| {
                SolverError::ModelInvalid("Unresolved table component PB encoding".into())
            })?
            .algorithm;
        let mut rows = rows.to_vec();
        rows.sort();
        rows.dedup();
        let mut cells = Vec::with_capacity(inputs.len());
        for (column, input) in inputs.iter().enumerate() {
            validate_pb_groups(&input.groups, input.terms.len())?;
            let terms = input
                .terms
                .iter()
                .map(|(weight, expression)| self.encode(expression).map(|term| (*weight, term)))
                .collect::<Result<Vec<_>, _>>()?;
            let values = rows
                .iter()
                .map(|row| row[column])
                .collect::<std::collections::BTreeSet<_>>();
            let mut equalities = std::collections::BTreeMap::new();
            for value in values {
                if terms.is_empty() {
                    equalities.insert(value, Term::Constant(input.constant == value));
                    continue;
                }
                let bound = i64::try_from(i128::from(value) - i128::from(input.constant)).map_err(
                    |_| {
                        SolverError::ModelInvalid(
                            "Table cell difference exceeds the library range".into(),
                        )
                    },
                )?;
                let truth = Term::Literal(self.instance.new_lit());
                if !self.choice_relation(
                    truth,
                    IntegerRelation::Equal,
                    bound,
                    &terms,
                    &input.groups,
                ) {
                    self.integer_relation(
                        pb,
                        truth,
                        IntegerRelation::Equal,
                        bound,
                        &terms,
                        &input.groups,
                    )?;
                }
                equalities.insert(value, truth);
            }
            cells.push(equalities);
        }
        let truth = match algorithm {
            TableEncoding::Tuple => {
                let matches = rows
                    .iter()
                    .map(|row| {
                        self.combine(
                            true,
                            row.iter()
                                .enumerate()
                                .map(|(column, value)| cells[column][value])
                                .collect(),
                        )
                    })
                    .collect();
                self.combine(false, matches)
            }
            TableEncoding::Mdd => self.table_mdd(0, rows, &cells, &mut HashMap::new()),
        };
        self.equate(output, if negative { truth.negated() } else { truth });
        Ok(())
    }

    /// Identical suffix relations share one layered node and its library gates.
    fn table_mdd(
        &mut self,
        column: usize,
        rows: Vec<Vec<i64>>,
        cells: &[std::collections::BTreeMap<i64, Term>],
        memo: &mut TableNodeCache,
    ) -> Term {
        if column == cells.len() {
            return Term::Constant(!rows.is_empty());
        }
        let key = (column, rows.clone());
        if let Some(truth) = memo.get(&key) {
            return *truth;
        }
        let mut branches = std::collections::BTreeMap::<i64, Vec<Vec<i64>>>::new();
        for row in rows {
            branches.entry(row[0]).or_default().push(row[1..].to_vec());
        }
        let mut truths = Vec::new();
        for (value, mut suffixes) in branches {
            suffixes.sort();
            suffixes.dedup();
            let suffix = self.table_mdd(column + 1, suffixes, cells, memo);
            truths.push(self.combine(true, vec![cells[column][&value], suffix]));
        }
        let truth = self.combine(false, truths);
        memo.insert(key, truth);
        truth
    }

    fn alldifferent(
        &mut self,
        output: Term,
        inputs: &[crate::ast::sat_decision::SatIntegerView],
        except: Option<i64>,
        encoding: &Option<
            crate::ast::sat_decision::EncodingSelection<
                crate::ast::sat_decision::AllDifferentEncoding,
            >,
        >,
        amo_encoding: &Option<
            crate::ast::sat_decision::EncodingSelection<crate::ast::sat_decision::AmoEncoding>,
        >,
        pb_encoding: &Option<
            crate::ast::sat_decision::EncodingSelection<crate::ast::sat_decision::PbEncoding>,
        >,
    ) -> Result<(), SolverError> {
        use crate::ast::sat_decision::{AllDifferentEncoding, IntegerRelation, PbTermStructure};
        let algorithm = encoding
            .as_ref()
            .ok_or_else(|| {
                SolverError::ModelInvalid("Unresolved allDifferent encoding decision".into())
            })?
            .algorithm;
        if inputs.len() < 2 {
            self.equate(output, Term::Constant(true));
            return Ok(());
        }
        let mut truths = Vec::new();
        if algorithm == AllDifferentEncoding::ValueAmo {
            let amo = amo_encoding
                .as_ref()
                .ok_or_else(|| {
                    SolverError::ModelInvalid(
                        "Unresolved allDifferent AMO encoding decision".into(),
                    )
                })?
                .algorithm;
            let mut by_value = std::collections::BTreeMap::<i64, Vec<Term>>::new();
            for input in inputs {
                let choices = input.choices.as_ref().ok_or_else(|| SolverError::ModelInvalid(
                    "allDifferent value-amo requires value indicators for every operand (Direct or Boolean views)".into()))?;
                for (value, expression) in choices {
                    if except == Some(*value) {
                        continue;
                    }
                    by_value
                        .entry(*value)
                        .or_default()
                        .push(self.encode(expression)?);
                }
            }
            for terms in by_value.into_values().filter(|terms| terms.len() >= 2) {
                if matches!(output, Term::Constant(true)) {
                    self.asserted_amo(amo, terms)?;
                } else {
                    let pb = pb_encoding
                        .as_ref()
                        .ok_or_else(|| {
                            SolverError::ModelInvalid(
                                "Unresolved allDifferent PB encoding decision".into(),
                            )
                        })?
                        .algorithm;
                    let truth = self.instance.new_lit();
                    let terms: Vec<_> = terms.into_iter().map(|term| (1, term)).collect();
                    self.integer_relation(
                        pb,
                        Term::Literal(truth),
                        IntegerRelation::LessEqual,
                        1,
                        &terms,
                        &[],
                    )?;
                    truths.push(Term::Literal(truth));
                }
            }
        } else if let Some(except) = except {
            let pb = pb_encoding
                .as_ref()
                .ok_or_else(|| {
                    SolverError::ModelInvalid("Unresolved allDifferent PB encoding decision".into())
                })?
                .algorithm;
            let exception = crate::ast::sat_decision::SatIntegerView {
                constant: except,
                terms: vec![],
                groups: vec![],
                choices: None,
            };
            for (index, left) in inputs.iter().enumerate() {
                let exempt = self.view_equality(left, &exception, pb)?;
                for right in &inputs[index + 1..] {
                    let distinct = self.view_equality(left, right, pb)?.negated();
                    let truth = self.combine(false, vec![exempt, distinct]);
                    if matches!(output, Term::Constant(true)) {
                        self.equate(output, truth);
                    } else {
                        truths.push(truth);
                    }
                }
            }
        } else {
            let pb = pb_encoding
                .as_ref()
                .ok_or_else(|| {
                    SolverError::ModelInvalid("Unresolved allDifferent PB encoding decision".into())
                })?
                .algorithm;
            let views = inputs
                .iter()
                .map(|input| {
                    validate_pb_groups(&input.groups, input.terms.len())?;
                    let terms = input
                        .terms
                        .iter()
                        .map(|(weight, expression)| {
                            self.encode(expression).map(|term| (*weight, term))
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok((input, terms))
                })
                .collect::<Result<Vec<_>, SolverError>>()?;
            for (index, (left, lhs)) in views.iter().enumerate() {
                for (right, rhs) in &views[index + 1..] {
                    let range_error = || {
                        SolverError::ModelInvalid(
                            "allDifferent numeric difference exceeds the library range".into(),
                        )
                    };
                    let bound =
                        i64::try_from(i128::from(right.constant) - i128::from(left.constant))
                            .map_err(|_| range_error())?;
                    let mut terms = lhs.clone();
                    for &(weight, term) in rhs {
                        terms.push((weight.checked_neg().ok_or_else(range_error)?, term));
                    }
                    let mut groups = left.groups.clone();
                    for group in &right.groups {
                        let mut group = group.clone();
                        group.start += lhs.len();
                        group.end += lhs.len();
                        if let PbTermStructure::BoundedBinary { lower, upper } = group.structure {
                            group.structure = PbTermStructure::BoundedBinary {
                                lower: upper.checked_neg().ok_or_else(range_error)?,
                                upper: lower.checked_neg().ok_or_else(range_error)?,
                            };
                        }
                        groups.push(group);
                    }
                    let truth = if matches!(output, Term::Constant(true)) {
                        output
                    } else {
                        Term::Literal(self.instance.new_lit())
                    };
                    if !self.choice_relation(
                        truth,
                        IntegerRelation::NotEqual,
                        bound,
                        &terms,
                        &groups,
                    ) {
                        self.integer_relation(
                            pb,
                            truth,
                            IntegerRelation::NotEqual,
                            bound,
                            &terms,
                            &groups,
                        )?;
                    }
                    truths.push(truth);
                }
            }
        }
        if !matches!(output, Term::Constant(true)) {
            let value = self.combine(true, truths);
            self.equate(output, value);
        }
        Ok(())
    }

    /// Preserve one-hot numeric choices instead of constructing a weighted counter.
    fn choice_relation(
        &mut self,
        output: Term,
        relation: crate::ast::sat_decision::IntegerRelation,
        bound: i64,
        terms: &[(i64, Term)],
        groups: &[crate::ast::sat_decision::PbTermGroup],
    ) -> bool {
        use crate::ast::sat_decision::{IntegerRelation, PbTermStructure};
        if groups.is_empty()
            || groups.len() > 2
            || groups[0].start != 0
            || groups.last().is_none_or(|group| group.end != terms.len())
            || groups
                .iter()
                .any(|group| group.structure != PbTermStructure::Choice)
            || (groups.len() == 2 && groups[0].end != groups[1].start)
        {
            return false;
        }
        let mut alternatives = Vec::new();
        if groups.len() == 1 {
            for &(value, term) in terms {
                let matches = match relation {
                    IntegerRelation::Equal => value == bound,
                    IntegerRelation::NotEqual => value != bound,
                    IntegerRelation::Less => value < bound,
                    IntegerRelation::LessEqual => value <= bound,
                    IntegerRelation::Greater => value > bound,
                    IntegerRelation::GreaterEqual => value >= bound,
                };
                if matches {
                    alternatives.push(term);
                }
            }
        } else {
            if !matches!(relation, IntegerRelation::Equal | IntegerRelation::NotEqual) {
                return false;
            }
            let mut right = HashMap::<i128, Vec<Term>>::new();
            for &(weight, term) in &terms[groups[1].start..] {
                right.entry(i128::from(weight)).or_default().push(term);
            }
            for &(weight, term) in &terms[..groups[0].end] {
                if let Some(matches) = right.get(&(i128::from(bound) - i128::from(weight))) {
                    let matches = self.combine(false, matches.clone());
                    alternatives.push(self.combine(true, vec![term, matches]));
                }
            }
        }
        let value = self.combine(false, alternatives);
        self.equate(
            output,
            if groups.len() == 2 && relation == IntegerRelation::NotEqual {
                value.negated()
            } else {
                value
            },
        );
        true
    }

    fn equate(&mut self, output: Term, value: Term) {
        match (output, value) {
            (Term::Literal(output), Term::Literal(value)) => {
                self.instance
                    .add_clause(atomics::lit_impl_lit(output, value));
                self.instance
                    .add_clause(atomics::lit_impl_lit(value, output));
            }
            (Term::Constant(value), term) | (term, Term::Constant(value)) => {
                self.assert(if value { term } else { term.negated() })
            }
        }
    }

    fn integer_relation(
        &mut self,
        algorithm: crate::ast::sat_decision::PbEncoding,
        output: Term,
        relation: crate::ast::sat_decision::IntegerRelation,
        bound: i64,
        terms: &[(i64, Term)],
        groups: &[crate::ast::sat_decision::PbTermGroup],
    ) -> Result<(), SolverError> {
        use crate::ast::sat_decision::{CardinalityRelation, IntegerRelation, PbEncoding};
        if !groups.is_empty()
            && matches!(
                algorithm,
                PbEncoding::PindakaasBdd | PbEncoding::PindakaasSwc
            )
        {
            if matches!(
                (relation, output),
                (IntegerRelation::Equal, Term::Constant(true))
                    | (IntegerRelation::NotEqual, Term::Constant(false))
            ) {
                return self.pseudo_boolean(
                    algorithm,
                    CardinalityRelation::Exactly,
                    bound,
                    terms.to_vec(),
                    groups,
                );
            }
            let bound = i128::from(bound);
            return match relation {
                IntegerRelation::Less => {
                    self.structured_reified_upper(algorithm, output, bound - 1, terms, groups)
                }
                IntegerRelation::LessEqual => {
                    self.structured_reified_upper(algorithm, output, bound, terms, groups)
                }
                IntegerRelation::Greater => {
                    self.structured_reified_upper(algorithm, output.negated(), bound, terms, groups)
                }
                IntegerRelation::GreaterEqual => self.structured_reified_upper(
                    algorithm,
                    output.negated(),
                    bound - 1,
                    terms,
                    groups,
                ),
                IntegerRelation::Equal | IntegerRelation::NotEqual => {
                    let upper = self.instance.new_lit();
                    let below = self.instance.new_lit();
                    self.structured_reified_upper(
                        algorithm,
                        Term::Literal(upper),
                        bound,
                        terms,
                        groups,
                    )?;
                    self.structured_reified_upper(
                        algorithm,
                        Term::Literal(below),
                        bound - 1,
                        terms,
                        groups,
                    )?;
                    let value =
                        self.combine(true, vec![Term::Literal(upper), Term::Literal(!below)]);
                    self.equate(
                        output,
                        if relation == IntegerRelation::Equal {
                            value
                        } else {
                            value.negated()
                        },
                    );
                    Ok(())
                }
            };
        }
        let (mut bound, coefficients) = canonical_pb_terms(bound, terms);
        let mut total = 0i128;
        let mut positive = Vec::new();
        for (literal, weight) in coefficients {
            if weight == 0 {
                continue;
            }
            let (literal, weight) = if weight < 0 {
                bound -= weight;
                (!literal, -weight)
            } else {
                (literal, weight)
            };
            total += weight;
            positive.push((literal, weight));
        }
        if matches!(
            (relation, output),
            (IntegerRelation::Equal, Term::Constant(true))
                | (IntegerRelation::NotEqual, Term::Constant(false))
        ) {
            if bound < 0 || bound > total {
                self.assert(Term::Constant(false));
                return Ok(());
            }
            if total >= (isize::MAX as i128).min(i128::from(i64::MAX)) {
                return Err(SolverError::ModelInvalid(
                    "Integer coefficient sum exceeds the library range".into(),
                ));
            }
            return self.pseudo_boolean(
                algorithm,
                crate::ast::sat_decision::CardinalityRelation::Exactly,
                bound as i64,
                positive
                    .iter()
                    .map(|(lit, weight)| (*weight as i64, Term::Literal(*lit)))
                    .collect(),
                &[],
            );
        }
        match relation {
            IntegerRelation::Less => {
                self.reified_upper(algorithm, output, bound - 1, &positive, total)
            }
            IntegerRelation::LessEqual => {
                self.reified_upper(algorithm, output, bound, &positive, total)
            }
            IntegerRelation::Greater => {
                self.reified_upper(algorithm, output.negated(), bound, &positive, total)
            }
            IntegerRelation::GreaterEqual => {
                self.reified_upper(algorithm, output.negated(), bound - 1, &positive, total)
            }
            IntegerRelation::Equal | IntegerRelation::NotEqual => {
                let upper = self.instance.new_lit();
                let lower = self.instance.new_lit();
                self.reified_upper(algorithm, Term::Literal(upper), bound, &positive, total)?;
                let inverted: Vec<_> = positive
                    .iter()
                    .map(|(lit, weight)| (!*lit, *weight))
                    .collect();
                self.reified_upper(
                    algorithm,
                    Term::Literal(lower),
                    total - bound,
                    &inverted,
                    total,
                )?;
                let conjunction = self.instance.new_lit();
                for clause in atomics::lit_impl_cube(conjunction, &[upper, lower]) {
                    self.instance.add_clause(clause);
                }
                self.instance
                    .add_clause(atomics::cube_impl_lit(&[upper, lower], conjunction));
                self.equate(
                    output,
                    Term::Literal(if relation == IntegerRelation::Equal {
                        conjunction
                    } else {
                        !conjunction
                    }),
                );
                Ok(())
            }
        }
    }

    // Keep the original polarities and group bounds in both implication directions.
    fn structured_reified_upper(
        &mut self,
        algorithm: crate::ast::sat_decision::PbEncoding,
        output: Term,
        bound: i128,
        terms: &[(i64, Term)],
        groups: &[crate::ast::sat_decision::PbTermGroup],
    ) -> Result<(), SolverError> {
        use crate::ast::sat_decision::CardinalityRelation;
        let (adjusted, coefficients) = canonical_pb_terms(0, terms);
        let minimum = -adjusted
            + coefficients
                .values()
                .filter(|weight| **weight < 0)
                .sum::<i128>();
        let maximum = -adjusted
            + coefficients
                .values()
                .filter(|weight| **weight > 0)
                .sum::<i128>();
        if bound < minimum || bound >= maximum {
            self.equate(output, Term::Constant(bound >= maximum));
            return Ok(());
        }
        let bound = i64::try_from(bound).map_err(|_| {
            SolverError::ModelInvalid("Structured integer bound exceeds the library range".into())
        })?;
        let complement_bound = || {
            i64::try_from(i128::from(bound) + 1).map_err(|_| {
                SolverError::ModelInvalid(
                    "Structured integer bound exceeds the library range".into(),
                )
            })
        };
        if self.counters.is_some() && matches!(output, Term::Literal(_)) {
            return self.cached_weighted_threshold(algorithm, output, bound, terms, groups);
        }
        match output {
            Term::Constant(value) => self.pseudo_boolean(
                algorithm,
                if value {
                    CardinalityRelation::AtMost
                } else {
                    CardinalityRelation::AtLeast
                },
                if value { bound } else { complement_bound()? },
                terms.to_vec(),
                groups,
            ),
            Term::Literal(literal) => {
                self.guarded_pseudo_boolean(
                    algorithm,
                    CardinalityRelation::AtMost,
                    bound,
                    terms.to_vec(),
                    groups,
                    Some(literal),
                )?;
                self.guarded_pseudo_boolean(
                    algorithm,
                    CardinalityRelation::AtLeast,
                    complement_bound()?,
                    terms.to_vec(),
                    groups,
                    Some(!literal),
                )
            }
        }
    }

    fn reified_upper(
        &mut self,
        algorithm: crate::ast::sat_decision::PbEncoding,
        output: Term,
        bound: i128,
        positive: &[(Lit, i128)],
        total: i128,
    ) -> Result<(), SolverError> {
        use crate::ast::sat_decision::CardinalityRelation;
        use rustsat::types::constraints::PbConstraint;
        if bound < 0 || bound >= total {
            self.equate(output, Term::Constant(bound >= total));
            return Ok(());
        }
        if let Term::Constant(value) = output {
            let (terms, bound) = if value {
                (
                    positive
                        .iter()
                        .map(|(lit, weight)| (*weight as i64, Term::Literal(*lit)))
                        .collect(),
                    bound,
                )
            } else {
                (
                    positive
                        .iter()
                        .map(|(lit, weight)| (*weight as i64, Term::Literal(!*lit)))
                        .collect(),
                    total - bound - 1,
                )
            };
            if total >= (isize::MAX as i128).min(i128::from(i64::MAX)) {
                return Err(SolverError::ModelInvalid(
                    "Integer coefficient sum exceeds the library range".into(),
                ));
            }
            return self.pseudo_boolean(
                algorithm,
                CardinalityRelation::AtMost,
                bound as i64,
                terms,
                &[],
            );
        }
        // RustSAT's reverse implication computes twice the coefficient sum.
        if total >= (isize::MAX as i128).min(i128::from(i64::MAX)) / 2 {
            return Err(SolverError::ModelInvalid(
                "Reified integer coefficient sum exceeds the library range".into(),
            ));
        }
        if self.counters.is_some() {
            let terms: Vec<_> = positive
                .iter()
                .map(|(lit, weight)| (*weight as i64, Term::Literal(*lit)))
                .collect();
            return self.cached_weighted_threshold(algorithm, output, bound as i64, &terms, &[]);
        }
        let Term::Literal(literal) = output else {
            unreachable!()
        };
        let constraint = PbConstraint::new_ub_unsigned(
            positive
                .iter()
                .map(|(lit, weight)| (*lit, *weight as usize)),
            bound as isize,
        );
        // Library implication transformations supply both directions of equivalence.
        for constraint in [
            atomics::lit_impl_pb(literal, &constraint),
            atomics::pb_impl_lit(&constraint, literal),
        ] {
            let (terms, bound, relation) = match constraint {
                PbConstraint::Ub(c) => {
                    let (terms, bound) = c.decompose();
                    (terms, bound, CardinalityRelation::AtMost)
                }
                PbConstraint::Lb(c) => {
                    let (terms, bound) = c.decompose();
                    (terms, bound, CardinalityRelation::AtLeast)
                }
                PbConstraint::Eq(_) => {
                    unreachable!("Implication of an upper bound remains an inequality")
                }
            };
            self.pseudo_boolean(
                algorithm,
                relation,
                bound as i64,
                terms
                    .into_iter()
                    .map(|(lit, weight)| (weight as i64, Term::Literal(lit)))
                    .collect(),
                &[],
            )?;
        }
        Ok(())
    }

    fn cached_weighted_threshold(
        &mut self,
        algorithm: crate::ast::sat_decision::PbEncoding,
        output: Term,
        bound: i64,
        terms: &[(i64, Term)],
        groups: &[crate::ast::sat_decision::PbTermGroup],
    ) -> Result<(), SolverError> {
        use crate::ast::sat_decision::PbTermStructure;
        use crate::ast::sat_decision::{CardinalityRelation, PbEncoding};
        let (adjusted, coefficients) = canonical_pb_terms(bound, terms);
        let mut coefficients: Vec<_> = coefficients
            .into_iter()
            .filter(|(_, weight)| *weight != 0)
            .collect();
        // S <= b and -S <= -b-1 are complementary predicates.
        let inverted = coefficients.first().is_some_and(|(_, weight)| *weight < 0);
        let sign = if inverted { -1i128 } else { 1 };
        for (_, weight) in &mut coefficients {
            *weight *= sign;
        }
        let mut group_keys: Vec<_> = groups
            .iter()
            .map(|group| {
                let mut members: Vec<_> = terms[group.start..group.end]
                    .iter()
                    .map(|(weight, term)| (i128::from(*weight) * sign, *term))
                    .collect();
                let (kind, lower, upper) = match group.structure {
                    PbTermStructure::Choice => (0, 0, 0),
                    PbTermStructure::Chain => (1, 0, 0),
                    PbTermStructure::BoundedBinary { lower, upper } => {
                        let a = i128::from(lower) * sign;
                        let b = i128::from(upper) * sign;
                        (2, a.min(b), a.max(b))
                    }
                };
                // Chains carry implication order; choice/binary sums do not.
                if kind != 1 {
                    members.sort_unstable();
                }
                (kind, lower, upper, members)
            })
            .collect();
        group_keys.sort_unstable();
        let key = WeightedThresholdKey {
            algorithm,
            coefficients,
            groups: group_keys,
            bound: if inverted { -adjusted - 1 } else { adjusted },
        };
        let existing = self
            .counters
            .as_ref()
            .unwrap()
            .weighted_thresholds
            .get(&key)
            .copied();
        tracing::debug!(
            ?algorithm,
            bound,
            inputs = terms.len(),
            reused = existing.is_some(),
            "compiling shared weighted threshold"
        );
        let predicate = if let Some(predicate) = existing {
            predicate
        } else {
            let predicate = self.instance.new_lit();
            if algorithm == PbEncoding::RustsatBinaryAdder
                || (algorithm == PbEncoding::RustsatGeneralizedTotalizer
                    && terms.len() <= MAX_SHARED_WEIGHTED_INPUTS)
            {
                self.guarded_pseudo_boolean(
                    algorithm,
                    CardinalityRelation::AtMost,
                    bound,
                    terms.to_vec(),
                    groups,
                    Some(predicate),
                )?;
                self.guarded_pseudo_boolean(
                    algorithm,
                    CardinalityRelation::AtLeast,
                    bound + 1,
                    terms.to_vec(),
                    groups,
                    Some(!predicate),
                )?;
            } else {
                // DPW controls are bound-specific; Pindakaas is one-shot. Large GTE
                // predicates retain the previous implication construction for propagation.
                // Repeated bounds still share the complete equivalence.
                let mut one_shot = Compiler {
                    instance: self.instance,
                    variables: self.variables,
                    counters: None,
                };
                one_shot.integer_relation(
                    algorithm,
                    Term::Literal(predicate),
                    crate::ast::sat_decision::IntegerRelation::LessEqual,
                    bound,
                    terms,
                    groups,
                )?;
            }
            self.counters
                .as_deref_mut()
                .unwrap()
                .weighted_thresholds
                .insert(key, if inverted { !predicate } else { predicate });
            if inverted { !predicate } else { predicate }
        };
        self.equate(
            output,
            Term::Literal(if inverted { !predicate } else { predicate }),
        );
        Ok(())
    }

    fn cached_weighted_bound(
        &mut self,
        algorithm: crate::ast::sat_decision::PbEncoding,
        relation: crate::ast::sat_decision::CardinalityRelation,
        bound: usize,
        positive: Vec<(Lit, usize)>,
        guard: Option<Lit>,
    ) -> Result<(), SolverError> {
        use crate::ast::sat_decision::{CardinalityRelation, PbEncoding};
        use rustsat::encodings::pb::{BoundUpper, BoundUpperIncremental};
        let total: usize = positive.iter().map(|(_, weight)| *weight).sum();
        let mut bounds = Vec::new();
        if relation != CardinalityRelation::AtLeast {
            bounds.push((positive.clone(), bound));
        }
        if relation != CardinalityRelation::AtMost {
            bounds.push((
                positive
                    .iter()
                    .map(|(lit, weight)| (!*lit, *weight))
                    .collect(),
                total - bound,
            ));
        }
        for (mut inputs, upper) in bounds {
            inputs.sort_unstable();
            let key = (algorithm, inputs);
            let cache = self.counters.as_deref_mut().unwrap();
            tracing::debug!(
                ?algorithm,
                bound = upper,
                inputs = key.1.len(),
                reused = cache.weighted.contains_key(&key),
                "compiling shared weighted counter"
            );
            let encoder =
                cache
                    .weighted
                    .entry(key)
                    .or_insert_with_key(|(_, inputs)| match algorithm {
                        PbEncoding::RustsatGeneralizedTotalizer => {
                            WeightedEncoder::Totalizer(inputs.iter().copied().collect())
                        }
                        PbEncoding::RustsatBinaryAdder => {
                            WeightedEncoder::Adder(inputs.iter().copied().collect())
                        }
                        _ => unreachable!(),
                    });
            let mut cnf = Cnf::new();
            macro_rules! encode {
                ($encoder:expr) => {{
                    $encoder
                        .encode_ub_change(upper..=upper, &mut cnf, self.instance.var_manager_mut())
                        .map_err(|e| {
                            SolverError::Runtime(format!("Incremental PB encoding failed: {e}"))
                        })?;
                    $encoder.enforce_ub(upper).map_err(|e| {
                        SolverError::Runtime(format!("Incremental PB bound failed: {e}"))
                    })?
                }};
            }
            let enforcement = match encoder {
                WeightedEncoder::Totalizer(encoder) => encode!(encoder),
                WeightedEncoder::Adder(encoder) => encode!(encoder),
            };
            for clause in cnf {
                self.instance.add_clause(clause);
            }
            for literal in enforcement {
                self.assert_guarded(Term::Literal(literal), guard);
            }
        }
        Ok(())
    }

    fn pseudo_boolean(
        &mut self,
        algorithm: crate::ast::sat_decision::PbEncoding,
        relation: crate::ast::sat_decision::CardinalityRelation,
        bound: i64,
        terms: Vec<(i64, Term)>,
        groups: &[crate::ast::sat_decision::PbTermGroup],
    ) -> Result<(), SolverError> {
        self.guarded_pseudo_boolean(algorithm, relation, bound, terms, groups, None)
    }

    fn guarded_pseudo_boolean(
        &mut self,
        algorithm: crate::ast::sat_decision::PbEncoding,
        relation: crate::ast::sat_decision::CardinalityRelation,
        bound: i64,
        terms: Vec<(i64, Term)>,
        groups: &[crate::ast::sat_decision::PbTermGroup],
        guard: Option<Lit>,
    ) -> Result<(), SolverError> {
        use crate::ast::sat_decision::{CardinalityRelation, PbEncoding};
        if !groups.is_empty()
            && matches!(
                algorithm,
                PbEncoding::PindakaasBdd | PbEncoding::PindakaasSwc
            )
            && relation == CardinalityRelation::Exactly
        {
            // The library's structured choice views are safe for inequalities;
            // direct equality can reject valid one-hot assignments in 0.5.1.
            self.guarded_pseudo_boolean(
                algorithm,
                CardinalityRelation::AtMost,
                bound,
                terms.clone(),
                groups,
                guard,
            )?;
            return self.guarded_pseudo_boolean(
                algorithm,
                CardinalityRelation::AtLeast,
                bound,
                terms,
                groups,
                guard,
            );
        }
        // Aggregate by variable before making weights positive, retaining multiplicity
        // and cancelling complements. Widening keeps signed boundary values safe.
        let (mut bound, coefficients) = canonical_pb_terms(bound, &terms);
        let structured_input = (!groups.is_empty()
            && matches!(
                algorithm,
                PbEncoding::PindakaasBdd | PbEncoding::PindakaasSwc
            ))
        .then(|| (bound, coefficients.clone()));
        let mut total = 0i128;
        let mut positive = Vec::new();
        for (literal, weight) in coefficients {
            if weight == 0 {
                continue;
            }
            let (literal, weight) = if weight < 0 {
                bound -= weight;
                (!literal, -weight)
            } else {
                (literal, weight)
            };
            total += weight;
            positive.push((literal, weight));
        }
        let impossible = match relation {
            CardinalityRelation::AtMost => bound < 0,
            CardinalityRelation::AtLeast => bound > total,
            CardinalityRelation::Exactly => bound < 0 || bound > total,
        };
        if impossible {
            self.assert_guarded(Term::Constant(false), guard);
            return Ok(());
        }
        if (relation == CardinalityRelation::AtMost && bound >= total)
            || (relation == CardinalityRelation::AtLeast && bound <= 0)
        {
            return Ok(());
        }
        // Preserve the checked library range for nontrivial bounds before pruning.
        if bound != 0
            && bound != total
            && (total >= isize::MAX as i128 || total >= i128::from(i64::MAX))
        {
            return Err(SolverError::ModelInvalid(
                "Pseudo-Boolean coefficient sum exceeds the library range".into(),
            ));
        }
        if guard.is_some()
            && self.counters.is_some()
            && (algorithm == PbEncoding::RustsatBinaryAdder
                || (algorithm == PbEncoding::RustsatGeneralizedTotalizer
                    && positive.len() <= MAX_SHARED_WEIGHTED_INPUTS))
            && bound != 0
            && bound != total
        {
            return self.cached_weighted_bound(
                algorithm,
                relation,
                bound as usize,
                positive
                    .into_iter()
                    .map(|(lit, weight)| (lit, weight as usize))
                    .collect(),
                guard,
            );
        }
        // An upper/equality bound forbids every individual positive term above it.
        // Remove those terms before constructing a counter, including its inverted lower side.
        if structured_input.is_none() && relation != CardinalityRelation::AtLeast {
            let mut retained = Vec::new();
            total = 0;
            for (literal, weight) in positive {
                if weight > bound {
                    self.assert_guarded(Term::Literal(!literal), guard);
                } else {
                    total += weight;
                    retained.push((literal, weight));
                }
            }
            positive = retained;
            if relation == CardinalityRelation::Exactly && bound > total {
                self.assert_guarded(Term::Constant(false), guard);
                return Ok(());
            }
            if relation == CardinalityRelation::AtMost && bound >= total {
                return Ok(());
            }
        }
        if bound == 0 || bound == total {
            for (literal, _) in positive {
                self.assert_guarded(
                    Term::Literal(if bound == total { literal } else { !literal }),
                    guard,
                );
            }
            return Ok(());
        }
        let positive: Vec<_> = positive
            .into_iter()
            .map(|(literal, weight)| (literal, weight as usize))
            .collect();
        // Preserve unconditional pruning before sharing; wide assertions retain
        // the library's original BoundBoth construction and propagation.
        if guard.is_none()
            && self.counters.is_some()
            && positive.len() <= MAX_SHARED_WEIGHTED_INPUTS
            && matches!(
                algorithm,
                PbEncoding::RustsatGeneralizedTotalizer | PbEncoding::RustsatBinaryAdder
            )
        {
            return self.cached_weighted_bound(
                algorithm,
                relation,
                bound as usize,
                positive,
                guard,
            );
        }
        match algorithm {
            PbEncoding::RustsatGeneralizedTotalizer
            | PbEncoding::RustsatBinaryAdder
            | PbEncoding::RustsatDynamicPolyWatchdog => {
                debug_assert!(guard.is_none());
                use rustsat::{
                    encodings::pb::{
                        BinaryAdder, BoundBoth, DoubleGeneralizedTotalizer, DynamicPolyWatchdog,
                        simulators::{Double, Inverted},
                    },
                    types::constraints::PbConstraint,
                };
                let constraint = match relation {
                    CardinalityRelation::AtMost => {
                        PbConstraint::new_ub_unsigned(positive, bound as isize)
                    }
                    CardinalityRelation::AtLeast => {
                        PbConstraint::new_lb_unsigned(positive, bound as isize)
                    }
                    CardinalityRelation::Exactly => {
                        PbConstraint::new_eq_unsigned(positive, bound as isize)
                    }
                };
                let mut cnf = Cnf::new();
                // Use the selected implementation directly, avoiding the convenience
                // dispatcher's implicit switch to its default cardinality encoder.
                let result = match algorithm {
                    PbEncoding::RustsatGeneralizedTotalizer => {
                        DoubleGeneralizedTotalizer::encode_constr(
                            constraint,
                            &mut cnf,
                            self.instance.var_manager_mut(),
                        )
                    }
                    PbEncoding::RustsatBinaryAdder => BinaryAdder::encode_constr(
                        constraint,
                        &mut cnf,
                        self.instance.var_manager_mut(),
                    ),
                    PbEncoding::RustsatDynamicPolyWatchdog => {
                        // The inverted copy supplies lower bounds; precision remains exact.
                        Double::<DynamicPolyWatchdog, Inverted<DynamicPolyWatchdog>>::encode_constr(
                            constraint,
                            &mut cnf,
                            self.instance.var_manager_mut(),
                        )
                    }
                    _ => unreachable!(),
                };
                result.map_err(|error| {
                    SolverError::Runtime(format!("Pseudo-Boolean encoder failed: {error}"))
                })?;
                for clause in cnf {
                    self.instance.add_clause(clause);
                }
            }
            PbEncoding::PindakaasBdd | PbEncoding::PindakaasSwc => {
                use pindakaas::{
                    Encoder,
                    bool_linear::{
                        BddEncoder, BoolLinAggregator, BoolLinExp, BoolLinVariant, BoolLinear,
                        Comparator, SwcEncoder,
                    },
                };
                let mut comparison = match relation {
                    CardinalityRelation::AtMost => Comparator::LessEq,
                    CardinalityRelation::AtLeast => Comparator::GreaterEq,
                    CardinalityRelation::Exactly => Comparator::Equal,
                };
                let (expression, bound) = if let Some((bound, coefficients)) = structured_input {
                    comparison = if relation == CardinalityRelation::Exactly {
                        Comparator::Equal
                    } else {
                        Comparator::LessEq
                    };
                    self.structured_pb_expression(relation, bound, coefficients, &terms, groups)
                } else {
                    (
                        BoolLinExp::from_terms(
                            &positive
                                .into_iter()
                                .map(|(literal, weight)| (pind_lit(literal), weight as i64))
                                .collect::<Vec<_>>(),
                        ),
                        bound as i64,
                    )
                };
                let mut sink = PindakaasSink {
                    instance: self.instance,
                    guard,
                };
                let variant = BoolLinAggregator::default()
                    .aggregate(&mut sink, &BoolLinear::new(expression, comparison, bound));
                macro_rules! encode_variant {
                    ($encoder:expr) => {{
                        let encoder = $encoder;
                        match variant {
                            Ok(BoolLinVariant::Linear(linear)) => {
                                encoder.encode(&mut sink, &linear)
                            }
                            Ok(BoolLinVariant::Cardinality(cardinality)) => {
                                encoder.encode(&mut sink, &cardinality)
                            }
                            Ok(BoolLinVariant::CardinalityOne(cardinality)) => encoder.encode(
                                &mut sink,
                                &pindakaas::cardinality::Cardinality::from(cardinality),
                            ),
                            Ok(BoolLinVariant::Trivial) => Ok(()),
                            Err(error) => Err(error),
                        }
                    }};
                }
                let result = match algorithm {
                    PbEncoding::PindakaasBdd => encode_variant!(BddEncoder::default()),
                    PbEncoding::PindakaasSwc => encode_variant!(SwcEncoder::default()),
                    _ => unreachable!(),
                };
                if result.is_err() {
                    self.assert_guarded(Term::Constant(false), guard);
                }
            }
        }
        Ok(())
    }
    fn structured_pb_expression(
        &mut self,
        relation: crate::ast::sat_decision::CardinalityRelation,
        mut bound: i128,
        mut coefficients: std::collections::BTreeMap<Lit, i128>,
        terms: &[(i64, Term)],
        groups: &[crate::ast::sat_decision::PbTermGroup],
    ) -> (pindakaas::bool_linear::BoolLinExp, i64) {
        use crate::ast::sat_decision::{CardinalityRelation, PbTermStructure};
        use pindakaas::bool_linear::BoolLinExp;
        // Normalise the comparator ourselves: the library's bounded binary path
        // expects unsigned groups and its >= conversion does not retain scaled bounds.
        let inverted = relation == CardinalityRelation::AtLeast;
        if inverted {
            bound = -bound;
            for coefficient in coefficients.values_mut() {
                *coefficient = -*coefficient;
            }
        }
        let mut library_total: i128 = coefficients.values().map(|weight| weight.abs()).sum();
        let mut occurrences = HashMap::<Lit, usize>::new();
        for (_, term) in terms {
            if let Term::Literal(literal) = term {
                *occurrences.entry(literal.var().pos_lit()).or_default() += 1;
            }
        }
        let mut expression = BoolLinExp::default();
        for group in groups {
            let inputs = terms[group.start..group.end]
                .iter()
                .map(|(_, term)| match term {
                    // Choice/chain invariants refer to these polarities. Do not let
                    // library canonicalisation turn negated inputs into a false hint.
                    Term::Literal(literal) if !literal.is_neg() => Some(*literal),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>();
            let Some(inputs) = inputs else { continue };
            let unique: std::collections::HashSet<_> = inputs.iter().copied().collect();
            if unique.len() != inputs.len() {
                continue;
            }
            match group.structure {
                PbTermStructure::Choice | PbTermStructure::Chain => {
                    let weighted: Vec<_> = inputs
                        .iter()
                        .filter_map(|literal| {
                            coefficients
                                .get(literal)
                                .copied()
                                .filter(|weight| *weight != 0)
                                .map(|weight| (pind_lit(*literal), weight as i64))
                        })
                        .collect();
                    if !weighted.is_empty() {
                        if group.structure == PbTermStructure::Choice {
                            let minimum = weighted
                                .iter()
                                .map(|(_, weight)| i128::from(*weight))
                                .min()
                                .unwrap();
                            if minimum < 0 {
                                // AMO normalisation introduces a none-selected literal
                                // and shifts the other weights. Check the expanded sum.
                                let original: i128 = weighted
                                    .iter()
                                    .map(|(_, weight)| i128::from(*weight).abs())
                                    .sum();
                                let expanded: i128 = weighted
                                    .iter()
                                    .map(|(_, weight)| i128::from(*weight))
                                    .sum::<i128>()
                                    - (weighted.len() as i128 + 1) * minimum;
                                let adjusted = library_total - original + expanded;
                                if adjusted >= i128::from(i64::MAX) {
                                    continue;
                                }
                                library_total = adjusted;
                            }
                        }
                        for literal in &inputs {
                            coefficients.remove(literal);
                        }
                        expression = match group.structure {
                            PbTermStructure::Choice => expression.add_choice(&weighted),
                            _ => expression.add_chain(&weighted),
                        };
                    }
                }
                PbTermStructure::BoundedBinary { lower, upper } => {
                    // A bound describes the original occurrence. Combining repeated
                    // bits can change its weights; retain the flat expression then.
                    if inputs.iter().any(|literal| {
                        occurrences[literal] != 1 || !coefficients.contains_key(literal)
                    }) {
                        continue;
                    }
                    let weights: Vec<_> =
                        inputs.iter().map(|literal| coefficients[literal]).collect();
                    let factor = weights[0].abs();
                    if factor == 0
                        || weights.iter().enumerate().any(|(index, weight)| {
                            1i128
                                .checked_shl(index as u32)
                                .and_then(|power| factor.checked_mul(power))
                                != Some(weight.abs())
                        })
                    {
                        continue;
                    }
                    let constant: i128 = weights.iter().copied().filter(|weight| *weight < 0).sum();
                    let (lower, upper) = if inverted {
                        (-i128::from(upper), -i128::from(lower))
                    } else {
                        (i128::from(lower), i128::from(upper))
                    };
                    let lower = lower - constant;
                    let upper = upper - constant;
                    let capacity: i128 = weights.iter().map(|weight| weight.abs()).sum();
                    if lower < 0 || upper > capacity || lower % factor != 0 || upper % factor != 0 {
                        continue;
                    }
                    let weighted: Vec<_> = inputs
                        .into_iter()
                        .zip(weights)
                        .map(|(literal, weight)| {
                            coefficients.remove(&literal);
                            let literal = if weight < 0 {
                                // Positive proxy identifiers preserve complemented-bit
                                // structure through the library's variable aggregation.
                                let proxy = self.instance.new_lit();
                                self.instance
                                    .add_clause(atomics::lit_impl_lit(proxy, !literal));
                                self.instance
                                    .add_clause(atomics::lit_impl_lit(!literal, proxy));
                                proxy
                            } else {
                                literal
                            };
                            (pind_lit(literal), weight.abs() as i64)
                        })
                        .collect();
                    bound -= constant;
                    expression = expression.add_bounded_log_encoding(
                        &weighted,
                        (lower / factor) as i64,
                        (upper / factor) as i64,
                    );
                }
            }
        }
        let free: Vec<_> = coefficients
            .into_iter()
            .filter(|(_, weight)| *weight != 0)
            .map(|(literal, weight)| (pind_lit(literal), weight as i64))
            .collect();
        expression += BoolLinExp::from_terms(&free);
        (expression, bound as i64)
    }
    fn cached_cardinality(
        &mut self,
        algorithm: crate::ast::sat_decision::CardinalityEncoding,
        relation: crate::ast::sat_decision::CardinalityRelation,
        bound: i64,
        mut literals: Vec<Lit>,
        guard: Option<Lit>,
    ) -> Result<(), SolverError> {
        use crate::ast::sat_decision::{CardinalityEncoding, CardinalityRelation};
        use rustsat::encodings::card::{
            BoundLower, BoundLowerIncremental, BoundUpper, BoundUpperIncremental,
        };
        // Input order has no semantic meaning, but multiplicity and polarity do.
        literals.sort_unstable();
        let cache = self.counters.as_deref_mut().unwrap();
        tracing::debug!(
            ?algorithm,
            ?relation,
            bound,
            inputs = literals.len(),
            reused = cache.occurrences.contains_key(&literals),
            "compiling shared cardinality bound"
        );
        let inputs = if let Some(inputs) = cache.occurrences.get(&literals) {
            inputs.clone()
        } else {
            let mut seen = std::collections::HashSet::new();
            let inputs: Vec<_> = literals
                .iter()
                .copied()
                .map(|literal| {
                    if seen.insert(literal.var()) {
                        literal
                    } else {
                        let alias = self.instance.new_lit();
                        self.instance
                            .add_clause(atomics::lit_impl_lit(alias, literal));
                        self.instance
                            .add_clause(atomics::lit_impl_lit(literal, alias));
                        alias
                    }
                })
                .collect();
            cache.occurrences.insert(literals.clone(), inputs.clone());
            inputs
        };
        if algorithm == CardinalityEncoding::RustsatTotalizer {
            let encoder = cache
                .totalizers
                .entry(literals)
                .or_insert_with(|| inputs.into_iter().collect());
            let mut cnf = Cnf::new();
            let mut enforcement = Vec::new();
            let bound = bound as usize;
            if relation != CardinalityRelation::AtLeast {
                encoder
                    .encode_ub_change(bound..=bound, &mut cnf, self.instance.var_manager_mut())
                    .map_err(|e| {
                        SolverError::Runtime(format!(
                            "Incremental cardinality encoding failed: {e}"
                        ))
                    })?;
                enforcement.extend(encoder.enforce_ub(bound).map_err(|e| {
                    SolverError::Runtime(format!("Cardinality upper bound failed: {e}"))
                })?);
            }
            if relation != CardinalityRelation::AtMost {
                encoder
                    .encode_lb_change(bound..=bound, &mut cnf, self.instance.var_manager_mut())
                    .map_err(|e| {
                        SolverError::Runtime(format!(
                            "Incremental cardinality encoding failed: {e}"
                        ))
                    })?;
                enforcement.extend(encoder.enforce_lb(bound).map_err(|e| {
                    SolverError::Runtime(format!("Cardinality lower bound failed: {e}"))
                })?);
            }
            // Counter structure is unconditional; only enforcement belongs to the guard.
            for clause in cnf {
                self.instance.add_clause(clause);
            }
            for literal in enforcement {
                self.assert_guarded(Term::Literal(literal), guard);
            }
            return Ok(());
        }
        // Pindakaas exposes one-shot networks. Retain a fully equivalent threshold,
        // so another guard or bound direction can reuse its existing network safely.
        let bounds: Vec<_> = match relation {
            CardinalityRelation::AtMost => vec![(bound, true)],
            CardinalityRelation::AtLeast => vec![(bound - 1, false)],
            CardinalityRelation::Exactly => vec![(bound, true), (bound - 1, false)],
        };
        for (upper, positive) in bounds {
            let key = (literals.clone(), upper);
            let threshold =
                if let Some(&literal) = self.counters.as_ref().unwrap().thresholds.get(&key) {
                    literal
                } else {
                    let output = self.instance.new_lit();
                    let mut one_shot = Compiler {
                        instance: self.instance,
                        variables: self.variables,
                        counters: None,
                    };
                    one_shot.cardinality_guarded(
                        algorithm,
                        CardinalityRelation::AtMost,
                        upper,
                        inputs.iter().copied().map(Term::Literal).collect(),
                        Some(output),
                    )?;
                    one_shot.cardinality_guarded(
                        algorithm,
                        CardinalityRelation::AtLeast,
                        upper + 1,
                        inputs.iter().copied().map(Term::Literal).collect(),
                        Some(!output),
                    )?;
                    self.counters
                        .as_deref_mut()
                        .unwrap()
                        .thresholds
                        .insert(key, output);
                    output
                };
            self.assert_guarded(
                Term::Literal(if positive { threshold } else { !threshold }),
                guard,
            );
        }
        Ok(())
    }

    fn count_relation(
        &mut self,
        algorithm: crate::ast::sat_decision::CardinalityEncoding,
        amo: Option<crate::ast::sat_decision::AmoEncoding>,
        output: Term,
        relation: crate::ast::sat_decision::IntegerRelation,
        bound: i64,
        terms: Vec<Term>,
    ) -> Result<(), SolverError> {
        use crate::ast::sat_decision::IntegerRelation;
        let bound = i128::from(bound)
            - terms
                .iter()
                .filter(|term| matches!(term, Term::Constant(true)))
                .count() as i128;
        let literals: Vec<_> = terms
            .into_iter()
            .filter_map(|term| match term {
                Term::Literal(literal) => Some(literal),
                _ => None,
            })
            .collect();
        match relation {
            IntegerRelation::Less => self.count_upper(algorithm, amo, output, bound - 1, &literals),
            IntegerRelation::LessEqual => {
                self.count_upper(algorithm, amo, output, bound, &literals)
            }
            IntegerRelation::Greater => {
                self.count_upper(algorithm, amo, output.negated(), bound, &literals)
            }
            IntegerRelation::GreaterEqual => {
                self.count_upper(algorithm, amo, output.negated(), bound - 1, &literals)
            }
            IntegerRelation::Equal | IntegerRelation::NotEqual => {
                let upper = Term::Literal(self.instance.new_lit());
                let below = Term::Literal(self.instance.new_lit());
                self.count_upper(algorithm, amo, upper, bound, &literals)?;
                self.count_upper(algorithm, amo, below, bound - 1, &literals)?;
                let value = self.combine(true, vec![upper, below.negated()]);
                self.equate(
                    output,
                    if relation == IntegerRelation::Equal {
                        value
                    } else {
                        value.negated()
                    },
                );
                Ok(())
            }
        }
    }

    fn count_upper(
        &mut self,
        algorithm: crate::ast::sat_decision::CardinalityEncoding,
        amo: Option<crate::ast::sat_decision::AmoEncoding>,
        output: Term,
        bound: i128,
        literals: &[Lit],
    ) -> Result<(), SolverError> {
        use crate::ast::sat_decision::CardinalityRelation;
        if bound < 0 || bound >= literals.len() as i128 {
            self.equate(output, Term::Constant(bound >= 0));
            return Ok(());
        }
        let terms = || literals.iter().copied().map(Term::Literal).collect();
        if !matches!(output, Term::Constant(false)) {
            let guard = match output {
                Term::Literal(literal) => Some(literal),
                _ => None,
            };
            if bound == 1 {
                let amo = amo.ok_or_else(|| {
                    SolverError::ModelInvalid("Unresolved count AMO encoding decision".into())
                })?;
                self.amo_guarded(amo, literals.to_vec(), guard)?;
            } else {
                self.cardinality_guarded(
                    algorithm,
                    CardinalityRelation::AtMost,
                    bound as i64,
                    terms(),
                    guard,
                )?;
            }
        }
        if !matches!(output, Term::Constant(true)) {
            let guard = match output {
                Term::Literal(literal) => Some(!literal),
                _ => None,
            };
            self.cardinality_guarded(
                algorithm,
                CardinalityRelation::AtLeast,
                (bound + 1) as i64,
                terms(),
                guard,
            )?;
        }
        Ok(())
    }

    fn cardinality(
        &mut self,
        algorithm: crate::ast::sat_decision::CardinalityEncoding,
        relation: crate::ast::sat_decision::CardinalityRelation,
        bound: i64,
        terms: Vec<Term>,
    ) -> Result<(), SolverError> {
        self.cardinality_guarded(algorithm, relation, bound, terms, None)
    }

    fn cardinality_guarded(
        &mut self,
        algorithm: crate::ast::sat_decision::CardinalityEncoding,
        relation: crate::ast::sat_decision::CardinalityRelation,
        bound: i64,
        terms: Vec<Term>,
        guard: Option<Lit>,
    ) -> Result<(), SolverError> {
        use crate::ast::sat_decision::{CardinalityEncoding, CardinalityRelation};
        let constants = terms
            .iter()
            .filter(|term| matches!(term, Term::Constant(true)))
            .count() as i64;
        let bound = bound
            .checked_sub(constants)
            .ok_or_else(|| SolverError::ModelInvalid("Cardinality bound overflow".into()))?;
        let literals: Vec<_> = terms
            .into_iter()
            .filter_map(|term| match term {
                Term::Literal(lit) => Some(lit),
                _ => None,
            })
            .collect();
        let size = literals.len() as i64;
        let impossible = match relation {
            CardinalityRelation::AtMost => bound < 0,
            CardinalityRelation::AtLeast => bound > size,
            CardinalityRelation::Exactly => bound < 0 || bound > size,
        };
        if impossible {
            self.assert_guarded(Term::Constant(false), guard);
            return Ok(());
        }
        if (relation == CardinalityRelation::AtMost && bound >= size)
            || (relation == CardinalityRelation::AtLeast && bound <= 0)
        {
            return Ok(());
        }
        if bound == 0 || bound == size {
            for literal in literals {
                self.assert_guarded(
                    Term::Literal(if bound == size { literal } else { !literal }),
                    guard,
                );
            }
            return Ok(());
        }
        if self.counters.is_some() {
            return self.cached_cardinality(algorithm, relation, bound, literals, guard);
        }
        match algorithm {
            CardinalityEncoding::RustsatTotalizer => {
                use rustsat::{
                    encodings::card::{Totalizer, encode_cardinality_constraint},
                    types::constraints::CardConstraint,
                };
                let constraint = match relation {
                    CardinalityRelation::AtMost => CardConstraint::new_ub(literals, bound as usize),
                    CardinalityRelation::AtLeast => {
                        CardConstraint::new_lb(literals, bound as usize)
                    }
                    CardinalityRelation::Exactly => {
                        CardConstraint::new_eq(literals, bound as usize)
                    }
                };
                let mut cnf = Cnf::new();
                encode_cardinality_constraint::<Totalizer, _>(
                    constraint,
                    &mut cnf,
                    self.instance.var_manager_mut(),
                )
                .map_err(|error| SolverError::Runtime(format!("Totalizer failed: {error}")))?;
                for mut clause in cnf {
                    if let Some(guard) = guard {
                        clause.add(!guard);
                    }
                    self.instance.add_clause(clause);
                }
            }
            CardinalityEncoding::PindakaasSortingNetwork => {
                use pindakaas::{
                    Encoder,
                    bool_linear::{
                        BoolLinAggregator, BoolLinExp, BoolLinVariant, BoolLinear, Comparator,
                    },
                    cardinality::SortingNetworkEncoder,
                };
                // Pindakaas cardinality inputs must use distinct variables. Alias repeats and
                // opposite polarities instead of allowing aggregation into weighted PB terms.
                let mut seen = std::collections::HashSet::new();
                let literals = literals
                    .into_iter()
                    .map(|literal| {
                        let literal = if seen.insert(literal.var()) {
                            literal
                        } else {
                            let alias = self.instance.new_lit();
                            self.instance
                                .add_clause(atomics::lit_impl_lit(alias, literal));
                            self.instance
                                .add_clause(atomics::lit_impl_lit(literal, alias));
                            alias
                        };
                        pind_lit(literal)
                    })
                    .collect::<Vec<_>>();
                let comparison = match relation {
                    CardinalityRelation::AtMost => Comparator::LessEq,
                    CardinalityRelation::AtLeast => Comparator::GreaterEq,
                    CardinalityRelation::Exactly => Comparator::Equal,
                };
                let expression = BoolLinExp::from_terms(
                    &literals.into_iter().map(|lit| (lit, 1)).collect::<Vec<_>>(),
                );
                let mut sink = PindakaasSink {
                    instance: self.instance,
                    guard,
                };
                let variant = BoolLinAggregator::default()
                    .aggregate(&mut sink, &BoolLinear::new(expression, comparison, bound));
                let encoder = SortingNetworkEncoder::default();
                let result = match variant {
                    Ok(BoolLinVariant::Cardinality(cardinality)) => {
                        encoder.encode(&mut sink, &cardinality)
                    }
                    Ok(BoolLinVariant::CardinalityOne(cardinality)) => encoder.encode(
                        &mut sink,
                        &pindakaas::cardinality::Cardinality::from(cardinality),
                    ),
                    Ok(BoolLinVariant::Trivial) => Ok(()),
                    Ok(BoolLinVariant::Linear(_)) => {
                        return Err(SolverError::ModelInvalid(
                            "Cardinality normalisation unexpectedly produced weighted terms".into(),
                        ));
                    }
                    Err(error) => Err(error),
                };
                if result.is_err() {
                    self.assert_guarded(Term::Constant(false), guard);
                }
            }
        }
        Ok(())
    }
    fn amo(
        &mut self,
        algorithm: crate::ast::sat_decision::AmoEncoding,
        inputs: Vec<Lit>,
    ) -> Result<(), SolverError> {
        self.amo_guarded(algorithm, inputs, None)
    }

    fn amo_guarded(
        &mut self,
        algorithm: crate::ast::sat_decision::AmoEncoding,
        inputs: Vec<Lit>,
        guard: Option<Lit>,
    ) -> Result<(), SolverError> {
        use crate::ast::sat_decision::AmoEncoding;
        use rustsat::encodings::am1::{self, Encode};
        let mut cnf = Cnf::new();
        let manager = self.instance.var_manager_mut();
        let result = match algorithm {
            AmoEncoding::Pairwise => am1::Pairwise::from(inputs).encode(&mut cnf, manager),
            AmoEncoding::Ladder => am1::Ladder::from(inputs).encode(&mut cnf, manager),
            AmoEncoding::Bitwise => am1::Bitwise::from(inputs).encode(&mut cnf, manager),
            AmoEncoding::Commander => am1::Commander::<4>::from(inputs).encode(&mut cnf, manager),
            AmoEncoding::Bimander => am1::Bimander::<4>::from(inputs).encode(&mut cnf, manager),
            AmoEncoding::TwoProduct => {
                am1::TwoProduct::<am1::Pairwise>::from(inputs).encode(&mut cnf, manager)
            }
        };
        result.map_err(|error| SolverError::Runtime(format!("AMO encoder failed: {error}")))?;
        for mut clause in cnf {
            if let Some(guard) = guard {
                clause.add(!guard);
            }
            self.instance.add_clause(clause);
        }
        Ok(())
    }
    fn assert_guarded(&mut self, term: Term, guard: Option<Lit>) {
        if let Some(guard) = guard {
            match term {
                Term::Constant(true) => (),
                Term::Constant(false) => self.assert(Term::Literal(!guard)),
                Term::Literal(literal) => self
                    .instance
                    .add_clause(atomics::lit_impl_lit(guard, literal)),
            }
        } else {
            self.assert(term);
        }
    }
    fn assert(&mut self, term: Term) {
        match term {
            Term::Constant(true) => (),
            Term::Constant(false) => self.instance.add_clause(Clause::new()),
            Term::Literal(lit) => self.instance.add_clause([lit].into_iter().collect()),
        }
    }
    fn resolve_alias(&self, mut term: Term) -> Term {
        while let Term::Literal(literal) = term {
            let positive = literal.var().pos_lit();
            let Some(value) = self
                .counters
                .as_ref()
                .and_then(|cache| cache.aliases.get(&positive))
            else {
                break;
            };
            term = if literal.is_pos() {
                *value
            } else {
                value.negated()
            };
        }
        term
    }

    // Keep the emitted equivalence: earlier clauses may already use the original literal.
    fn retain_boolean_alias(&mut self, output: Term, value: Term) {
        let output = self.resolve_alias(output);
        let value = self.resolve_alias(value);
        let Term::Literal(output) = output else {
            return;
        };
        if matches!(value, Term::Literal(value) if value.var() == output.var()) {
            return;
        }
        if let Some(cache) = self.counters.as_mut() {
            cache.aliases.insert(
                output.var().pos_lit(),
                if output.is_pos() {
                    value
                } else {
                    value.negated()
                },
            );
        }
    }

    fn combine(&mut self, and: bool, terms: Vec<Term>) -> Term {
        let mut literals = Vec::new();
        for term in terms {
            match self.resolve_alias(term) {
                Term::Constant(value) if value != and => return Term::Constant(value),
                Term::Constant(_) => (),
                Term::Literal(lit) => literals.push(lit),
            }
        }
        match literals.as_slice() {
            [] => Term::Constant(and),
            [lit] => Term::Literal(*lit),
            _ => {
                // Gate inputs are commutative; aliases join independently rebuilt projections.
                literals.sort_unstable();
                let key = (and, literals.clone());
                let existing = self
                    .counters
                    .as_ref()
                    .and_then(|cache| cache.gates.get(&key))
                    .copied();
                tracing::debug!(reused = existing.is_some(), "compiling shared Boolean gate");
                if let Some(output) = existing {
                    return self.resolve_alias(Term::Literal(output));
                }
                let output = self.instance.new_lit();
                if and {
                    for clause in atomics::lit_impl_cube(output, &literals) {
                        self.instance.add_clause(clause);
                    }
                    self.instance
                        .add_clause(atomics::cube_impl_lit(&literals, output));
                } else {
                    for clause in atomics::clause_impl_lit(&literals, output) {
                        self.instance.add_clause(clause);
                    }
                    self.instance
                        .add_clause(atomics::lit_impl_clause(output, &literals));
                }
                if let Some(cache) = self.counters.as_mut() {
                    cache.gates.insert(key, output);
                }
                Term::Literal(output)
            }
        }
    }
    fn encode(&mut self, expression: &Expression) -> Result<Term, SolverError> {
        Ok(match expression {
            Expression::Atomic(_, Atom::Literal(Literal::Bool(value))) => Term::Constant(*value),
            Expression::Atomic(_, Atom::Reference(reference)) => {
                if !reference.domain().is_some_and(|domain| domain.is_bool()) {
                    return Err(SolverError::ModelInvalid(format!(
                        "Non-Boolean SAT reference: {reference}"
                    )));
                }
                let literal = *self
                    .variables
                    .entry(reference.name().clone())
                    .or_insert_with(|| self.instance.new_lit());
                self.resolve_alias(Term::Literal(literal))
            }
            Expression::Not(_, inner) => self.encode(inner)?.negated(),
            Expression::And(_, children) | Expression::Or(_, children) => {
                let Expression::AbstractLiteral(_, AbstractLiteral::Matrix(children, _)) =
                    children.as_ref()
                else {
                    return Err(SolverError::ModelInvalid(
                        "SAT Boolean operands must be an explicit matrix".into(),
                    ));
                };
                let terms = children
                    .iter()
                    .map(|child| self.encode(child))
                    .collect::<Result<Vec<_>, _>>()?;
                self.combine(matches!(expression, Expression::And(..)), terms)
            }
            Expression::Imply(_, left, right) => {
                let left = self.encode(left)?.negated();
                let right = self.encode(right)?;
                self.combine(false, vec![left, right])
            }
            Expression::Iff(_, left, right) => {
                let left = self.encode(left)?;
                let right = self.encode(right)?;
                let forward = self.combine(false, vec![left.negated(), right]);
                let backward = self.combine(false, vec![right.negated(), left]);
                self.combine(true, vec![forward, backward])
            }
            _ => {
                return Err(SolverError::ModelInvalid(format!(
                    "Unsupported semantic SAT operation: {expression}"
                )));
            }
        })
    }
}

fn pind_lit(literal: Lit) -> pindakaas::Lit {
    pindakaas::Lit::from_raw(std::num::NonZeroI32::new(literal.to_ipasir()).unwrap())
}
/// Bridge both providers to the RustSAT allocator; no independent variable namespace.
struct PindakaasSink<'a> {
    instance: &'a mut SatInstance,
    guard: Option<Lit>,
}
impl pindakaas::ClauseDatabase for PindakaasSink<'_> {
    fn add_clause_from_slice(
        &mut self,
        clause: &[pindakaas::Lit],
    ) -> Result<(), pindakaas::Unsatisfiable> {
        self.instance.add_clause(
            clause
                .iter()
                .map(|lit| {
                    let raw: std::num::NonZeroI32 = (*lit).into();
                    Lit::from_ipasir(raw.get()).unwrap()
                })
                .chain(self.guard.map(|guard| !guard))
                .collect(),
        );
        // Pindakaas normalisation relies on contradiction() returning this error.
        if clause.is_empty() {
            Err(pindakaas::Unsatisfiable)
        } else {
            Ok(())
        }
    }
    fn new_var_range(&mut self, len: usize) -> pindakaas::VarRange {
        if len == 0 {
            return pindakaas::VarRange::empty();
        }
        let first = pind_lit(self.instance.new_lit()).var();
        let mut last = first;
        for _ in 1..len {
            last = pind_lit(self.instance.new_lit()).var();
        }
        pindakaas::VarRange::new(first, last)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{DeclarationPtr, Domain, Metadata, Moo, Reference};
    use rustsat::{
        instances::{BasicVarManager, Cnf},
        solvers::{Solve, SolveIncremental, SolverResult},
    };
    use rustsat_cadical::CaDiCaL;

    #[test]
    fn rebuilt_boolean_projections_share_literals_across_batches() {
        use crate::ast::sat_decision::{
            EncodingSelection, IntegerRelation, PbEncoding, SelectionProvenance,
        };
        use rustsat::instances::ManageVars;

        for algorithm in PbEncoding::ALL {
            let inputs: Vec<_> = ["a", "b"]
                .into_iter()
                .map(|name| DeclarationPtr::new_find(Name::User(name.into()), Domain::bool()))
                .collect();
            let expressions: Vec<Expression> = inputs
                .iter()
                .map(|decl| Reference::new(decl.clone()).into())
                .collect();
            let mut cache = EncodingCache::default();
            let mut map = HashMap::new();
            let mut solver = CaDiCaL::default();
            let mut next_free = 0;
            let mut previous_projection = None;
            let mut outputs = Vec::new();
            let mut named_projections = Vec::new();
            for batch in 0..3 {
                let mut instance: SatInstance = SatInstance::new();
                instance
                    .var_manager_mut()
                    .increase_next_free(rustsat::types::Var::new(next_free));
                let declarations: Vec<_> = (0..4)
                    .map(|index| {
                        DeclarationPtr::new_find(
                            Name::User(format!("batch{batch}_{index}").into()),
                            Domain::bool(),
                        )
                    })
                    .collect();
                let refs: Vec<Expression> = declarations
                    .iter()
                    .map(|decl| Reference::new(decl.clone()).into())
                    .collect();
                let mut children = expressions.clone();
                if batch == 1 {
                    children.reverse();
                }
                let conjunction = Expression::And(
                    Metadata::new(),
                    Moo::new(crate::into_matrix_expr!(children)),
                );
                let decisions = [
                    SatEncodingDecision::Boolean {
                        output: refs[0].clone(),
                        expression: conjunction,
                    },
                    SatEncodingDecision::Boolean {
                        output: refs[1].clone(),
                        expression: Expression::Not(Metadata::new(), Moo::new(refs[0].clone())),
                    },
                    SatEncodingDecision::Boolean {
                        output: refs[2].clone(),
                        expression: Expression::Not(Metadata::new(), Moo::new(refs[1].clone())),
                    },
                    SatEncodingDecision::IntegerRelation {
                        output: refs[3].clone(),
                        terms: vec![(3, refs[2].clone()), (2, expressions[0].clone())],
                        groups: vec![],
                        relation: IntegerRelation::LessEqual,
                        bound: if batch == 1 { 0 } else { 3 },
                        encoding: Some(EncodingSelection {
                            algorithm,
                            provenance: SelectionProvenance::ExplicitConfiguration,
                        }),
                    },
                ];
                compile_decisions_with_cache(&decisions, &mut instance, &mut map, &mut cache)
                    .unwrap();
                let compiler = Compiler {
                    instance: &mut instance,
                    variables: &mut map,
                    counters: Some(&mut cache),
                };
                let projection = compiler
                    .resolve_alias(Term::Literal(compiler.variables[&declarations[2].name()]));
                if let Some(previous) = previous_projection {
                    assert!(projection == previous);
                }
                previous_projection = Some(projection);
                assert_eq!(
                    cache.gates.len(),
                    1,
                    "{algorithm}: decoder gate should be shared"
                );
                named_projections.push(
                    declarations[..3]
                        .iter()
                        .map(|decl| map[&decl.name()])
                        .collect::<Vec<_>>(),
                );
                outputs.push((map[&declarations[3].name()], if batch == 1 { 0 } else { 3 }));
                let (cnf, manager): (Cnf, BasicVarManager) = instance.into_cnf();
                if batch == 2 {
                    assert_eq!(
                        manager.n_used(),
                        next_free + 4,
                        "{algorithm}: repeated projections and bounds need only new named outputs"
                    );
                }
                next_free = manager.n_used();
                solver.add_cnf(cnf).unwrap();
            }
            for bits in 0..4 {
                let a = bits & 1 != 0;
                let b = bits & 2 != 0;
                let value = 3 * i64::from(a && b) + 2 * i64::from(a);
                let mut assumptions: Vec<_> = inputs
                    .iter()
                    .zip([a, b])
                    .map(|(decl, truth)| {
                        let lit = map[&decl.name()];
                        if truth { lit } else { !lit }
                    })
                    .collect();
                for projections in &named_projections {
                    assumptions.extend(
                        projections
                            .iter()
                            .zip([a && b, !(a && b), a && b])
                            .map(|(&lit, truth)| if truth { lit } else { !lit }),
                    );
                }
                assumptions.extend(
                    outputs
                        .iter()
                        .map(|&(lit, bound)| if value <= bound { lit } else { !lit }),
                );
                assert_eq!(
                    solver.solve_assumps(&assumptions).unwrap(),
                    SolverResult::Sat
                );
                for index in 2..assumptions.len() {
                    assumptions[index] = !assumptions[index];
                    assert_eq!(
                        solver.solve_assumps(&assumptions).unwrap(),
                        SolverResult::Unsat,
                        "{algorithm}: batch projection/output {index}, inputs {bits}"
                    );
                    assumptions[index] = !assumptions[index];
                }
            }
        }
    }

    #[test]
    fn boolean_aliases_retain_existing_uses_and_multiple_definitions() {
        let vars: Vec<_> = ["a", "out"]
            .into_iter()
            .map(|name| DeclarationPtr::new_find(Name::User(name.into()), Domain::bool()))
            .collect();
        let refs: Vec<Expression> = vars
            .iter()
            .map(|decl| Reference::new(decl.clone()).into())
            .collect();
        let mut instance = SatInstance::new();
        let mut map = HashMap::new();
        let decisions = [
            SatEncodingDecision::Assert(refs[1].clone()),
            SatEncodingDecision::Boolean {
                output: refs[1].clone(),
                expression: Expression::Not(Metadata::new(), Moo::new(refs[0].clone())),
            },
            SatEncodingDecision::Boolean {
                output: refs[1].clone(),
                expression: true.into(),
            },
        ];
        compile_decisions(&decisions, &mut instance, &mut map).unwrap();
        let (cnf, _): (Cnf, BasicVarManager) = instance.into_cnf();
        let mut solver = CaDiCaL::default();
        solver.add_cnf(cnf).unwrap();
        for a in [false, true] {
            let lit = map[&vars[0].name()];
            assert_eq!(
                solver.solve_assumps(&[if a { lit } else { !lit }]).unwrap() == SolverResult::Sat,
                !a
            );
        }
    }

    #[test]
    fn semantic_gates_preserve_all_input_and_output_assignments() {
        let variables: Vec<_> = ["a", "b", "out"]
            .into_iter()
            .map(|name| DeclarationPtr::new_find(Name::User(name.into()), Domain::bool()))
            .collect();
        let expressions: Vec<Expression> = variables
            .iter()
            .map(|decl| Reference::new(decl.clone()).into())
            .collect();
        let a = expressions[0].clone();
        let b = expressions[1].clone();
        let matrix = || Moo::new(crate::into_matrix_expr!(vec![a.clone(), b.clone()]));
        let gates = [
            Expression::And(Metadata::new(), matrix()),
            Expression::Or(Metadata::new(), matrix()),
            Expression::Not(Metadata::new(), Moo::new(a.clone())),
            Expression::Iff(Metadata::new(), Moo::new(a.clone()), Moo::new(b.clone())),
            Expression::Imply(Metadata::new(), Moo::new(a), Moo::new(b)),
        ];
        for (gate, expression) in gates.into_iter().enumerate() {
            let mut instance = SatInstance::new();
            let mut map = HashMap::new();
            for variable in &variables {
                map.insert(variable.name().clone(), instance.new_lit());
            }
            compile_decisions(
                &[SatEncodingDecision::Boolean {
                    output: expressions[2].clone(),
                    expression,
                }],
                &mut instance,
                &mut map,
            )
            .unwrap();
            let (cnf, _): (Cnf, BasicVarManager) = instance.into_cnf();
            let mut solver = CaDiCaL::default();
            solver.add_cnf(cnf).unwrap();
            for bits in 0..8 {
                let a = bits & 1 != 0;
                let b = bits & 2 != 0;
                let out = bits & 4 != 0;
                let expected = [a && b, a || b, !a, a == b, !a || b][gate];
                let assumptions: Vec<_> = variables
                    .iter()
                    .zip([a, b, out])
                    .map(|(variable, value)| {
                        let lit = map[&variable.name()];
                        if value { lit } else { !lit }
                    })
                    .collect();
                assert_eq!(
                    solver.solve_assumps(&assumptions).unwrap() == SolverResult::Sat,
                    out == expected,
                    "gate {gate}, assignment {bits}"
                );
            }
        }
    }
}

#[cfg(test)]
mod amo_tests {
    use super::*;
    use crate::ast::sat_decision::{AmoEncoding, EncodingSelection, SelectionProvenance};
    use crate::ast::{DeclarationPtr, Domain, Reference};
    use rustsat::{
        instances::{BasicVarManager, Cnf},
        solvers::{Solve, SolveIncremental, SolverResult},
    };
    use rustsat_cadical::CaDiCaL;

    fn check(
        inputs: Vec<Expression>,
        variables: &[DeclarationPtr],
        algorithm: AmoEncoding,
        expected: impl Fn(usize) -> bool,
    ) {
        let mut instance = SatInstance::new();
        let mut map = HashMap::new();
        for variable in variables {
            map.insert(variable.name().clone(), instance.new_lit());
        }
        compile_decisions(
            &[SatEncodingDecision::AtMostOne {
                inputs,
                encoding: Some(EncodingSelection {
                    algorithm,
                    provenance: SelectionProvenance::ExplicitConfiguration,
                }),
            }],
            &mut instance,
            &mut map,
        )
        .unwrap();
        let (cnf, _): (Cnf, BasicVarManager) = instance.into_cnf();
        let mut solver = CaDiCaL::default();
        solver.add_cnf(cnf).unwrap();
        for bits in 0..(1usize << variables.len()) {
            let assumptions: Vec<_> = variables
                .iter()
                .enumerate()
                .map(|(index, variable)| {
                    let literal = map[&variable.name()];
                    if bits & (1 << index) != 0 {
                        literal
                    } else {
                        !literal
                    }
                })
                .collect();
            assert_eq!(
                solver.solve_assumps(&assumptions).unwrap() == SolverResult::Sat,
                expected(bits),
                "{algorithm}, assignment {bits}"
            );
        }
    }
    #[test]
    fn every_amo_encoder_preserves_projection_and_boundary_sizes() {
        for algorithm in AmoEncoding::ALL {
            for size in [0, 1, 2, 3, 4, 5, 8, 9] {
                let variables: Vec<_> = (0..size)
                    .map(|index| {
                        DeclarationPtr::new_find(
                            Name::User(format!("x{index}").into()),
                            Domain::bool(),
                        )
                    })
                    .collect();
                let inputs = variables
                    .iter()
                    .map(|variable| Reference::new(variable.clone()).into())
                    .collect();
                check(inputs, &variables, algorithm, |bits| bits.count_ones() <= 1);
            }
        }
    }
    #[test]
    fn amo_constants_and_repeated_inputs_keep_their_multiplicity() {
        let variable = DeclarationPtr::new_find(Name::User("x".into()), Domain::bool());
        let x: Expression = Reference::new(variable.clone()).into();
        for algorithm in AmoEncoding::ALL {
            check(
                vec![x.clone(), x.clone()],
                std::slice::from_ref(&variable),
                algorithm,
                |bits| bits == 0,
            );
            check(
                vec![true.into(), x.clone(), false.into()],
                std::slice::from_ref(&variable),
                algorithm,
                |bits| bits == 0,
            );
            check(
                vec![true.into(), true.into(), x.clone()],
                std::slice::from_ref(&variable),
                algorithm,
                |_| false,
            );
        }
    }
    #[test]
    fn unresolved_decisions_are_rejected_at_the_solver_boundary() {
        assert!(
            compile_decisions(
                &[SatEncodingDecision::AtMostOne {
                    inputs: vec![],
                    encoding: None
                }],
                &mut SatInstance::new(),
                &mut HashMap::new()
            )
            .is_err()
        );
    }
}

#[cfg(test)]
mod cardinality_tests {
    use super::*;
    use crate::ast::sat_decision::{
        CardinalityEncoding, CardinalityRelation, EncodingSelection, SelectionProvenance,
    };
    use crate::ast::{DeclarationPtr, Domain, Metadata, Moo, Reference};
    use rustsat::{
        instances::{BasicVarManager, ManageVars},
        solvers::{Solve, SolveIncremental, SolverResult},
    };
    use rustsat_cadical::CaDiCaL;
    #[test]
    fn reusable_cardinality_batches_preserve_guards_and_occurrence_counts() {
        for algorithm in CardinalityEncoding::ALL {
            let mut cache = EncodingCache::default();
            let mut variables = HashMap::new();
            let mut solver = CaDiCaL::default();
            let mut initial: SatInstance = SatInstance::new();
            let inputs: Vec<_> = (0..3).map(|_| initial.new_lit()).collect();
            let (_, mut manager): (Cnf, BasicVarManager) = initial.into_cnf();
            let mut guards = Vec::new();
            // Opposite/repeated occurrences remain unit-weight count inputs.
            let occurrences = vec![inputs[0], !inputs[1], inputs[0], inputs[2], inputs[1]];
            for (batch, bound) in [2, 4, 1, 3, 2, 0, 5].into_iter().enumerate() {
                for relation in [
                    CardinalityRelation::AtMost,
                    CardinalityRelation::AtLeast,
                    CardinalityRelation::Exactly,
                ] {
                    let mut delta: SatInstance = SatInstance::new();
                    delta
                        .var_manager_mut()
                        .increase_next_free(rustsat::types::Var::new(manager.n_used()));
                    let guard = delta.new_lit();
                    guards.push(guard);
                    let mut terms = occurrences.clone();
                    if batch % 2 == 0 {
                        terms.reverse();
                    }
                    let mut compiler = Compiler {
                        instance: &mut delta,
                        variables: &mut variables,
                        counters: Some(&mut cache),
                    };
                    compiler
                        .cardinality_guarded(
                            algorithm,
                            relation,
                            bound,
                            terms.iter().copied().map(Term::Literal).collect(),
                            Some(guard),
                        )
                        .unwrap();
                    let allocated = compiler.instance.var_manager_mut().n_used();
                    let clauses = compiler.instance.cnf().len();
                    compiler
                        .cardinality_guarded(
                            algorithm,
                            relation,
                            bound,
                            terms.iter().copied().map(Term::Literal).collect(),
                            Some(guard),
                        )
                        .unwrap();
                    assert_eq!(
                        compiler.instance.var_manager_mut().n_used(),
                        allocated,
                        "repeated threshold must reuse auxiliaries: {algorithm:?}"
                    );
                    if bound > 0 && bound < occurrences.len() as i64 {
                        assert!(
                            compiler.instance.cnf().len() - clauses <= 2,
                            "repeated threshold must only add guard enforcement"
                        );
                    }
                    let (cnf, next): (Cnf, BasicVarManager) = delta.into_cnf();
                    manager = next;
                    solver.add_cnf(cnf).unwrap();
                    for assignment in 0..8 {
                        let count = 2 * usize::from(assignment & 1 != 0)
                            + 1
                            + usize::from(assignment & 4 != 0);
                        let expected = match relation {
                            CardinalityRelation::AtMost => count <= bound as usize,
                            CardinalityRelation::AtLeast => count >= bound as usize,
                            CardinalityRelation::Exactly => count == bound as usize,
                        };
                        let fixed: Vec<_> = inputs
                            .iter()
                            .enumerate()
                            .map(|(bit, &lit)| {
                                if assignment & (1 << bit) != 0 {
                                    lit
                                } else {
                                    !lit
                                }
                            })
                            .collect();
                        for active in [false, true] {
                            let mut assumptions = fixed.clone();
                            assumptions.extend(
                                guards
                                    .iter()
                                    .map(|&lit| if lit == guard && active { lit } else { !lit }),
                            );
                            assert_eq!(
                                solver.solve_assumps(&assumptions).unwrap() == SolverResult::Sat,
                                !active || expected,
                                "{algorithm:?} {relation:?} bound={bound} assignment={assignment} active={active}"
                            );
                        }
                    }
                }
            }
            if algorithm == CardinalityEncoding::RustsatTotalizer {
                assert_eq!(cache.totalizers.len(), 1);
            }
            assert_eq!(cache.occurrences.len(), 1);
        }
    }

    #[test]
    fn count_relations_preserve_both_truth_values_for_every_native_provider() {
        use crate::ast::sat_decision::{AmoEncoding, IntegerRelation};
        for size in 0..=4 {
            let vars: Vec<_> = (0..=size)
                .map(|i| {
                    DeclarationPtr::new_find(
                        Name::User(format!("count_{i}").into()),
                        Domain::bool(),
                    )
                })
                .collect();
            let expressions: Vec<Expression> = vars
                .iter()
                .cloned()
                .map(Reference::new)
                .map(Into::into)
                .collect();
            for special in [false, true] {
                let mut inputs = expressions[..size].to_vec();
                if special {
                    inputs.extend([true.into(), false.into()]);
                    if size > 0 {
                        inputs.extend([
                            expressions[0].clone(),
                            Expression::Not(Metadata::new(), Moo::new(expressions[0].clone())),
                        ]);
                    }
                }
                let mut bounds: Vec<_> = (-1..=inputs.len() as i64 + 1).collect();
                bounds.extend([i64::MIN, i64::MAX]);
                for algorithm in CardinalityEncoding::ALL {
                    for amo in AmoEncoding::ALL {
                        for relation in [
                            IntegerRelation::Equal,
                            IntegerRelation::NotEqual,
                            IntegerRelation::Less,
                            IntegerRelation::LessEqual,
                            IntegerRelation::Greater,
                            IntegerRelation::GreaterEqual,
                        ] {
                            for &bound in &bounds {
                                for fixed in [None, Some(false), Some(true)] {
                                    let mut instance = SatInstance::new();
                                    let mut map = HashMap::new();
                                    for var in &vars {
                                        map.insert(var.name().clone(), instance.new_lit());
                                    }
                                    let output = fixed
                                        .map(Expression::from)
                                        .unwrap_or_else(|| expressions[size].clone());
                                    compile_decisions(
                                        &[SatEncodingDecision::CountRelation {
                                            output,
                                            inputs: inputs.clone(),
                                            relation,
                                            bound,
                                            encoding: Some(EncodingSelection {
                                                algorithm,
                                                provenance:
                                                    SelectionProvenance::ExplicitConfiguration,
                                            }),
                                            amo_encoding: Some(EncodingSelection {
                                                algorithm: amo,
                                                provenance:
                                                    SelectionProvenance::ExplicitConfiguration,
                                            }),
                                        }],
                                        &mut instance,
                                        &mut map,
                                    )
                                    .unwrap();
                                    let used = instance.var_manager_mut().n_used();
                                    assert_eq!(instance.new_lit().var().idx32(), used);
                                    let (cnf, _): (Cnf, BasicVarManager) = instance.into_cnf();
                                    let mut solver = CaDiCaL::default();
                                    solver.add_cnf(cnf).unwrap();
                                    for assignment in 0..(1usize << size) {
                                        let count = assignment.count_ones() as i64
                                            + if special {
                                                if size > 0 { 2 } else { 1 }
                                            } else {
                                                0
                                            };
                                        let expected = match relation {
                                            IntegerRelation::Equal => count == bound,
                                            IntegerRelation::NotEqual => count != bound,
                                            IntegerRelation::Less => count < bound,
                                            IntegerRelation::LessEqual => count <= bound,
                                            IntegerRelation::Greater => count > bound,
                                            IntegerRelation::GreaterEqual => count >= bound,
                                        };
                                        for out in [false, true] {
                                            if fixed.is_some_and(|fixed| fixed != out) {
                                                continue;
                                            }
                                            let mut assumptions: Vec<_> = vars[..size]
                                                .iter()
                                                .enumerate()
                                                .map(|(i, var)| {
                                                    let lit = map[&var.name()];
                                                    if assignment & (1 << i) != 0 {
                                                        lit
                                                    } else {
                                                        !lit
                                                    }
                                                })
                                                .collect();
                                            if fixed.is_none() {
                                                let lit = map[&vars[size].name()];
                                                assumptions.push(if out { lit } else { !lit });
                                            }
                                            assert_eq!(
                                                solver.solve_assumps(&assumptions).unwrap()
                                                    == SolverResult::Sat,
                                                out == expected,
                                                "{algorithm:?}/{amo:?} {relation:?} {bound}, size={size}, special={special}, assignment={assignment}, fixed={fixed:?}, output={out}"
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn cardinality_providers_preserve_all_bounds_assignments_and_multiplicities() {
        for algorithm in CardinalityEncoding::ALL {
            for size in 0..=7 {
                let vars: Vec<_> = (0..size)
                    .map(|i| {
                        DeclarationPtr::new_find(Name::User(format!("b{i}").into()), Domain::bool())
                    })
                    .collect();
                let original: Vec<Expression> = vars
                    .iter()
                    .map(|var| Reference::new(var.clone()).into())
                    .collect();
                for special in [false, true] {
                    let mut inputs = original.clone();
                    if special {
                        inputs.extend([true.into(), false.into()]);
                        if let Some(first) = original.first() {
                            inputs.push(first.clone());
                            inputs.push(Expression::Not(
                                crate::ast::Metadata::new(),
                                crate::ast::Moo::new(first.clone()),
                            ));
                        }
                    }
                    for relation in [
                        CardinalityRelation::AtMost,
                        CardinalityRelation::AtLeast,
                        CardinalityRelation::Exactly,
                    ] {
                        for bound in -1..=inputs.len() as i64 + 1 {
                            let mut instance = SatInstance::new();
                            let mut map = HashMap::new();
                            for var in &vars {
                                map.insert(var.name().clone(), instance.new_lit());
                            }
                            compile_decisions(
                                &[SatEncodingDecision::Cardinality {
                                    inputs: inputs.clone(),
                                    relation,
                                    bound,
                                    encoding: Some(EncodingSelection {
                                        algorithm,
                                        provenance: SelectionProvenance::ExplicitConfiguration,
                                    }),
                                }],
                                &mut instance,
                                &mut map,
                            )
                            .unwrap();
                            // Later named allocations must remain beyond either provider's auxiliaries.
                            let used = instance.var_manager_mut().n_used();
                            assert_eq!(instance.new_lit().var().idx32(), used);
                            let (cnf, _): (Cnf, BasicVarManager) = instance.into_cnf();
                            let mut solver = CaDiCaL::default();
                            solver.add_cnf(cnf).unwrap();
                            for assignment in 0..(1usize << size) {
                                let count = assignment.count_ones() as i64
                                    + if special {
                                        if size > 0 { 2 } else { 1 }
                                    } else {
                                        0
                                    };
                                let expected = match relation {
                                    CardinalityRelation::AtMost => count <= bound,
                                    CardinalityRelation::AtLeast => count >= bound,
                                    CardinalityRelation::Exactly => count == bound,
                                };
                                let assumptions: Vec<_> = vars
                                    .iter()
                                    .enumerate()
                                    .map(|(i, var)| {
                                        let lit = map[&var.name()];
                                        if assignment & (1 << i) != 0 {
                                            lit
                                        } else {
                                            !lit
                                        }
                                    })
                                    .collect();
                                assert_eq!(
                                    solver.solve_assumps(&assumptions).unwrap()
                                        == SolverResult::Sat,
                                    expected,
                                    "{algorithm} {relation:?} n={size} special={special} bound={bound} bits={assignment}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
#[cfg(test)]
mod pseudo_boolean_tests {
    use super::*;
    use crate::ast::sat_decision::{
        CardinalityRelation, EncodingSelection, PbEncoding, PbTermGroup, PbTermStructure,
        SelectionProvenance,
    };
    use crate::ast::{DeclarationPtr, Domain, Metadata, Moo, Reference};
    use rustsat::{
        instances::{BasicVarManager, ManageVars},
        solvers::{Solve, SolveIncremental, SolverResult},
    };
    use rustsat_cadical::CaDiCaL;

    #[test]
    fn weighted_thresholds_reuse_batches_and_coexist_for_every_provider() {
        use crate::ast::sat_decision::IntegerRelation;
        for algorithm in PbEncoding::ALL {
            let mut cache = EncodingCache::default();
            let mut variables = HashMap::new();
            let mut solver = CaDiCaL::default();
            let mut initial: SatInstance = SatInstance::new();
            let inputs: Vec<_> = (0..3).map(|_| initial.new_lit()).collect();
            let (_, mut manager): (Cnf, BasicVarManager) = initial.into_cnf();
            let terms = vec![
                (3, Term::Literal(inputs[0])),
                (-2, Term::Literal(inputs[1])),
                (1, Term::Literal(inputs[0])),
                (2, Term::Literal(!inputs[2])),
                (1, Term::Constant(true)),
            ];
            let mut outputs = Vec::new();
            // Visit bounds in both directions, including repeated and trivial bounds.
            for bound in [3, 0, 4, 1, 3, -2, 7, 2] {
                let mut delta: SatInstance = SatInstance::new();
                delta
                    .var_manager_mut()
                    .increase_next_free(rustsat::types::Var::new(manager.n_used()));
                let output = delta.new_lit();
                let mut compiler = Compiler {
                    instance: &mut delta,
                    variables: &mut variables,
                    counters: Some(&mut cache),
                };
                compiler
                    .integer_relation(
                        algorithm,
                        Term::Literal(output),
                        IntegerRelation::LessEqual,
                        bound,
                        &terms,
                        &[],
                    )
                    .unwrap();
                let allocated = compiler.instance.var_manager_mut().n_used();
                let clauses = compiler.instance.cnf().len();
                compiler
                    .integer_relation(
                        algorithm,
                        Term::Literal(output),
                        IntegerRelation::LessEqual,
                        bound,
                        &terms,
                        &[],
                    )
                    .unwrap();
                assert_eq!(
                    compiler.instance.var_manager_mut().n_used(),
                    allocated,
                    "Repeated PB thresholds must reuse auxiliaries: {algorithm:?}"
                );
                assert!(compiler.instance.cnf().len() - clauses <= 2);
                let (cnf, next): (Cnf, BasicVarManager) = delta.into_cnf();
                manager = next;
                solver.add_cnf(cnf).unwrap();
                outputs.push((output, bound));
            }
            for assignment in 0..8 {
                let value = 4 * i64::from(assignment & 1 != 0) - 2 * i64::from(assignment & 2 != 0)
                    + 2 * i64::from(assignment & 4 == 0)
                    + 1;
                let mut assumptions: Vec<_> = inputs
                    .iter()
                    .enumerate()
                    .map(|(bit, &lit)| {
                        if assignment & (1 << bit) != 0 {
                            lit
                        } else {
                            !lit
                        }
                    })
                    .collect();
                assumptions.extend(
                    outputs
                        .iter()
                        .map(|&(lit, bound)| if value <= bound { lit } else { !lit }),
                );
                // Every old and new bound must coexist, including DPW thresholds.
                assert_eq!(
                    solver.solve_assumps(&assumptions).unwrap(),
                    SolverResult::Sat,
                    "Compatible bounds rejected: {algorithm:?} assignment={assignment}"
                );
                for index in inputs.len()..assumptions.len() {
                    assumptions[index] = !assumptions[index];
                    assert_eq!(
                        solver.solve_assumps(&assumptions).unwrap(),
                        SolverResult::Unsat,
                        "Reverse implication missing: {algorithm:?} assignment={assignment} output={index}"
                    );
                    assumptions[index] = !assumptions[index];
                }
            }
            if matches!(
                algorithm,
                PbEncoding::RustsatGeneralizedTotalizer | PbEncoding::RustsatBinaryAdder
            ) {
                assert_eq!(
                    cache.weighted.len(),
                    2,
                    "Upper and inverted lower counters must be shared"
                );
            }
        }
    }

    #[test]
    fn structured_threshold_cache_ignores_group_order_and_constant_spelling() {
        use crate::ast::sat_decision::IntegerRelation;
        for algorithm in [PbEncoding::PindakaasBdd, PbEncoding::PindakaasSwc] {
            let mut instance = SatInstance::new();
            let mut variables = HashMap::new();
            let mut cache = EncodingCache::default();
            let inputs: Vec<_> = (0..6).map(|_| instance.new_lit()).collect();
            let weights = [-2, 3, 2, -1, 4, 1];
            let original: Vec<_> = weights
                .iter()
                .zip(&inputs)
                .map(|(&weight, &lit)| (weight, Term::Literal(lit)))
                .collect();
            let groups = vec![
                PbTermGroup {
                    start: 0,
                    end: 3,
                    structure: PbTermStructure::Chain,
                },
                PbTermGroup {
                    start: 3,
                    end: 6,
                    structure: PbTermStructure::Choice,
                },
            ];
            let mut outputs = Vec::new();
            for bound in [0, 4, 1, 0] {
                let output = instance.new_lit();
                let mut compiler = Compiler {
                    instance: &mut instance,
                    variables: &mut variables,
                    counters: Some(&mut cache),
                };
                let mut terms = original.clone();
                terms.push((1, Term::Constant(true)));
                compiler
                    .integer_relation(
                        algorithm,
                        Term::Literal(output),
                        IntegerRelation::LessEqual,
                        bound,
                        &terms,
                        &groups,
                    )
                    .unwrap();
                let allocated = compiler.instance.var_manager_mut().n_used();
                // Move the choice before the chain, retaining chain implication order.
                let mut reordered = original[3..].to_vec();
                reordered.reverse();
                reordered.extend_from_slice(&original[..3]);
                reordered.push((2, Term::Constant(true)));
                let reordered_groups = vec![
                    PbTermGroup {
                        start: 3,
                        end: 6,
                        structure: PbTermStructure::Chain,
                    },
                    PbTermGroup {
                        start: 0,
                        end: 3,
                        structure: PbTermStructure::Choice,
                    },
                ];
                compiler
                    .integer_relation(
                        algorithm,
                        Term::Literal(output),
                        IntegerRelation::LessEqual,
                        bound + 1,
                        &reordered,
                        &reordered_groups,
                    )
                    .unwrap();
                let complementary: Vec<_> = terms
                    .iter()
                    .map(|(weight, term)| (-*weight, *term))
                    .collect();
                compiler
                    .integer_relation(
                        algorithm,
                        Term::Literal(!output),
                        IntegerRelation::LessEqual,
                        -bound - 1,
                        &complementary,
                        &groups,
                    )
                    .unwrap();
                assert_eq!(
                    compiler.instance.var_manager_mut().n_used(),
                    allocated,
                    "Equivalent structured thresholds must share auxiliaries: {algorithm:?}"
                );
                outputs.push((output, bound));
            }
            let mut solver = CaDiCaL::default();
            solver.add_cnf(instance.cnf().clone()).unwrap();
            for assignment in 0usize..64 {
                let set = |bit: usize| assignment & (1usize << bit) != 0;
                if (set(1) && !set(0)) || (set(2) && !set(1)) || (assignment >> 3).count_ones() > 1
                {
                    continue;
                }
                let value = 1 + weights
                    .iter()
                    .enumerate()
                    .map(|(bit, weight)| weight * i64::from(set(bit)))
                    .sum::<i64>();
                let mut assumptions: Vec<_> = inputs
                    .iter()
                    .enumerate()
                    .map(|(bit, &lit)| if set(bit) { lit } else { !lit })
                    .collect();
                assumptions.extend(
                    outputs
                        .iter()
                        .map(|&(lit, bound)| if value <= bound { lit } else { !lit }),
                );
                assert_eq!(
                    solver.solve_assumps(&assumptions).unwrap(),
                    SolverResult::Sat
                );
                for index in inputs.len()..assumptions.len() {
                    assumptions[index] = !assumptions[index];
                    assert_eq!(
                        solver.solve_assumps(&assumptions).unwrap(),
                        SolverResult::Unsat
                    );
                    assumptions[index] = !assumptions[index];
                }
            }
        }
    }

    #[test]
    fn large_gte_chain_predicates_preserve_projection_and_threshold_reuse() {
        use crate::ast::sat_decision::IntegerRelation;
        let mut instance = SatInstance::new();
        let mut variables = HashMap::new();
        let mut cache = EncodingCache::default();
        let inputs: Vec<_> = (0..96).map(|_| instance.new_lit()).collect();
        for pair in inputs.windows(2) {
            instance.add_clause(atomics::lit_impl_lit(pair[1], pair[0]));
        }
        let output = instance.new_lit();
        let terms: Vec<_> = inputs.iter().map(|&lit| (1, Term::Literal(lit))).collect();
        let groups = vec![PbTermGroup {
            start: 0,
            end: inputs.len(),
            structure: PbTermStructure::Chain,
        }];
        let mut compiler = Compiler {
            instance: &mut instance,
            variables: &mut variables,
            counters: Some(&mut cache),
        };
        compiler
            .integer_relation(
                PbEncoding::RustsatGeneralizedTotalizer,
                Term::Literal(output),
                IntegerRelation::LessEqual,
                48,
                &terms,
                &groups,
            )
            .unwrap();
        let allocated = compiler.instance.var_manager_mut().n_used();
        compiler
            .integer_relation(
                PbEncoding::RustsatGeneralizedTotalizer,
                Term::Literal(output),
                IntegerRelation::LessEqual,
                48,
                &terms,
                &groups,
            )
            .unwrap();
        assert_eq!(compiler.instance.var_manager_mut().n_used(), allocated);
        assert!(cache.weighted.is_empty());
        assert_eq!(cache.weighted_thresholds.len(), 1);
        let mut solver = CaDiCaL::default();
        solver.add_cnf(instance.cnf().clone()).unwrap();
        for count in [0, 47, 48, 49, 96] {
            for truth in [false, true] {
                let mut assumptions: Vec<_> = inputs
                    .iter()
                    .enumerate()
                    .map(|(bit, &lit)| if bit < count { lit } else { !lit })
                    .collect();
                assumptions.push(if truth { output } else { !output });
                assert_eq!(
                    solver.solve_assumps(&assumptions).unwrap() == SolverResult::Sat,
                    truth == (count <= 48)
                );
            }
        }
    }

    #[test]
    fn wide_asserted_equalities_preserve_native_bound_both_projection() {
        for algorithm in [
            PbEncoding::RustsatGeneralizedTotalizer,
            PbEncoding::RustsatBinaryAdder,
        ] {
            let mut instance = SatInstance::new();
            let mut variables = HashMap::new();
            let mut cache = EncodingCache::default();
            let inputs: Vec<_> = (0..96).map(|_| instance.new_lit()).collect();
            for pair in inputs.windows(2) {
                instance.add_clause(atomics::lit_impl_lit(pair[1], pair[0]));
            }
            let mut compiler = Compiler {
                instance: &mut instance,
                variables: &mut variables,
                counters: Some(&mut cache),
            };
            compiler
                .pseudo_boolean(
                    algorithm,
                    CardinalityRelation::Exactly,
                    48,
                    inputs.iter().map(|&lit| (1, Term::Literal(lit))).collect(),
                    &[],
                )
                .unwrap();
            assert!(cache.weighted.is_empty());
            let mut solver = CaDiCaL::default();
            solver.add_cnf(instance.cnf().clone()).unwrap();
            for count in [0, 47, 48, 49, 96] {
                let assumptions: Vec<_> = inputs
                    .iter()
                    .enumerate()
                    .map(|(bit, &lit)| if bit < count { lit } else { !lit })
                    .collect();
                assert_eq!(
                    solver.solve_assumps(&assumptions).unwrap() == SolverResult::Sat,
                    count == 48
                );
            }
        }
    }

    #[test]
    fn table_mdd_shares_suffixes_and_rejects_malformed_rows() {
        use crate::ast::sat_decision::{SatIntegerView, TableEncoding};
        let variables: Vec<_> = (0..4)
            .map(|i| {
                DeclarationPtr::new_find(Name::User(format!("mdd_{i}").into()), Domain::bool())
            })
            .collect();
        let bits: Vec<Expression> = variables
            .iter()
            .cloned()
            .map(Reference::new)
            .map(Into::into)
            .collect();
        let inputs: Vec<_> = bits[..3]
            .iter()
            .map(|bit| SatIntegerView {
                constant: 0,
                terms: vec![(1, bit.clone())],
                groups: vec![],
                choices: None,
            })
            .collect();
        let build = |strategy, rows| SatEncodingDecision::Table {
            output: bits[3].clone(),
            inputs: inputs.clone(),
            rows,
            negative: false,
            encoding: Some(EncodingSelection {
                algorithm: strategy,
                provenance: SelectionProvenance::ExplicitConfiguration,
            }),
            pb_encoding: Some(EncodingSelection {
                algorithm: PbEncoding::RustsatGeneralizedTotalizer,
                provenance: SelectionProvenance::ExplicitConfiguration,
            }),
        };
        let rows: Vec<_> = (0..8)
            .map(|n| (0..3).map(|i| (n >> i) & 1).collect())
            .collect();
        let mut counts = Vec::new();
        for strategy in TableEncoding::ALL {
            let mut instance = SatInstance::new();
            let mut map = HashMap::new();
            for v in &variables {
                map.insert(v.name().clone(), instance.new_lit());
            }
            compile_decisions(&[build(strategy, rows.clone())], &mut instance, &mut map).unwrap();
            counts.push(instance.var_manager_mut().n_used());
        }
        assert!(
            counts[1] < counts[0],
            "Shared suffixes should save auxiliaries: {counts:?}"
        );
        let mut instance = SatInstance::new();
        let mut map = HashMap::new();
        for v in &variables {
            map.insert(v.name().clone(), instance.new_lit());
        }
        assert!(
            compile_decisions(
                &[build(TableEncoding::Mdd, vec![vec![0]])],
                &mut instance,
                &mut map
            )
            .is_err()
        );
    }

    #[test]
    fn tables_preserve_numeric_views_and_both_truth_values() {
        use crate::ast::sat_decision::{SatIntegerView, TableEncoding};
        let variables: Vec<_> = (0..5)
            .map(|i| {
                DeclarationPtr::new_find(Name::User(format!("table_{i}").into()), Domain::bool())
            })
            .collect();
        let bits: Vec<Expression> = variables
            .iter()
            .cloned()
            .map(Reference::new)
            .map(Into::into)
            .collect();
        let not_a = Expression::Not(Metadata::new(), Moo::new(bits[0].clone()));
        let choice = SatIntegerView {
            constant: 0,
            terms: vec![(-2, not_a.clone()), (3, bits[0].clone())],
            groups: vec![PbTermGroup {
                start: 0,
                end: 2,
                structure: PbTermStructure::Choice,
            }],
            choices: Some(vec![(-2, not_a), (3, bits[0].clone())]),
        };
        let views = vec![
            choice.clone(),
            SatIntegerView {
                constant: -1,
                terms: vec![(2, bits[1].clone()), (3, bits[2].clone())],
                groups: vec![PbTermGroup {
                    start: 0,
                    end: 2,
                    structure: PbTermStructure::Chain,
                }],
                choices: None,
            },
            SatIntegerView {
                constant: 1,
                terms: vec![(2, bits[3].clone())],
                groups: vec![PbTermGroup {
                    start: 0,
                    end: 1,
                    structure: PbTermStructure::BoundedBinary { lower: 0, upper: 2 },
                }],
                choices: None,
            },
            choice,
            SatIntegerView {
                constant: 0,
                terms: vec![],
                groups: vec![],
                choices: Some(vec![(0, true.into())]),
            },
        ];
        let relation = vec![
            vec![-2, -1, 1, -2, 0],
            vec![3, 4, 3, 3, 0],
            vec![3, 4, 3, 3, 0],
            vec![42, 0, 1, 42, 0],
        ];
        for strategy in TableEncoding::ALL {
            for pb in PbEncoding::ALL {
                for negative in [false, true] {
                    for assertion in [None, Some(false), Some(true)] {
                        for (inputs, rows) in [
                            (views.clone(), relation.clone()),
                            (views.clone(), vec![]),
                            (vec![], vec![]),
                            (vec![], vec![vec![]]),
                        ] {
                            let mut instance = SatInstance::new();
                            let mut map = HashMap::new();
                            for variable in &variables {
                                map.insert(variable.name().clone(), instance.new_lit());
                            }
                            let mut decisions = vec![
                                SatEncodingDecision::Assert(Expression::Imply(
                                    Metadata::new(),
                                    Moo::new(bits[2].clone()),
                                    Moo::new(bits[1].clone()),
                                )),
                                SatEncodingDecision::Table {
                                    output: bits[4].clone(),
                                    inputs: inputs.clone(),
                                    rows: rows.clone(),
                                    negative,
                                    encoding: Some(EncodingSelection {
                                        algorithm: strategy,
                                        provenance: SelectionProvenance::ExplicitConfiguration,
                                    }),
                                    pb_encoding: Some(EncodingSelection {
                                        algorithm: pb,
                                        provenance: SelectionProvenance::ExplicitConfiguration,
                                    }),
                                },
                            ];
                            if let Some(value) = assertion {
                                decisions.push(SatEncodingDecision::Assert(if value {
                                    bits[4].clone()
                                } else {
                                    Expression::Not(Metadata::new(), Moo::new(bits[4].clone()))
                                }));
                            }
                            assert_eq!(
                                serde_json::from_str::<SatEncodingDecision>(
                                    &serde_json::to_string(&decisions[1]).unwrap()
                                )
                                .unwrap(),
                                decisions[1]
                            );
                            compile_decisions(&decisions, &mut instance, &mut map).unwrap();
                            let used = instance.var_manager_mut().n_used();
                            assert_eq!(instance.new_lit().var().idx32(), used);
                            let (cnf, _): (Cnf, BasicVarManager) = instance.into_cnf();
                            let mut solver = CaDiCaL::default();
                            solver.add_cnf(cnf).unwrap();
                            for assignment in 0usize..32 {
                                let set = |i| assignment & (1usize << i) != 0;
                                let numeric = if inputs.is_empty() {
                                    vec![]
                                } else {
                                    vec![
                                        if set(0) { 3 } else { -2 },
                                        -1 + 2 * i64::from(set(1)) + 3 * i64::from(set(2)),
                                        1 + 2 * i64::from(set(3)),
                                        if set(0) { 3 } else { -2 },
                                        0,
                                    ]
                                };
                                let truth = rows.contains(&numeric) != negative;
                                let valid = (!set(2) || set(1))
                                    && set(4) == truth
                                    && assertion.is_none_or(|value| value == set(4));
                                let assumptions: Vec<_> = variables
                                    .iter()
                                    .enumerate()
                                    .map(|(i, v)| {
                                        if set(i) {
                                            map[&v.name()]
                                        } else {
                                            !map[&v.name()]
                                        }
                                    })
                                    .collect();
                                assert_eq!(
                                    solver.solve_assumps(&assumptions).unwrap()
                                        == SolverResult::Sat,
                                    valid,
                                    "{strategy} {pb} negative={negative} asserted={assertion:?} bits={assignment} rows={rows:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn alldifferent_preserves_sparse_values_repetition_and_both_truth_values() {
        use crate::ast::sat_decision::{AllDifferentEncoding, AmoEncoding, SatIntegerView};
        let variables: Vec<_> = (0..10)
            .map(|index| {
                DeclarationPtr::new_find(Name::User(format!("ad{index}").into()), Domain::bool())
            })
            .collect();
        let expressions: Vec<Expression> = variables
            .iter()
            .map(|variable| Reference::new(variable.clone()).into())
            .collect();
        let values = [[-2, 0, 3], [-1, 0, 3], [-2, 0, 2]];
        let views: Vec<_> = values
            .iter()
            .enumerate()
            .map(|(row, values)| {
                let terms: Vec<_> = values
                    .iter()
                    .zip(&expressions[row * 3..row * 3 + 3])
                    .map(|(value, expression)| (*value, expression.clone()))
                    .collect();
                SatIntegerView {
                    constant: 0,
                    choices: Some(terms.clone()),
                    terms,
                    groups: vec![PbTermGroup {
                        start: 0,
                        end: 3,
                        structure: PbTermStructure::Choice,
                    }],
                }
            })
            .collect();
        for strategy in AllDifferentEncoding::ALL {
            for pb in PbEncoding::ALL {
                for amo in AmoEncoding::ALL {
                    for (kind, except) in (0..5).flat_map(|kind| {
                        [None, Some(-2), Some(0), Some(3), Some(42)]
                            .into_iter()
                            .map(move |except| (kind, except))
                    }) {
                        let mut inputs = views.clone();
                        match kind {
                            1 => inputs[2] = inputs[0].clone(),
                            2 => {
                                inputs[2] = SatIntegerView {
                                    constant: 0,
                                    terms: vec![],
                                    groups: vec![],
                                    choices: Some(vec![(0, true.into())]),
                                }
                            }
                            3 => inputs.clear(),
                            4 => inputs.truncate(1),
                            _ => (),
                        }
                        for assertion in [None, Some(false), Some(true)] {
                            let mut instance = SatInstance::new();
                            let mut map = HashMap::new();
                            for variable in &variables {
                                map.insert(variable.name().clone(), instance.new_lit());
                            }
                            for row in 0..3 {
                                let literals: Vec<_> = variables[row * 3..row * 3 + 3]
                                    .iter()
                                    .map(|variable| map[&variable.name()])
                                    .collect();
                                instance.add_clause(literals.iter().copied().collect());
                                for a in 0..3 {
                                    for b in a + 1..3 {
                                        instance.add_clause(
                                            [!literals[a], !literals[b]].into_iter().collect(),
                                        );
                                    }
                                }
                            }
                            let mut decisions = vec![SatEncodingDecision::AllDifferent {
                                output: expressions[9].clone(),
                                inputs: inputs.clone(),
                                except,
                                encoding: Some(EncodingSelection {
                                    algorithm: strategy,
                                    provenance: SelectionProvenance::ExplicitConfiguration,
                                }),
                                amo_encoding: Some(EncodingSelection {
                                    algorithm: amo,
                                    provenance: SelectionProvenance::ExplicitConfiguration,
                                }),
                                pb_encoding: Some(EncodingSelection {
                                    algorithm: pb,
                                    provenance: SelectionProvenance::ExplicitConfiguration,
                                }),
                            }];
                            if let Some(value) = assertion {
                                decisions.push(SatEncodingDecision::Assert(if value {
                                    expressions[9].clone()
                                } else {
                                    Expression::Not(
                                        Metadata::new(),
                                        Moo::new(expressions[9].clone()),
                                    )
                                }));
                            }
                            assert_eq!(
                                serde_json::from_str::<SatEncodingDecision>(
                                    &serde_json::to_string(&decisions[0]).unwrap()
                                )
                                .unwrap(),
                                decisions[0]
                            );
                            compile_decisions(&decisions, &mut instance, &mut map).unwrap();
                            let (cnf, _): (Cnf, BasicVarManager) = instance.into_cnf();
                            let mut solver = CaDiCaL::default();
                            solver.add_cnf(cnf).unwrap();
                            for assignment in 0usize..512 {
                                let set = |index| assignment & (1usize << index) != 0;
                                let valid = (0..3)
                                    .all(|row| ((assignment >> (row * 3)) & 7).count_ones() == 1);
                                let mut numeric = [0; 3];
                                for row in 0..3 {
                                    for column in 0..3 {
                                        numeric[row] +=
                                            values[row][column] * i64::from(set(row * 3 + column));
                                    }
                                }
                                if kind == 1 {
                                    numeric[2] = numeric[0];
                                }
                                if kind == 2 {
                                    numeric[2] = 0;
                                }
                                let truth = (0..inputs.len()).all(|left| {
                                    (left + 1..inputs.len()).all(|right| {
                                        numeric[left] != numeric[right]
                                            || Some(numeric[left]) == except
                                    })
                                });
                                for output in [false, true] {
                                    let assumptions: Vec<_> = variables
                                        .iter()
                                        .enumerate()
                                        .map(|(index, variable)| {
                                            let literal = map[&variable.name()];
                                            if if index == 9 { output } else { set(index) } {
                                                literal
                                            } else {
                                                !literal
                                            }
                                        })
                                        .collect();
                                    let expected = valid
                                        && output == truth
                                        && assertion.is_none_or(|value| value == output);
                                    assert_eq!(
                                        solver.solve_assumps(&assumptions).unwrap()
                                            == SolverResult::Sat,
                                        expected,
                                        "{strategy} {pb} {amo} kind={kind} except={except:?} assertion={assertion:?} bits={assignment} output={output}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn integer_relations_preserve_both_truth_values_for_every_provider() {
        use crate::ast::sat_decision::IntegerRelation;
        let variables: Vec<_> = (0..4)
            .map(|index| {
                DeclarationPtr::new_find(Name::User(format!("r{index}").into()), Domain::bool())
            })
            .collect();
        let inputs: Vec<Expression> = variables
            .iter()
            .map(|variable| Reference::new(variable.clone()).into())
            .collect();
        let terms = vec![
            (2, inputs[0].clone()),
            (-3, inputs[1].clone()),
            (5, inputs[2].clone()),
            (1, inputs[0].clone()),
            (
                -2,
                Expression::Not(Metadata::new(), Moo::new(inputs[0].clone())),
            ),
            (4, true.into()),
            (9, false.into()),
        ];
        for algorithm in PbEncoding::ALL {
            for relation in [
                IntegerRelation::Equal,
                IntegerRelation::NotEqual,
                IntegerRelation::Less,
                IntegerRelation::LessEqual,
                IntegerRelation::Greater,
                IntegerRelation::GreaterEqual,
            ] {
                for bound in [i64::MIN, -10, -3, 0, 1, 2, 4, 8, 10, i64::MAX] {
                    for output in [
                        inputs[3].clone(),
                        inputs[0].clone(),
                        true.into(),
                        false.into(),
                    ] {
                        let mut instance = SatInstance::new();
                        let mut map = HashMap::new();
                        for variable in &variables {
                            map.insert(variable.name().clone(), instance.new_lit());
                        }
                        let decision = SatEncodingDecision::IntegerRelation {
                            output: output.clone(),
                            terms: terms.clone(),
                            groups: vec![],
                            relation,
                            bound,
                            encoding: Some(EncodingSelection {
                                algorithm,
                                provenance: SelectionProvenance::ExplicitConfiguration,
                            }),
                        };
                        assert_eq!(
                            serde_json::from_str::<SatEncodingDecision>(
                                &serde_json::to_string(&decision).unwrap()
                            )
                            .unwrap(),
                            decision
                        );
                        compile_decisions(&[decision], &mut instance, &mut map).unwrap();
                        let used = instance.var_manager_mut().n_used();
                        assert_eq!(instance.new_lit().var().idx32(), used);
                        let (cnf, _): (Cnf, BasicVarManager) = instance.into_cnf();
                        let mut solver = CaDiCaL::default();
                        solver.add_cnf(cnf).unwrap();
                        for assignment in 0usize..16 {
                            let bit = |index| i64::from(assignment & (1usize << index) != 0);
                            let value = 5 * bit(0) - 3 * bit(1) + 5 * bit(2) + 2;
                            let truth = match relation {
                                IntegerRelation::Equal => value == bound,
                                IntegerRelation::NotEqual => value != bound,
                                IntegerRelation::Less => value < bound,
                                IntegerRelation::LessEqual => value <= bound,
                                IntegerRelation::Greater => value > bound,
                                IntegerRelation::GreaterEqual => value >= bound,
                            };
                            let actual_output = if output == inputs[3] {
                                bit(3) != 0
                            } else if output == inputs[0] {
                                bit(0) != 0
                            } else {
                                output == Expression::from(true)
                            };
                            let assumptions: Vec<_> = variables
                                .iter()
                                .enumerate()
                                .map(|(index, variable)| {
                                    let lit = map[&variable.name()];
                                    if bit(index) != 0 { lit } else { !lit }
                                })
                                .collect();
                            assert_eq!(
                                solver.solve_assumps(&assumptions).unwrap() == SolverResult::Sat,
                                truth == actual_output,
                                "{algorithm} {relation:?} bound={bound} bits={assignment} output={output}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn choice_relations_preserve_truth_for_sparse_and_signed_values() {
        use crate::ast::sat_decision::{IntegerRelation, PbTermGroup, PbTermStructure};
        let variables: Vec<_> = (0..5)
            .map(|index| {
                DeclarationPtr::new_find(Name::User(format!("c{index}").into()), Domain::bool())
            })
            .collect();
        let inputs: Vec<Expression> = variables
            .iter()
            .map(|variable| Reference::new(variable.clone()).into())
            .collect();
        let weights = [-3, 2, 3, -6];
        for algorithm in PbEncoding::ALL {
            for relation in [
                IntegerRelation::Equal,
                IntegerRelation::NotEqual,
                IntegerRelation::Less,
                IntegerRelation::LessEqual,
                IntegerRelation::Greater,
                IntegerRelation::GreaterEqual,
            ] {
                for count in [1, 2] {
                    for bound in [-5, 0, 3, 6] {
                        let mut instance = SatInstance::new();
                        let mut map = HashMap::new();
                        for variable in &variables {
                            map.insert(variable.name().clone(), instance.new_lit());
                        }
                        compile_decisions(
                            &[SatEncodingDecision::IntegerRelation {
                                output: inputs[4].clone(),
                                terms: weights[..count * 2]
                                    .iter()
                                    .zip(&inputs)
                                    .map(|(weight, input)| (*weight, input.clone()))
                                    .collect(),
                                groups: (0..count)
                                    .map(|index| PbTermGroup {
                                        start: index * 2,
                                        end: index * 2 + 2,
                                        structure: PbTermStructure::Choice,
                                    })
                                    .collect(),
                                relation,
                                bound,
                                encoding: Some(EncodingSelection {
                                    algorithm,
                                    provenance: SelectionProvenance::ExplicitConfiguration,
                                }),
                            }],
                            &mut instance,
                            &mut map,
                        )
                        .unwrap();
                        let (cnf, _): (Cnf, BasicVarManager) = instance.into_cnf();
                        let mut solver = CaDiCaL::default();
                        solver.add_cnf(cnf).unwrap();
                        for assignment in 0usize..32 {
                            if (0..count)
                                .any(|index| ((assignment >> (index * 2)) & 3).count_ones() != 1)
                            {
                                continue;
                            }
                            let value: i64 = weights[..count * 2]
                                .iter()
                                .enumerate()
                                .filter(|(index, _)| assignment & (1 << index) != 0)
                                .map(|(_, weight)| *weight)
                                .sum();
                            let truth = match relation {
                                IntegerRelation::Equal => value == bound,
                                IntegerRelation::NotEqual => value != bound,
                                IntegerRelation::Less => value < bound,
                                IntegerRelation::LessEqual => value <= bound,
                                IntegerRelation::Greater => value > bound,
                                IntegerRelation::GreaterEqual => value >= bound,
                            };
                            let assumptions: Vec<_> = variables
                                .iter()
                                .enumerate()
                                .map(|(index, variable)| {
                                    let literal = map[&variable.name()];
                                    if assignment & (1 << index) != 0 {
                                        literal
                                    } else {
                                        !literal
                                    }
                                })
                                .collect();
                            assert_eq!(
                                solver.solve_assumps(&assumptions).unwrap() == SolverResult::Sat,
                                truth == (assignment & 16 != 0),
                                "{algorithm} {relation:?} groups={count} bound={bound} bits={assignment}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn asserted_relations_allocate_outputs_and_preserve_truth_in_both_orders() {
        use crate::ast::sat_decision::IntegerRelation;
        let variables: Vec<_> = (0..3)
            .map(|index| {
                DeclarationPtr::new_find(Name::User(format!("a{index}").into()), Domain::bool())
            })
            .collect();
        let inputs: Vec<Expression> = variables
            .iter()
            .map(|variable| Reference::new(variable.clone()).into())
            .collect();
        for algorithm in PbEncoding::ALL {
            for relation in [
                IntegerRelation::Equal,
                IntegerRelation::NotEqual,
                IntegerRelation::Less,
                IntegerRelation::LessEqual,
                IntegerRelation::Greater,
                IntegerRelation::GreaterEqual,
            ] {
                for bound in [-3, -1, 0, 2] {
                    for truth in [false, true] {
                        for assertion_first in [false, true] {
                            let mut instance = SatInstance::new();
                            let mut map = HashMap::new();
                            for variable in &variables[..2] {
                                map.insert(variable.name().clone(), instance.new_lit());
                            }
                            let assertion = SatEncodingDecision::Assert(if truth {
                                inputs[2].clone()
                            } else {
                                Expression::Not(Metadata::new(), Moo::new(inputs[2].clone()))
                            });
                            let decision = SatEncodingDecision::IntegerRelation {
                                output: inputs[2].clone(),
                                terms: vec![(2, inputs[0].clone()), (-3, inputs[1].clone())],
                                groups: vec![],
                                relation,
                                bound,
                                encoding: Some(EncodingSelection {
                                    algorithm,
                                    provenance: SelectionProvenance::ExplicitConfiguration,
                                }),
                            };
                            let decisions = if assertion_first {
                                vec![assertion, decision]
                            } else {
                                vec![decision, assertion]
                            };
                            compile_decisions(&decisions, &mut instance, &mut map).unwrap();
                            let (cnf, _): (Cnf, BasicVarManager) = instance.into_cnf();
                            let mut solver = CaDiCaL::default();
                            solver.add_cnf(cnf).unwrap();
                            for assignment in 0usize..4 {
                                let bit = |index| assignment & (1usize << index) != 0;
                                let value = 2 * i64::from(bit(0)) - 3 * i64::from(bit(1));
                                let expected = match relation {
                                    IntegerRelation::Equal => value == bound,
                                    IntegerRelation::NotEqual => value != bound,
                                    IntegerRelation::Less => value < bound,
                                    IntegerRelation::LessEqual => value <= bound,
                                    IntegerRelation::Greater => value > bound,
                                    IntegerRelation::GreaterEqual => value >= bound,
                                };
                                let assumptions: Vec<_> = variables[..2]
                                    .iter()
                                    .enumerate()
                                    .map(|(index, variable)| {
                                        let lit = map[&variable.name()];
                                        if bit(index) { lit } else { !lit }
                                    })
                                    .collect();
                                assert_eq!(
                                    solver.solve_assumps(&assumptions).unwrap()
                                        == SolverResult::Sat,
                                    expected == truth,
                                    "{algorithm} {relation:?} bound={bound} truth={truth} bits={assignment} first={assertion_first}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn structured_providers_preserve_representation_domains_and_signed_occurrences() {
        let variables: Vec<_> = (0..4)
            .map(|index| {
                DeclarationPtr::new_find(Name::User(format!("g{index}").into()), Domain::bool())
            })
            .collect();
        let inputs: Vec<Expression> = variables
            .iter()
            .map(|variable| Reference::new(variable.clone()).into())
            .collect();
        for algorithm in PbEncoding::ALL {
            for kind in 0..5 {
                for scale in [-3, 1, 2] {
                    let weights = match kind {
                        0 => [2, 5, 9],
                        1 => [2, 2, 2],
                        2 => [1, 2, -4],
                        4 => [-3, -1, 2],
                        _ => [1, 2, 4],
                    };
                    let (low, high) = if kind == 2 { (-3, 2) } else { (0, 5) };
                    let structure = match kind {
                        0 | 4 => PbTermStructure::Choice,
                        1 => PbTermStructure::Chain,
                        _ => PbTermStructure::BoundedBinary {
                            lower: (scale * low).min(scale * high),
                            upper: (scale * low).max(scale * high),
                        },
                    };
                    for repeated in [false, true] {
                        let mut terms: Vec<_> = weights
                            .iter()
                            .zip(&inputs)
                            .map(|(weight, input)| (weight * scale, input.clone()))
                            .collect();
                        terms.push((5, inputs[3].clone()));
                        if repeated {
                            terms.extend([
                                (2, inputs[0].clone()),
                                (
                                    -3,
                                    Expression::Not(Metadata::new(), Moo::new(inputs[1].clone())),
                                ),
                                (4, true.into()),
                            ]);
                        }
                        for relation in [
                            CardinalityRelation::AtMost,
                            CardinalityRelation::AtLeast,
                            CardinalityRelation::Exactly,
                        ] {
                            for bound in -32..=32 {
                                let mut instance = SatInstance::new();
                                let mut map = HashMap::new();
                                for variable in &variables {
                                    map.insert(variable.name().clone(), instance.new_lit());
                                }
                                // Representation invariants remain independently enforced.
                                for assignment in 0usize..8 {
                                    let set = |index: usize| assignment & (1usize << index) != 0;
                                    let value: i64 = weights
                                        .iter()
                                        .enumerate()
                                        .map(|(index, weight)| weight * i64::from(set(index)))
                                        .sum();
                                    let valid = match kind {
                                        0 | 4 => assignment.count_ones() <= 1,
                                        1 => (!set(1) || set(0)) && (!set(2) || set(1)),
                                        _ => (low..=high).contains(&value),
                                    };
                                    if !valid {
                                        instance.add_clause(
                                            variables[..3]
                                                .iter()
                                                .enumerate()
                                                .map(|(index, variable)| {
                                                    let literal = map[&variable.name()];
                                                    if set(index) { !literal } else { literal }
                                                })
                                                .collect(),
                                        );
                                    }
                                }
                                compile_decisions(
                                    &[SatEncodingDecision::PseudoBoolean {
                                        terms: terms.clone(),
                                        groups: vec![PbTermGroup {
                                            start: 0,
                                            end: 3,
                                            structure: structure.clone(),
                                        }],
                                        relation,
                                        bound,
                                        encoding: Some(EncodingSelection {
                                            algorithm,
                                            provenance: SelectionProvenance::ExplicitConfiguration,
                                        }),
                                    }],
                                    &mut instance,
                                    &mut map,
                                )
                                .unwrap();
                                let used = instance.var_manager_mut().n_used();
                                assert_eq!(instance.new_lit().var().idx32(), used);
                                let (cnf, _): (Cnf, BasicVarManager) = instance.into_cnf();
                                let mut solver = CaDiCaL::default();
                                solver.add_cnf(cnf).unwrap();
                                for assignment in 0usize..16 {
                                    let set = |index: usize| assignment & (1usize << index) != 0;
                                    let base: i64 = weights
                                        .iter()
                                        .enumerate()
                                        .map(|(index, weight)| weight * i64::from(set(index)))
                                        .sum();
                                    let valid = match kind {
                                        0 | 4 => (assignment & 7).count_ones() <= 1,
                                        1 => (!set(1) || set(0)) && (!set(2) || set(1)),
                                        _ => (low..=high).contains(&base),
                                    };
                                    let value = base * scale
                                        + 5 * i64::from(set(3))
                                        + if repeated {
                                            2 * i64::from(set(0)) - 3 * i64::from(!set(1)) + 4
                                        } else {
                                            0
                                        };
                                    let expected = valid
                                        && match relation {
                                            CardinalityRelation::AtMost => value <= bound,
                                            CardinalityRelation::AtLeast => value >= bound,
                                            CardinalityRelation::Exactly => value == bound,
                                        };
                                    let assumptions: Vec<_> = variables
                                        .iter()
                                        .enumerate()
                                        .map(|(index, variable)| {
                                            let literal = map[&variable.name()];
                                            if set(index) { literal } else { !literal }
                                        })
                                        .collect();
                                    assert_eq!(
                                        solver.solve_assumps(&assumptions).unwrap()
                                            == SolverResult::Sat,
                                        expected,
                                        "{algorithm} kind={kind} scale={scale} repeated={repeated} {relation:?} bound={bound} bits={assignment}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn structured_integer_relations_preserve_both_truth_values_and_assertions() {
        use crate::ast::sat_decision::IntegerRelation;
        let variables: Vec<_> = (0..5)
            .map(|index| {
                DeclarationPtr::new_find(Name::User(format!("g{index}").into()), Domain::bool())
            })
            .collect();
        let inputs: Vec<Expression> = variables
            .iter()
            .map(|variable| Reference::new(variable.clone()).into())
            .collect();
        for algorithm in [PbEncoding::PindakaasBdd, PbEncoding::PindakaasSwc] {
            for kind in 0..5 {
                for scale in [-3, 1, 2] {
                    let weights = match kind {
                        0 => [2, 5, 9],
                        1 => [2, 2, 2],
                        2 => [1, 2, -4],
                        4 => [-3, -1, 2],
                        _ => [1, 2, 4],
                    };
                    let (low, high) = if kind == 2 { (-3, 2) } else { (0, 5) };
                    let structure = match kind {
                        0 | 4 => PbTermStructure::Choice,
                        1 => PbTermStructure::Chain,
                        _ => PbTermStructure::BoundedBinary {
                            lower: (scale * low).min(scale * high),
                            upper: (scale * low).max(scale * high),
                        },
                    };
                    for repeated in [false, true] {
                        let mut terms: Vec<_> = weights
                            .iter()
                            .zip(&inputs)
                            .map(|(weight, input)| (weight * scale, input.clone()))
                            .collect();
                        terms.push((5, inputs[3].clone()));
                        if repeated {
                            terms.extend([
                                (2, inputs[0].clone()),
                                (
                                    -3,
                                    Expression::Not(Metadata::new(), Moo::new(inputs[1].clone())),
                                ),
                                (4, true.into()),
                            ]);
                        }
                        for relation in [
                            IntegerRelation::Equal,
                            IntegerRelation::NotEqual,
                            IntegerRelation::Less,
                            IntegerRelation::LessEqual,
                            IntegerRelation::Greater,
                            IntegerRelation::GreaterEqual,
                        ] {
                            for bound in [-32, -4, 0, 3, 8, 16, 32, i64::MIN, i64::MAX] {
                                for output_index in [3, 4] {
                                    for assertion in [None, Some(false), Some(true)] {
                                        let mut instance = SatInstance::new();
                                        let mut map = HashMap::new();
                                        for variable in &variables {
                                            map.insert(variable.name().clone(), instance.new_lit());
                                        }
                                        // Representation invariants remain independently enforced.
                                        for assignment in 0usize..8 {
                                            let set =
                                                |index: usize| assignment & (1usize << index) != 0;
                                            let value: i64 = weights
                                                .iter()
                                                .enumerate()
                                                .map(|(index, weight)| {
                                                    weight * i64::from(set(index))
                                                })
                                                .sum();
                                            let valid = match kind {
                                                0 | 4 => assignment.count_ones() <= 1,
                                                1 => (!set(1) || set(0)) && (!set(2) || set(1)),
                                                _ => (low..=high).contains(&value),
                                            };
                                            if !valid {
                                                instance.add_clause(
                                                    variables[..3]
                                                        .iter()
                                                        .enumerate()
                                                        .map(|(index, variable)| {
                                                            let literal = map[&variable.name()];
                                                            if set(index) {
                                                                !literal
                                                            } else {
                                                                literal
                                                            }
                                                        })
                                                        .collect(),
                                                );
                                            }
                                        }
                                        let mut decisions =
                                            vec![SatEncodingDecision::IntegerRelation {
                                                output: inputs[output_index].clone(),
                                                terms: terms.clone(),
                                                groups: vec![PbTermGroup {
                                                    start: 0,
                                                    end: 3,
                                                    structure: structure.clone(),
                                                }],
                                                relation,
                                                bound,
                                                encoding: Some(EncodingSelection {
                                                    algorithm,
                                                    provenance:
                                                        SelectionProvenance::ExplicitConfiguration,
                                                }),
                                            }];
                                        if let Some(value) = assertion {
                                            decisions.push(SatEncodingDecision::Assert(if value {
                                                inputs[output_index].clone()
                                            } else {
                                                Expression::Not(
                                                    Metadata::new(),
                                                    Moo::new(inputs[output_index].clone()),
                                                )
                                            }));
                                        }
                                        compile_decisions(&decisions, &mut instance, &mut map)
                                            .unwrap();
                                        let used = instance.var_manager_mut().n_used();
                                        assert_eq!(instance.new_lit().var().idx32(), used);
                                        let (cnf, _): (Cnf, BasicVarManager) = instance.into_cnf();
                                        let mut solver = CaDiCaL::default();
                                        solver.add_cnf(cnf).unwrap();
                                        for assignment in 0usize..32 {
                                            let set =
                                                |index: usize| assignment & (1usize << index) != 0;
                                            let base: i64 = weights
                                                .iter()
                                                .enumerate()
                                                .map(|(index, weight)| {
                                                    weight * i64::from(set(index))
                                                })
                                                .sum();
                                            let valid = match kind {
                                                0 | 4 => (assignment & 7).count_ones() <= 1,
                                                1 => (!set(1) || set(0)) && (!set(2) || set(1)),
                                                _ => (low..=high).contains(&base),
                                            };
                                            let value = base * scale
                                                + 5 * i64::from(set(3))
                                                + if repeated {
                                                    2 * i64::from(set(0)) - 3 * i64::from(!set(1))
                                                        + 4
                                                } else {
                                                    0
                                                };
                                            let truth = match relation {
                                                IntegerRelation::Equal => value == bound,
                                                IntegerRelation::NotEqual => value != bound,
                                                IntegerRelation::Less => value < bound,
                                                IntegerRelation::LessEqual => value <= bound,
                                                IntegerRelation::Greater => value > bound,
                                                IntegerRelation::GreaterEqual => value >= bound,
                                            };
                                            let expected = valid
                                                && set(output_index) == truth
                                                && assertion
                                                    .is_none_or(|value| set(output_index) == value);
                                            let assumptions: Vec<_> = variables
                                                .iter()
                                                .enumerate()
                                                .map(|(index, variable)| {
                                                    let literal = map[&variable.name()];
                                                    if set(index) { literal } else { !literal }
                                                })
                                                .collect();
                                            assert_eq!(
                                                solver.solve_assumps(&assumptions).unwrap()
                                                    == SolverResult::Sat,
                                                expected,
                                                "{algorithm} kind={kind} scale={scale} repeated={repeated} {relation:?} bound={bound} bits={assignment}"
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn structured_relation_uses_choice_bound_in_both_directions() {
        use crate::ast::sat_decision::IntegerRelation;
        for algorithm in [PbEncoding::PindakaasBdd, PbEncoding::PindakaasSwc] {
            let compile = |structured| {
                let mut instance = SatInstance::new();
                let terms: Vec<_> = [2, 5, 9]
                    .into_iter()
                    .map(|weight| (weight, Term::Literal(instance.new_lit())))
                    .collect();
                let output = instance.new_lit();
                let groups = [PbTermGroup {
                    start: 0,
                    end: 3,
                    structure: PbTermStructure::Choice,
                }];
                let mut variables = HashMap::new();
                Compiler {
                    instance: &mut instance,
                    variables: &mut variables,
                    counters: None,
                }
                .integer_relation(
                    algorithm,
                    Term::Literal(output),
                    IntegerRelation::LessEqual,
                    9,
                    &terms,
                    if structured { &groups } else { &[] },
                )
                .unwrap();
                let used = instance.var_manager_mut().n_used();
                (instance, output, used)
            };
            let (instance, output, structured) = compile(true);
            let (_, _, flat) = compile(false);
            assert!(
                structured < flat,
                "{algorithm}: group information must reach the encoder"
            );
            let (cnf, _): (Cnf, BasicVarManager) = instance.into_cnf();
            let mut solver = CaDiCaL::default();
            solver.add_cnf(cnf).unwrap();
            // The choice maximum makes <= 9 true and its opposite impossible.
            assert_eq!(solver.solve_assumps(&[output]).unwrap(), SolverResult::Sat);
            assert_eq!(
                solver.solve_assumps(&[!output]).unwrap(),
                SolverResult::Unsat
            );
        }
    }

    #[test]
    fn choice_structure_avoids_encoding_an_already_implied_bound() {
        let mut instance = SatInstance::new();
        let literals: Vec<_> = (0..3).map(|_| instance.new_lit()).collect();
        let terms: Vec<_> = [2, 5, 9]
            .into_iter()
            .zip(literals)
            .map(|(weight, lit)| (weight, Term::Literal(lit)))
            .collect();
        let groups = [PbTermGroup {
            start: 0,
            end: 3,
            structure: PbTermStructure::Choice,
        }];
        let mut variables = HashMap::new();
        let mut compiler = Compiler {
            instance: &mut instance,
            variables: &mut variables,
            counters: None,
        };
        compiler
            .pseudo_boolean(
                PbEncoding::PindakaasBdd,
                CardinalityRelation::AtMost,
                9,
                terms,
                &groups,
            )
            .unwrap();
        let (cnf, _): (Cnf, BasicVarManager) = instance.into_cnf();
        assert_eq!(
            cnf.len(),
            0,
            "The existing AMO makes this upper bound redundant"
        );
    }

    #[test]
    fn malformed_term_groups_are_rejected() {
        for groups in [
            vec![PbTermGroup {
                start: 0,
                end: 4,
                structure: PbTermStructure::Choice,
            }],
            vec![PbTermGroup {
                start: 1,
                end: 1,
                structure: PbTermStructure::Chain,
            }],
            vec![
                PbTermGroup {
                    start: 0,
                    end: 2,
                    structure: PbTermStructure::Choice,
                },
                PbTermGroup {
                    start: 1,
                    end: 3,
                    structure: PbTermStructure::Chain,
                },
            ],
            vec![PbTermGroup {
                start: 0,
                end: 3,
                structure: PbTermStructure::BoundedBinary { lower: 2, upper: 1 },
            }],
        ] {
            assert!(validate_pb_groups(&groups, 3).is_err());
        }
    }

    #[test]
    fn choice_normalisation_near_the_library_limit_keeps_valid_assignments() {
        let variables: Vec<_> = (0..2)
            .map(|index| {
                DeclarationPtr::new_find(Name::User(format!("large{index}").into()), Domain::bool())
            })
            .collect();
        let weight = i64::MAX / 3 + 2;
        for algorithm in [PbEncoding::PindakaasBdd, PbEncoding::PindakaasSwc] {
            for relation in [
                CardinalityRelation::AtMost,
                CardinalityRelation::AtLeast,
                CardinalityRelation::Exactly,
            ] {
                let mut instance = SatInstance::new();
                let mut map = HashMap::new();
                for variable in &variables {
                    map.insert(variable.name().clone(), instance.new_lit());
                }
                instance
                    .add_clause([!map[&variables[0].name()], !map[&variables[1].name()]].into());
                compile_decisions(
                    &[SatEncodingDecision::PseudoBoolean {
                        terms: [-weight, weight - 1]
                            .into_iter()
                            .zip(&variables)
                            .map(|(weight, variable)| {
                                (weight, Reference::new(variable.clone()).into())
                            })
                            .collect(),
                        groups: vec![PbTermGroup {
                            start: 0,
                            end: 2,
                            structure: PbTermStructure::Choice,
                        }],
                        relation,
                        bound: 0,
                        encoding: Some(EncodingSelection {
                            algorithm,
                            provenance: SelectionProvenance::ExplicitConfiguration,
                        }),
                    }],
                    &mut instance,
                    &mut map,
                )
                .unwrap();
                let (cnf, _): (Cnf, BasicVarManager) = instance.into_cnf();
                let mut solver = CaDiCaL::default();
                solver.add_cnf(cnf).unwrap();
                for assignment in 0..4 {
                    let first = assignment & 1 != 0;
                    let second = assignment & 2 != 0;
                    let value = -weight * i64::from(first) + (weight - 1) * i64::from(second);
                    let expected = !(first && second)
                        && match relation {
                            CardinalityRelation::AtMost => value <= 0,
                            CardinalityRelation::AtLeast => value >= 0,
                            CardinalityRelation::Exactly => value == 0,
                        };
                    let assumptions: Vec<_> = [first, second]
                        .into_iter()
                        .zip(&variables)
                        .map(|(set, variable)| {
                            let lit = map[&variable.name()];
                            if set { lit } else { !lit }
                        })
                        .collect();
                    assert_eq!(
                        solver.solve_assumps(&assumptions).unwrap() == SolverResult::Sat,
                        expected,
                        "{algorithm} {relation:?} bits={assignment}"
                    );
                }
            }
        }
    }

    #[test]
    fn weighted_providers_preserve_signed_bounds_constants_and_multiplicities() {
        let variables: Vec<_> = (0..3)
            .map(|index| {
                DeclarationPtr::new_find(Name::User(format!("w{index}").into()), Domain::bool())
            })
            .collect();
        let inputs: Vec<Expression> = variables
            .iter()
            .map(|variable| Reference::new(variable.clone()).into())
            .collect();
        for algorithm in PbEncoding::ALL {
            for weights in [[3, 5, -2], [0, 1, 1], [-3, -2, -1], [2, 2, 2], [1, 64, 257]] {
                for special in [false, true] {
                    let mut terms: Vec<_> =
                        weights.into_iter().zip(inputs.iter().cloned()).collect();
                    if special {
                        terms.extend([
                            (4, true.into()),
                            (-3, false.into()),
                            (2, inputs[0].clone()),
                            (
                                -3,
                                Expression::Not(Metadata::new(), Moo::new(inputs[0].clone())),
                            ),
                        ]);
                    }
                    for relation in [
                        CardinalityRelation::AtMost,
                        CardinalityRelation::AtLeast,
                        CardinalityRelation::Exactly,
                    ] {
                        for bound in (-12..=20).chain([63, 64, 65, 256, 257, 258, 320, 321, 322]) {
                            let mut instance = SatInstance::new();
                            let mut map = HashMap::new();
                            for variable in &variables {
                                map.insert(variable.name().clone(), instance.new_lit());
                            }
                            compile_decisions(
                                &[SatEncodingDecision::PseudoBoolean {
                                    terms: terms.clone(),
                                    groups: vec![],
                                    relation,
                                    bound,
                                    encoding: Some(EncodingSelection {
                                        algorithm,
                                        provenance: SelectionProvenance::ExplicitConfiguration,
                                    }),
                                }],
                                &mut instance,
                                &mut map,
                            )
                            .unwrap();
                            let used = instance.var_manager_mut().n_used();
                            assert_eq!(instance.new_lit().var().idx32(), used);
                            let (cnf, _): (Cnf, BasicVarManager) = instance.into_cnf();
                            let mut solver = CaDiCaL::default();
                            solver.add_cnf(cnf).unwrap();
                            for assignment in 0..8 {
                                let active = |index| i64::from(assignment & (1usize << index) != 0);
                                let mut value: i64 = weights
                                    .into_iter()
                                    .enumerate()
                                    .map(|(index, weight)| weight * active(index))
                                    .sum();
                                if special {
                                    value += 4 + 2 * active(0) - 3 * (1 - active(0));
                                }
                                let expected = match relation {
                                    CardinalityRelation::AtMost => value <= bound,
                                    CardinalityRelation::AtLeast => value >= bound,
                                    CardinalityRelation::Exactly => value == bound,
                                };
                                let assumptions: Vec<_> = variables
                                    .iter()
                                    .enumerate()
                                    .map(|(index, variable)| {
                                        let literal = map[&variable.name()];
                                        if active(index) == 1 {
                                            literal
                                        } else {
                                            !literal
                                        }
                                    })
                                    .collect();
                                assert_eq!(
                                    solver.solve_assumps(&assumptions).unwrap()
                                        == SolverResult::Sat,
                                    expected,
                                    "{algorithm} {relation:?} weights={weights:?} special={special} bound={bound} assignment={assignment}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn weighted_overflow_is_reported_without_wrapping_or_panicking() {
        let variable = DeclarationPtr::new_find(Name::User("overflow".into()), Domain::bool());
        let input: Expression = Reference::new(variable).into();
        for algorithm in PbEncoding::ALL {
            for (terms, bound) in [
                (vec![(i64::MAX, input.clone())], 1),
                (vec![(i64::MIN, input.clone())], -1),
            ] {
                assert!(
                    compile_decisions(
                        &[SatEncodingDecision::PseudoBoolean {
                            terms,
                            groups: vec![],
                            relation: CardinalityRelation::Exactly,
                            bound,
                            encoding: Some(EncodingSelection {
                                algorithm,
                                provenance: SelectionProvenance::ExplicitConfiguration
                            })
                        }],
                        &mut SatInstance::new(),
                        &mut HashMap::new()
                    )
                    .is_err()
                );
            }
        }
    }
}

#[cfg(test)]
mod element_tests {
    use super::*;
    use crate::ast::sat_decision::{
        ElementEncoding, EncodingSelection, PbEncoding, PbTermGroup, PbTermStructure,
        SatIntegerView, SelectionProvenance,
    };
    use crate::ast::{DeclarationPtr, Domain, Reference};
    use rustsat::{
        instances::{BasicVarManager, Cnf, ManageVars},
        solvers::{Solve, SolveIncremental, SolverResult},
    };
    use rustsat_cadical::CaDiCaL;

    fn constant(value: i64) -> SatIntegerView {
        SatIntegerView {
            constant: value,
            terms: vec![],
            groups: vec![],
            choices: None,
        }
    }

    #[test]
    fn element_preserves_sparse_values_shared_views_and_free_outside_indices() {
        let variables: Vec<_> = (0..5)
            .map(|i| {
                DeclarationPtr::new_find(Name::User(format!("element_{i}").into()), Domain::bool())
            })
            .collect();
        let bits: Vec<Expression> = variables
            .iter()
            .cloned()
            .map(Reference::new)
            .map(Into::into)
            .collect();
        let inverse = Expression::Not(
            crate::ast::Metadata::new(),
            crate::ast::Moo::new(bits[0].clone()),
        );
        let index = SatIntegerView {
            constant: 0,
            terms: vec![(-2, inverse.clone()), (3, bits[0].clone())],
            groups: vec![PbTermGroup {
                start: 0,
                end: 2,
                structure: PbTermStructure::Choice,
            }],
            choices: Some(vec![(-2, inverse), (3, bits[0].clone())]),
        };
        let value = SatIntegerView {
            constant: -1,
            terms: vec![(2, bits[1].clone()), (3, bits[2].clone())],
            groups: vec![PbTermGroup {
                start: 0,
                end: 2,
                structure: PbTermStructure::Chain,
            }],
            choices: None,
        };
        let entry = SatIntegerView {
            constant: -1,
            terms: vec![(2, bits[3].clone()), (3, bits[4].clone())],
            groups: vec![PbTermGroup {
                start: 0,
                end: 2,
                structure: PbTermStructure::BoundedBinary { lower: 0, upper: 5 },
            }],
            choices: None,
        };
        for algorithm in ElementEncoding::ALL {
            for pb in PbEncoding::ALL {
                for entries in [
                    vec![(-2, entry.clone()), (3, value.clone())],
                    vec![(-2, constant(4))],
                    vec![],
                ] {
                    let mut instance = SatInstance::new();
                    let mut map = HashMap::new();
                    for variable in &variables {
                        map.insert(variable.name().clone(), instance.new_lit());
                    }
                    let decision = SatEncodingDecision::Element {
                        index_view: index.clone(),
                        value: value.clone(),
                        entries: entries.clone(),
                        encoding: Some(EncodingSelection {
                            algorithm,
                            provenance: SelectionProvenance::ExplicitConfiguration,
                        }),
                        pb_encoding: Some(EncodingSelection {
                            algorithm: pb,
                            provenance: SelectionProvenance::ExplicitConfiguration,
                        }),
                    };
                    assert_eq!(
                        serde_json::from_str::<SatEncodingDecision>(
                            &serde_json::to_string(&decision).unwrap()
                        )
                        .unwrap(),
                        decision
                    );
                    let invariants = SatEncodingDecision::Assert(Expression::Imply(
                        crate::ast::Metadata::new(),
                        crate::ast::Moo::new(bits[2].clone()),
                        crate::ast::Moo::new(bits[1].clone()),
                    ));
                    compile_decisions(&[decision, invariants], &mut instance, &mut map).unwrap();
                    let used = instance.var_manager_mut().n_used();
                    assert_eq!(instance.new_lit().var().idx32(), used);
                    let (cnf, _): (Cnf, BasicVarManager) = instance.into_cnf();
                    let mut solver = CaDiCaL::default();
                    solver.add_cnf(cnf).unwrap();
                    for assignment in 0usize..32 {
                        let set = |i| assignment & (1usize << i) != 0;
                        let index_number = if set(0) { 3 } else { -2 };
                        let numeric = |view: &SatIntegerView| {
                            view.constant
                                + view
                                    .terms
                                    .iter()
                                    .map(|(weight, expression)| {
                                        let position =
                                            bits.iter().position(|bit| bit == expression).unwrap();
                                        weight * i64::from(set(position))
                                    })
                                    .sum::<i64>()
                        };
                        let valid_chain = !set(2) || set(1);
                        let expected = valid_chain
                            && entries
                                .iter()
                                .filter(|(label, _)| *label == index_number)
                                .all(|(_, entry)| numeric(&value) == numeric(entry));
                        let assumptions: Vec<_> = variables
                            .iter()
                            .enumerate()
                            .map(|(i, variable)| {
                                let lit = map[&variable.name()];
                                if set(i) { lit } else { !lit }
                            })
                            .collect();
                        assert_eq!(
                            solver.solve_assumps(&assumptions).unwrap() == SolverResult::Sat,
                            expected,
                            "{algorithm:?}/{pb:?}, assignment {assignment}, entries {entries:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn element_rejects_duplicate_labels_and_unresolved_algorithms() {
        let mut decision = SatEncodingDecision::Element {
            index_view: constant(i64::MIN),
            value: constant(i64::MAX),
            entries: vec![(i64::MIN, constant(i64::MAX))],
            encoding: Some(EncodingSelection {
                algorithm: ElementEncoding::Support,
                provenance: SelectionProvenance::Heuristic,
            }),
            pb_encoding: Some(EncodingSelection {
                algorithm: PbEncoding::RustsatBinaryAdder,
                provenance: SelectionProvenance::Heuristic,
            }),
        };
        let compile =
            |decision| compile_decisions(&[decision], &mut SatInstance::new(), &mut HashMap::new());
        assert!(compile(decision.clone()).is_ok());
        if let SatEncodingDecision::Element { entries, .. } = &mut decision {
            entries.push(entries[0].clone());
        }
        assert!(compile(decision.clone()).is_err());
        if let SatEncodingDecision::Element {
            entries, encoding, ..
        } = &mut decision
        {
            entries.pop();
            *encoding = None;
        }
        assert!(compile(decision.clone()).is_err());
        if let SatEncodingDecision::Element {
            encoding,
            pb_encoding,
            ..
        } = &mut decision
        {
            *encoding = Some(EncodingSelection {
                algorithm: ElementEncoding::Implication,
                provenance: SelectionProvenance::Heuristic,
            });
            *pb_encoding = None;
        }
        assert!(compile(decision).is_err());
    }
}
