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
