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
            let inputs = distinct_occurrence_literals(self.instance, &literals);
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
                    constraint::cardinality::SortingNetworkEncoder,
                    constraint::linear::{Comparator, LinAggregator, LinExp, LinVariant, Linear},
                };
                // Pindakaas cardinality inputs must use distinct variables. Alias repeats and
                // opposite polarities instead of allowing aggregation into weighted PB terms.
                let literals = distinct_occurrence_literals(self.instance, &literals)
                    .into_iter()
                    .map(pind_lit)
                    .collect::<Vec<_>>();
                let comparison = match relation {
                    CardinalityRelation::AtMost => Comparator::LessEq,
                    CardinalityRelation::AtLeast => Comparator::GreaterEq,
                    CardinalityRelation::Exactly => Comparator::Equal,
                };
                let expression = LinExp::from_terms(
                    &literals.into_iter().map(|lit| (lit, 1)).collect::<Vec<_>>(),
                );
                let mut sink = PindakaasSink {
                    instance: self.instance,
                    guard,
                };
                let variant = LinAggregator::default()
                    .aggregate(&mut sink, &Linear::new(expression, comparison, bound));
                let encoder = SortingNetworkEncoder::default();
                let result = match variant {
                    Ok(LinVariant::Cardinality(cardinality)) => {
                        encoder.encode(&mut sink, &cardinality)
                    }
                    Ok(LinVariant::CardinalityOne(cardinality)) => encoder.encode(
                        &mut sink,
                        &pindakaas::constraint::cardinality::Cardinality::from(cardinality),
                    ),
                    Ok(LinVariant::Trivial) => Ok(()),
                    Ok(
                        LinVariant::Linear(_) | LinVariant::BoolLinear(_) | LinVariant::Count(_),
                    ) => {
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
