//! Generate solver clauses directly from semantic Boolean encoding decisions.
mod alldifferent;
mod amo;
mod cardinality;
mod element;
mod pseudo_boolean;
mod relation;
mod table;

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
                row_views,
                negative,
                encoding,
                pb_encoding,
            } => {
                let output = compiler.encode(output)?;
                if let Some(row_views) = row_views {
                    compiler.general_table(
                        output,
                        inputs,
                        row_views,
                        *negative,
                        encoding,
                        pb_encoding,
                    )?;
                } else {
                    compiler.table(output, inputs, rows, *negative, encoding, pb_encoding)?;
                }
            }
            SatEncodingDecision::AllDifferent {
                output,
                inputs,
                except,
                comparisons,
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
                if let Some(comparisons) = comparisons {
                    if encoding.as_ref().is_none_or(|selection| {
                        selection.algorithm
                            != crate::ast::sat_decision::AllDifferentEncoding::Pairwise
                    }) {
                        return Err(SolverError::ModelInvalid(
                            "Compound allDifferent requires the pairwise encoding".into(),
                        ));
                    }
                    let terms = comparisons
                        .iter()
                        .map(|condition| compiler.encode(condition))
                        .collect::<Result<Vec<_>, _>>()?;
                    let truth = compiler.combine(true, terms);
                    compiler.equate(output, truth);
                    continue;
                }
                compiler.alldifferent(
                    output,
                    inputs,
                    except.as_ref(),
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
mod tests;
