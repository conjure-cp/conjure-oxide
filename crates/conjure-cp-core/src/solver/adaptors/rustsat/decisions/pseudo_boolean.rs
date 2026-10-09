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
                let structure = match group.structure {
                    PbTermStructure::Choice => WeightedGroupStructure::Choice,
                    PbTermStructure::Chain => WeightedGroupStructure::Chain,
                    PbTermStructure::BoundedBinary { lower, upper } => {
                        let a = i128::from(lower) * sign;
                        let b = i128::from(upper) * sign;
                        WeightedGroupStructure::BoundedBinary {
                            lower: a.min(b),
                            upper: a.max(b),
                        }
                    }
                };
                // Chains carry implication order; choice/binary sums do not.
                if structure != WeightedGroupStructure::Chain {
                    members.sort_unstable();
                }
                WeightedGroupKey { structure, members }
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
        // Aggregate by variable before making weights positive, retaining multiplicity
        // and cancelling complements. Widening keeps signed boundary values safe.
        let (bound, coefficients) = canonical_pb_terms(bound, &terms);
        let structured_input = (!groups.is_empty()
            && matches!(
                algorithm,
                PbEncoding::PindakaasBdd | PbEncoding::PindakaasSwc
            ))
        .then(|| (bound, coefficients.clone()));
        let (mut bound, mut positive, mut total) = positive_pb_terms(bound, coefficients);
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
                    constraint::linear::{Comparator, LinAggregator, LinExp, Linear},
                    encoder::{
                        decision_diagram::DecisionDiagramEncoder,
                        sequential_counter::SequentialCounterEncoder,
                    },
                };
                // A single AMO group's extrema include the none-selected value zero.
                // Fold fixed bounds before allocating that explicit direct-view indicator.
                if let [group] = groups
                    && group.start == 0
                    && group.end == terms.len()
                    && group.structure == crate::ast::sat_decision::PbTermStructure::Choice
                    && terms
                        .iter()
                        .all(|(_, term)| matches!(term, Term::Literal(lit) if !lit.is_neg()))
                    && terms
                        .iter()
                        .map(|(_, term)| *term)
                        .collect::<std::collections::HashSet<_>>()
                        .len()
                        == terms.len()
                {
                    let lower = terms
                        .iter()
                        .map(|(weight, _)| i128::from(*weight))
                        .min()
                        .unwrap_or(0)
                        .min(0);
                    let upper = terms
                        .iter()
                        .map(|(weight, _)| i128::from(*weight))
                        .max()
                        .unwrap_or(0)
                        .max(0);
                    let original_bound = structured_input.as_ref().unwrap().0;
                    let fixed = match relation {
                        CardinalityRelation::AtMost if original_bound >= upper => Some(true),
                        CardinalityRelation::AtMost if original_bound < lower => Some(false),
                        CardinalityRelation::AtLeast if original_bound <= lower => Some(true),
                        CardinalityRelation::AtLeast if original_bound > upper => Some(false),
                        CardinalityRelation::Exactly
                            if original_bound < lower || original_bound > upper =>
                        {
                            Some(false)
                        }
                        CardinalityRelation::Exactly if lower == upper => {
                            Some(original_bound == lower)
                        }
                        _ => None,
                    };
                    if let Some(value) = fixed {
                        self.assert_guarded(Term::Constant(value), guard);
                        return Ok(());
                    }
                }
                let comparison = match relation {
                    CardinalityRelation::AtMost => Comparator::LessEq,
                    CardinalityRelation::AtLeast => Comparator::GreaterEq,
                    CardinalityRelation::Exactly => Comparator::Equal,
                };
                let input = if let Some((bound, coefficients)) = structured_input {
                    self.structured_pb_expression(bound, coefficients, &terms, groups)
                } else {
                    Ok((
                        LinExp::from_terms(
                            &positive
                                .into_iter()
                                .map(|(literal, weight)| (pind_lit(literal), weight as i64))
                                .collect::<Vec<_>>(),
                        ),
                        bound as i64,
                    ))
                };
                let mut sink = PindakaasSink {
                    instance: self.instance,
                    guard,
                };
                let result = input.and_then(|(expression, bound)| {
                    let variant = LinAggregator::default()
                        .aggregate(&mut sink, &Linear::new(expression, comparison, bound))?;
                    match algorithm {
                        PbEncoding::PindakaasBdd => {
                            DecisionDiagramEncoder::default().encode(&mut sink, &variant)
                        }
                        PbEncoding::PindakaasSwc => {
                            SequentialCounterEncoder::default().encode(&mut sink, &variant)
                        }
                        _ => unreachable!(),
                    }
                });
                if result.is_err() {
                    self.assert_guarded(Term::Constant(false), guard);
                }
            }
        }
        Ok(())
    }
    /// Share guaranteed representation structure as native Pindakaas integer views.
    fn structured_pb_expression(
        &mut self,
        mut bound: i128,
        mut coefficients: std::collections::BTreeMap<Lit, i128>,
        terms: &[(i64, Term)],
        groups: &[crate::ast::sat_decision::PbTermGroup],
    ) -> Result<(pindakaas::constraint::linear::LinExp, i64), pindakaas::Unsatisfiable> {
        use crate::ast::sat_decision::PbTermStructure;
        use pindakaas::{BoolVal, constraint::linear::LinExp, decision::integer::IntVar};
        let mut occurrences = HashMap::<Lit, usize>::new();
        for (_, term) in terms {
            if let Term::Literal(literal) = term {
                *occurrences.entry(literal.var().pos_lit()).or_default() += 1;
            }
        }
        let mut expression = LinExp::default();
        for group in groups {
            let inputs = terms[group.start..group.end]
                .iter()
                .map(|(_, term)| match term {
                    // The guaranteed choice/chain refers to these original polarities.
                    Term::Literal(literal) if !literal.is_neg() => Some(*literal),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>();
            let Some(inputs) = inputs else { continue };
            if inputs
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != inputs.len()
            {
                continue;
            }
            let weights: Vec<_> = inputs
                .iter()
                .map(|literal| coefficients.get(literal).copied().unwrap_or(0))
                .collect();
            if weights.iter().all(|weight| *weight == 0) {
                continue;
            }
            let (view, factor) = match group.structure {
                PbTermStructure::Choice => {
                    // A choice permits none selected. Merge equal-valued alternatives,
                    // including zero, before supplying exactly-one direct indicators.
                    let mut values = std::collections::BTreeMap::<i64, Vec<Term>>::new();
                    let selected = self.combine(
                        false,
                        inputs
                            .iter()
                            .map(|literal| Term::Literal(*literal))
                            .collect(),
                    );
                    values.entry(0).or_default().push(selected.negated());
                    for (&literal, &weight) in inputs.iter().zip(&weights) {
                        values
                            .entry(weight as i64)
                            .or_default()
                            .push(Term::Literal(literal));
                    }
                    let walk = values
                        .into_iter()
                        .map(|(value, terms)| (value, pind_term(self.combine(false, terms))))
                        .collect::<Vec<_>>();
                    let mut sink = PindakaasSink {
                        instance: self.instance,
                        guard: None,
                    };
                    (IntVar::from_direct_walk(&mut sink, walk)?, 1)
                }
                PbTermStructure::Chain => {
                    let walk = if weights.iter().all(|weight| *weight >= 0) {
                        let mut sum = 0i128;
                        let mut walk = vec![(0, BoolVal::Const(true))];
                        for (&literal, &weight) in inputs.iter().zip(&weights) {
                            sum += weight;
                            if weight != 0 {
                                walk.push((sum as i64, pind_lit(literal).into()));
                            }
                        }
                        Some(walk)
                    } else if weights.iter().all(|weight| *weight <= 0) {
                        let mut sum: i128 = weights.iter().sum();
                        let mut walk = vec![(sum as i64, BoolVal::Const(true))];
                        for (&literal, &weight) in inputs.iter().zip(&weights).rev() {
                            sum -= weight;
                            if weight != 0 {
                                walk.push((sum as i64, pind_lit(!literal).into()));
                            }
                        }
                        Some(walk)
                    } else {
                        None
                    };
                    let view = if let Some(walk) = walk {
                        let mut sink = PindakaasSink {
                            instance: self.instance,
                            guard: None,
                        };
                        IntVar::from_order_walk(&mut sink, walk)?
                    } else {
                        // Mixed signed steps can revisit values; use prefix boundary
                        // indicators as a direct view instead of assuming monotonicity.
                        let mut values = std::collections::BTreeMap::<i64, Vec<Term>>::new();
                        values.entry(0).or_default().push(Term::Literal(!inputs[0]));
                        let mut sum = 0i128;
                        for (index, &weight) in weights.iter().enumerate() {
                            sum += weight;
                            let selected = if index + 1 == inputs.len() {
                                Term::Literal(inputs[index])
                            } else {
                                self.combine(
                                    true,
                                    vec![
                                        Term::Literal(inputs[index]),
                                        Term::Literal(!inputs[index + 1]),
                                    ],
                                )
                            };
                            values.entry(sum as i64).or_default().push(selected);
                        }
                        let walk = values
                            .into_iter()
                            .map(|(value, terms)| (value, pind_term(self.combine(false, terms))))
                            .collect::<Vec<_>>();
                        let mut sink = PindakaasSink {
                            instance: self.instance,
                            guard: None,
                        };
                        IntVar::from_direct_walk(&mut sink, walk)?
                    };
                    (view, 1)
                }
                PbTermStructure::BoundedBinary { lower, upper } => {
                    // Combining another occurrence changes the meaning of the original bound.
                    if inputs.iter().any(|literal| {
                        occurrences[literal] != 1 || !coefficients.contains_key(literal)
                    }) {
                        continue;
                    }
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
                    let lower = i128::from(lower) - constant;
                    let upper = i128::from(upper) - constant;
                    let capacity: i128 = weights.iter().map(|weight| weight.abs()).sum();
                    if lower < 0 || upper > capacity || lower % factor != 0 || upper % factor != 0 {
                        continue;
                    }
                    let upper = (upper / factor) as i64;
                    let width = (i64::BITS - upper.leading_zeros()) as usize;
                    let bits: Vec<_> = inputs
                        .iter()
                        .zip(&weights)
                        .take(width)
                        .map(|(&literal, &weight)| {
                            pind_lit(if weight < 0 { !literal } else { literal }).into()
                        })
                        .collect();
                    let mut sink = PindakaasSink {
                        instance: self.instance,
                        guard: None,
                    };
                    let view = IntVar::from_binary_encoding(
                        &mut sink,
                        (lower / factor) as i64..=upper,
                        &bits,
                        0,
                    )?;
                    bound -= constant;
                    (view, factor as i64)
                }
            };
            for literal in inputs {
                coefficients.remove(&literal);
            }
            expression += view * factor;
        }
        let free: Vec<_> = coefficients
            .into_iter()
            .filter(|(_, weight)| *weight != 0)
            .map(|(literal, weight)| (pind_lit(literal), weight as i64))
            .collect();
        expression += LinExp::from_terms(&free);
        Ok((expression, bound as i64))
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
    use crate::solver::adaptors::rustsat::adaptor::SatSolver;
    use rustsat::instances::{BasicVarManager, ManageVars};
    use rustsat::solvers::{Solve, SolveIncremental, SolverResult};

    #[test]
    fn structured_choice_equality_preserves_both_sum_three_assignments() {
        for algorithm in [PbEncoding::PindakaasBdd, PbEncoding::PindakaasSwc] {
            let mut instance: SatInstance = SatInstance::new();
            let inputs: Vec<_> = (0..4).map(|_| instance.new_lit()).collect();
            for pair in inputs.chunks_exact(2) {
                instance.add_clause([pair[0], pair[1]].into());
                instance.add_clause([!pair[0], !pair[1]].into());
            }
            let mut variables = HashMap::new();
            let mut cache = EncodingCache::default();
            Compiler {
                instance: &mut instance,
                variables: &mut variables,
                counters: Some(&mut cache),
            }
            .pseudo_boolean(
                algorithm,
                CardinalityRelation::Exactly,
                3,
                inputs
                    .iter()
                    .zip([1, 2, 1, 2])
                    .map(|(&literal, weight)| (weight, Term::Literal(literal)))
                    .collect(),
                &[
                    PbTermGroup {
                        start: 0,
                        end: 2,
                        structure: PbTermStructure::Choice,
                    },
                    PbTermGroup {
                        start: 2,
                        end: 4,
                        structure: PbTermStructure::Choice,
                    },
                ],
            )
            .unwrap();
            let (cnf, _): (Cnf, BasicVarManager) = instance.into_cnf();
            let mut solver = SatSolver::default();
            solver.add_cnf(cnf).unwrap();
            for assignment in 0..16 {
                let assumptions: Vec<_> = inputs
                    .iter()
                    .enumerate()
                    .map(|(index, literal)| {
                        if assignment & (1 << index) != 0 {
                            *literal
                        } else {
                            !*literal
                        }
                    })
                    .collect();
                assert_eq!(
                    solver.solve_assumps(&assumptions).unwrap() == SolverResult::Sat,
                    matches!(assignment, 6 | 9),
                    "{algorithm:?}, assignment={assignment}"
                );
            }
        }
    }

    #[test]
    fn weighted_thresholds_reuse_batches_and_coexist_for_every_provider() {
        use crate::ast::sat_decision::IntegerRelation;
        for algorithm in PbEncoding::ALL {
            let mut cache = EncodingCache::default();
            let mut variables = HashMap::new();
            let mut solver = SatSolver::default();
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
            let mut solver = SatSolver::default();
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
        let mut solver = SatSolver::default();
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
            let mut solver = SatSolver::default();
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
                                let mut solver = SatSolver::default();
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
                let mut solver = SatSolver::default();
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
                            let mut solver = SatSolver::default();
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
