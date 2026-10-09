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
                    // Domain constraints can themselves be the source of this hint.
                    // Enforce it independently before the native view folds fixed values.
                    // Bits above the supplied width must be zero in the unsigned view.
                    for (&literal, &weight) in inputs.iter().zip(&weights).skip(width) {
                        self.assert(Term::Literal(if weight < 0 { literal } else { !literal }));
                    }
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
                    view.constrain(&mut sink)?;
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
