//! allDifferent and allDifferentExcept constraints.
use super::*;

impl Compiler<'_> {
    pub(super) fn alldifferent(
        &mut self,
        output: Term,
        inputs: &[crate::ast::sat_decision::SatIntegerView],
        except: Option<&crate::ast::sat_decision::SatIntegerView>,
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
                    if except
                        .is_some_and(|except| except.terms.is_empty() && except.constant == *value)
                    {
                        continue;
                    }
                    by_value
                        .entry(*value)
                        .or_default()
                        .push(self.encode(expression)?);
                }
            }
            for (value, terms) in by_value.into_iter().filter(|(_, terms)| terms.len() >= 2) {
                let exempt = if let Some(except) = except {
                    let pb = pb_encoding
                        .as_ref()
                        .ok_or_else(|| {
                            SolverError::ModelInvalid(
                                "Unresolved allDifferent PB encoding decision".into(),
                            )
                        })?
                        .algorithm;
                    let value = crate::ast::sat_decision::SatIntegerView {
                        constant: value,
                        terms: vec![],
                        groups: vec![],
                        choices: None,
                    };
                    self.view_equality(except, &value, pb)?
                } else {
                    Term::Constant(false)
                };
                if matches!(output, Term::Constant(true)) {
                    match exempt {
                        Term::Constant(true) => (),
                        Term::Constant(false) => self.asserted_amo(amo, terms)?,
                        Term::Literal(literal) => {
                            self.asserted_amo_guarded(amo, terms, Some(!literal))?
                        }
                    }
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
                    let truth = self.combine(false, vec![exempt, Term::Literal(truth)]);
                    truths.push(truth);
                }
            }
        } else if let Some(exception) = except {
            let pb = pb_encoding
                .as_ref()
                .ok_or_else(|| {
                    SolverError::ModelInvalid("Unresolved allDifferent PB encoding decision".into())
                })?
                .algorithm;
            for (index, left) in inputs.iter().enumerate() {
                let exempt = self.view_equality(left, exception, pb)?;
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
                            let mut solver = SatSolver::default();
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
}
