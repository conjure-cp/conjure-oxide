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

#[derive(Clone, Copy)]
enum Term {
    Constant(bool),
    Literal(Lit),
}
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
    let mut compiler = Compiler {
        instance,
        variables,
    };
    for decision in decisions {
        match decision {
            SatEncodingDecision::PseudoBoolean {
                terms,
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
                compiler.pseudo_boolean(algorithm, *relation, *bound, terms)?;
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
                let true_count = terms
                    .iter()
                    .filter(|term| matches!(term, Term::Constant(true)))
                    .count();
                let literals = terms
                    .into_iter()
                    .filter_map(|term| match term {
                        Term::Literal(lit) => Some(lit),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if true_count >= 2 {
                    compiler.assert(Term::Constant(false));
                } else if true_count == 1 {
                    for lit in literals {
                        compiler.assert(Term::Literal(!lit));
                    }
                } else {
                    compiler.amo(algorithm, literals)?;
                }
            }
            SatEncodingDecision::Assert(expression) => {
                let term = compiler.encode(expression)?;
                compiler.assert(term);
            }
            SatEncodingDecision::Boolean { output, expression } => {
                let output = compiler.encode(output)?;
                let value = compiler.encode(expression)?;
                match (output, value) {
                    (Term::Literal(output), Term::Literal(value)) => {
                        compiler
                            .instance
                            .add_clause(atomics::lit_impl_lit(output, value));
                        compiler
                            .instance
                            .add_clause(atomics::lit_impl_lit(value, output));
                    }
                    (Term::Constant(value), term) | (term, Term::Constant(value)) => {
                        compiler.assert(if value { term } else { term.negated() })
                    }
                }
            }
        }
    }
    Ok(())
}
struct Compiler<'a> {
    instance: &'a mut SatInstance,
    variables: &'a mut HashMap<Name, Lit>,
}
impl Compiler<'_> {
    fn pseudo_boolean(
        &mut self,
        algorithm: crate::ast::sat_decision::PbEncoding,
        relation: crate::ast::sat_decision::CardinalityRelation,
        bound: i64,
        terms: Vec<(i64, Term)>,
    ) -> Result<(), SolverError> {
        use crate::ast::sat_decision::{CardinalityRelation, PbEncoding};
        // Aggregate by variable before making weights positive, retaining multiplicity
        // and cancelling complements. Widening keeps signed boundary values safe.
        let mut bound = i128::from(bound);
        let mut coefficients = std::collections::BTreeMap::<Lit, i128>::new();
        for (weight, term) in terms {
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
            self.assert(Term::Constant(false));
            return Ok(());
        }
        if (relation == CardinalityRelation::AtMost && bound >= total)
            || (relation == CardinalityRelation::AtLeast && bound <= 0)
        {
            return Ok(());
        }
        if bound == 0 || bound == total {
            for (literal, _) in positive {
                self.assert(Term::Literal(if bound == total {
                    literal
                } else {
                    !literal
                }));
            }
            return Ok(());
        }
        // Both libraries use signed machine-sized bounds and may calculate bound + 1.
        if total >= isize::MAX as i128 || total >= i128::from(i64::MAX) {
            return Err(SolverError::ModelInvalid(
                "Pseudo-Boolean coefficient sum exceeds the library range".into(),
            ));
        }
        let positive: Vec<_> = positive
            .into_iter()
            .map(|(literal, weight)| (literal, weight as usize))
            .collect();
        match algorithm {
            PbEncoding::RustsatGeneralizedTotalizer
            | PbEncoding::RustsatBinaryAdder
            | PbEncoding::RustsatDynamicPolyWatchdog => {
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
                let comparison = match relation {
                    CardinalityRelation::AtMost => Comparator::LessEq,
                    CardinalityRelation::AtLeast => Comparator::GreaterEq,
                    CardinalityRelation::Exactly => Comparator::Equal,
                };
                let expression = BoolLinExp::from_terms(
                    &positive
                        .into_iter()
                        .map(|(literal, weight)| (pind_lit(literal), weight as i64))
                        .collect::<Vec<_>>(),
                );
                let mut sink = PindakaasSink(self.instance);
                let variant = BoolLinAggregator::default().aggregate(
                    &mut sink,
                    &BoolLinear::new(expression, comparison, bound as i64),
                );
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
                    self.assert(Term::Constant(false));
                }
            }
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
            self.assert(Term::Constant(false));
            return Ok(());
        }
        if (relation == CardinalityRelation::AtMost && bound >= size)
            || (relation == CardinalityRelation::AtLeast && bound <= 0)
        {
            return Ok(());
        }
        if bound == 0 || bound == size {
            for literal in literals {
                self.assert(Term::Literal(if bound == size {
                    literal
                } else {
                    !literal
                }));
            }
            return Ok(());
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
                for clause in cnf {
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
                let mut sink = PindakaasSink(self.instance);
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
                    self.assert(Term::Constant(false));
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
        for clause in cnf {
            self.instance.add_clause(clause);
        }
        Ok(())
    }
    fn assert(&mut self, term: Term) {
        match term {
            Term::Constant(true) => (),
            Term::Constant(false) => self.instance.add_clause(Clause::new()),
            Term::Literal(lit) => self.instance.add_clause([lit].into_iter().collect()),
        }
    }
    fn combine(&mut self, and: bool, terms: Vec<Term>) -> Term {
        let mut literals = Vec::new();
        for term in terms {
            match term {
                Term::Constant(value) if value != and => return Term::Constant(value),
                Term::Constant(_) => (),
                Term::Literal(lit) => literals.push(lit),
            }
        }
        match literals.as_slice() {
            [] => Term::Constant(and),
            [lit] => Term::Literal(*lit),
            _ => {
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
                Term::Literal(
                    *self
                        .variables
                        .entry(reference.name().clone())
                        .or_insert_with(|| self.instance.new_lit()),
                )
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
struct PindakaasSink<'a>(&'a mut SatInstance);
impl pindakaas::ClauseDatabase for PindakaasSink<'_> {
    fn add_clause_from_slice(
        &mut self,
        clause: &[pindakaas::Lit],
    ) -> Result<(), pindakaas::Unsatisfiable> {
        self.0.add_clause(
            clause
                .iter()
                .map(|lit| {
                    let raw: std::num::NonZeroI32 = (*lit).into();
                    Lit::from_ipasir(raw.get()).unwrap()
                })
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
        let first = pind_lit(self.0.new_lit()).var();
        let mut last = first;
        for _ in 1..len {
            last = pind_lit(self.0.new_lit()).var();
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
    use crate::ast::{DeclarationPtr, Domain, Reference};
    use rustsat::{
        instances::{BasicVarManager, ManageVars},
        solvers::{Solve, SolveIncremental, SolverResult},
    };
    use rustsat_cadical::CaDiCaL;
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
        CardinalityRelation, EncodingSelection, PbEncoding, SelectionProvenance,
    };
    use crate::ast::{DeclarationPtr, Domain, Metadata, Moo, Reference};
    use rustsat::{
        instances::{BasicVarManager, ManageVars},
        solvers::{Solve, SolveIncremental, SolverResult},
    };
    use rustsat_cadical::CaDiCaL;

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
                                let active = |index| i64::from(assignment & (1 << index) != 0);
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
