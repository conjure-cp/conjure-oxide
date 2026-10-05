//! Cardinality constraints and shared occurrence counters.
use super::*;

impl Compiler<'_> {
    pub(super) fn cached_cardinality(
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

    pub(super) fn count_relation(
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

    pub(super) fn count_upper(
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

    pub(super) fn cardinality(
        &mut self,
        algorithm: crate::ast::sat_decision::CardinalityEncoding,
        relation: crate::ast::sat_decision::CardinalityRelation,
        bound: i64,
        terms: Vec<Term>,
    ) -> Result<(), SolverError> {
        self.cardinality_guarded(algorithm, relation, bound, terms, None)
    }

    pub(super) fn cardinality_guarded(
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::sat_decision::{
        CardinalityEncoding, CardinalityRelation, EncodingSelection, SelectionProvenance,
    };
    use crate::ast::{DeclarationPtr, Domain, Metadata, Moo, Reference};
    use crate::solver::adaptors::rustsat::adaptor::SatSolver;
    use rustsat::{
        instances::{BasicVarManager, ManageVars},
        solvers::{Solve, SolveIncremental, SolverResult},
    };
    #[test]
    fn reusable_cardinality_batches_preserve_guards_and_occurrence_counts() {
        for algorithm in CardinalityEncoding::ALL {
            let mut cache = EncodingCache::default();
            let mut variables = HashMap::new();
            let mut solver = SatSolver::default();
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
                                    let mut solver = SatSolver::default();
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
                            let mut solver = SatSolver::default();
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
