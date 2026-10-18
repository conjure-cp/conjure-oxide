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
