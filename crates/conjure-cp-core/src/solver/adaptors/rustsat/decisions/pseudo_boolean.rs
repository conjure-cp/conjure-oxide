//! Weighted pseudo-Boolean constraints and shared threshold predicates.
use super::*;

impl Compiler<'_> {
    // Keep the original polarities and group bounds in both implication directions.
    pub(super) fn structured_reified_upper(
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

    pub(super) fn reified_upper(
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

    pub(super) fn cached_weighted_threshold(
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

    pub(super) fn cached_weighted_bound(
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

    pub(super) fn pseudo_boolean(
        &mut self,
        algorithm: crate::ast::sat_decision::PbEncoding,
        relation: crate::ast::sat_decision::CardinalityRelation,
        bound: i64,
        terms: Vec<(i64, Term)>,
        groups: &[crate::ast::sat_decision::PbTermGroup],
    ) -> Result<(), SolverError> {
        self.guarded_pseudo_boolean(algorithm, relation, bound, terms, groups, None)
    }

    pub(super) fn guarded_pseudo_boolean(
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
    pub(super) fn structured_pb_expression(
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
}

#[cfg(test)]
mod tests {
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
    fn variable_table_rows_and_wildcards_preserve_full_reification() {
        use crate::ast::sat_decision::{SatIntegerView, TableEncoding};
        let variables = (0..4)
            .map(|index| {
                DeclarationPtr::new_find(
                    Name::User(format!("variable_table_{index}").into()),
                    Domain::bool(),
                )
            })
            .collect::<Vec<_>>();
        let bits = variables
            .iter()
            .cloned()
            .map(Reference::new)
            .map(Expression::from)
            .collect::<Vec<_>>();
        let view = |index: usize| SatIntegerView {
            constant: 0,
            terms: vec![(1, bits[index].clone())],
            groups: vec![],
            choices: None,
        };
        let constant = |value| SatIntegerView {
            constant: value,
            terms: vec![],
            groups: vec![],
            choices: None,
        };
        for algorithm in [TableEncoding::Tuple, TableEncoding::Mdd] {
            for pb in PbEncoding::ALL {
                for negative in [false, true] {
                    for wildcard in [false, true] {
                        let rows = if wildcard {
                            vec![vec![view(0), constant(1)], vec![constant(1), view(1)]]
                        } else {
                            vec![vec![view(2), view(0)], vec![constant(0), constant(1)]]
                        };
                        let decision = SatEncodingDecision::Table {
                            output: bits[3].clone(),
                            inputs: vec![view(0), view(1)],
                            rows: vec![],
                            row_views: Some(rows),
                            negative,
                            encoding: Some(EncodingSelection {
                                algorithm,
                                provenance: SelectionProvenance::ExplicitConfiguration,
                            }),
                            pb_encoding: Some(EncodingSelection {
                                algorithm: pb,
                                provenance: SelectionProvenance::ExplicitConfiguration,
                            }),
                        };
                        let mut instance = SatInstance::new();
                        let mut map = HashMap::new();
                        for variable in &variables {
                            map.insert(variable.name().clone(), instance.new_lit());
                        }
                        compile_decisions(&[decision], &mut instance, &mut map).unwrap();
                        let mut solver = CaDiCaL::default();
                        solver.add_cnf(instance.cnf().clone()).unwrap();
                        for assignment in 0..16 {
                            let set = |index| assignment & (1 << index) != 0;
                            let truth = if wildcard {
                                set(0) || set(1)
                            } else {
                                (set(0) == set(2) && set(1) == set(0)) || (!set(0) && set(1))
                            };
                            let assumptions = variables
                                .iter()
                                .enumerate()
                                .map(|(index, variable)| {
                                    if set(index) {
                                        map[&variable.name()]
                                    } else {
                                        !map[&variable.name()]
                                    }
                                })
                                .collect::<Vec<_>>();
                            assert_eq!(
                                solver.solve_assumps(&assumptions).unwrap() == SolverResult::Sat,
                                set(3) == (truth != negative),
                                "{algorithm} {pb} wildcard={wildcard} negative={negative} assignment={assignment}"
                            );
                        }
                    }
                }
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
            row_views: None,
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
        for strategy in [TableEncoding::Tuple, TableEncoding::Mdd] {
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
                            (views[..2].to_vec(), vec![vec![-2, -1], vec![3, 4]]),
                            (views[..2].to_vec(), vec![]),
                            (views.clone(), vec![]),
                            (vec![], vec![]),
                            (vec![], vec![vec![]]),
                        ] {
                            if strategy == TableEncoding::BinarySupport && inputs.len() != 2 {
                                continue;
                            }
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
                                    row_views: None,
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
                                let mut numeric = if inputs.is_empty() {
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
                                numeric.truncate(inputs.len());
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
                        [
                            None,
                            Some(-2),
                            Some(0),
                            Some(3),
                            Some(42),
                            Some(43),
                            Some(44),
                        ]
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
                                comparisons: None,
                                output: expressions[9].clone(),
                                inputs: inputs.clone(),
                                except: except.map(|constant| match constant {
                                    43 => views[0].clone(),
                                    44 => views[2].clone(),
                                    _ => SatIntegerView {
                                        constant,
                                        terms: vec![],
                                        groups: vec![],
                                        choices: None,
                                    },
                                }),
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
                                            || Some(numeric[left])
                                                == match except {
                                                    Some(43) => Some(numeric[0]),
                                                    Some(44) => Some(
                                                        values[2]
                                                            .iter()
                                                            .enumerate()
                                                            .map(|(column, value)| {
                                                                value * i64::from(set(6 + column))
                                                            })
                                                            .sum(),
                                                    ),
                                                    value => value,
                                                }
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
