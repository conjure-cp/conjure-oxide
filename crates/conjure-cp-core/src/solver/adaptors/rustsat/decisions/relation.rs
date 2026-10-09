//! Equality, choice and integer relations over integer views.
use super::*;

impl Compiler<'_> {
    pub(super) fn view_equality(
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

    /// Preserve one-hot numeric choices instead of constructing a weighted counter.
    pub(super) fn choice_relation(
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

    pub(super) fn equate(&mut self, output: Term, value: Term) {
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

    pub(super) fn integer_relation(
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
        let (bound, coefficients) = canonical_pb_terms(bound, terms);
        let (bound, positive, total) = positive_pb_terms(bound, coefficients);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::sat_decision::{
        EncodingSelection, PbEncoding, PbTermGroup, PbTermStructure, SelectionProvenance,
    };
    use crate::ast::{DeclarationPtr, Domain, Metadata, Moo, Reference};
    use crate::solver::adaptors::rustsat::adaptor::SatSolver;
    use rustsat::instances::{BasicVarManager, ManageVars};
    use rustsat::solvers::{Solve, SolveIncremental, SolverResult};

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
                        let mut solver = SatSolver::default();
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
                        let mut solver = SatSolver::default();
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
                            let mut solver = SatSolver::default();
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
                                        let mut solver = SatSolver::default();
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
            let mut solver = SatSolver::default();
            solver.add_cnf(cnf).unwrap();
            // The choice maximum makes <= 9 true and its opposite impossible.
            assert_eq!(solver.solve_assumps(&[output]).unwrap(), SolverResult::Sat);
            assert_eq!(
                solver.solve_assumps(&[!output]).unwrap(),
                SolverResult::Unsat
            );
        }
    }
}
