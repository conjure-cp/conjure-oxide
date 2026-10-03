//! Preserve asserted linear constraints as semantic weighted Boolean decisions.
use conjure_cp::ast::sat_decision::{CardinalityRelation, PbTermGroup, PbTermStructure};
use conjure_cp::ast::{
    AbstractLiteral::Matrix, Atom, Expression as Expr, Literal, Metadata, SATIntEncoding,
    SatEncodingDecision, SymbolTable,
};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

#[derive(Default)]
struct Linear {
    constant: i128,
    terms: Vec<(i128, Expr)>,
    groups: Vec<PbTermGroup>,
}
impl Linear {
    fn add(&mut self, expression: &Expr, scale: i128) -> Option<()> {
        if let Some(value) = constant_int(expression) {
            self.constant = self
                .constant
                .checked_add(scale.checked_mul(i128::from(value))?)?;
            return Some(());
        }
        match expression {
            Expr::Atomic(_, Atom::Literal(Literal::Int(value))) => {
                self.constant = self
                    .constant
                    .checked_add(scale.checked_mul(i128::from(*value))?)?
            }
            Expr::Atomic(_, Atom::Reference(reference)) => {
                use crate::types::int::{IntDirect, IntLog, IntOffset, IntOrder};
                let represented = if let Some(state) = reference.get_repr_as::<IntDirect>() {
                    state.sat_int_expr()
                } else if let Some(state) = reference.get_repr_as::<IntOrder>() {
                    state.sat_int_expr()
                } else if let Some(state) = reference.get_repr_as::<IntOffset>() {
                    state.sat_int_expr()
                } else {
                    reference.get_repr_as::<IntLog>()?.sat_int_expr()
                };
                self.add(&represented, scale)?;
            }
            Expr::Minus(_, left, right) => {
                self.add(left, scale)?;
                self.add(right, scale.checked_neg()?)?;
            }
            Expr::Neg(_, input) => self.add(input, scale.checked_neg()?)?,
            Expr::Sum(_, inputs) => {
                let Expr::AbstractLiteral(_, Matrix(inputs, _)) = inputs.as_ref() else {
                    return None;
                };
                for input in inputs {
                    self.add(input, scale)?;
                }
            }
            Expr::Product(_, inputs) => {
                let Expr::AbstractLiteral(_, Matrix(inputs, _)) = inputs.as_ref() else {
                    return None;
                };
                let mut multiplier = scale;
                let mut variable = None;
                for input in inputs {
                    if let Some(value) = constant_int(input) {
                        multiplier = multiplier.checked_mul(i128::from(value))?;
                    } else if variable.replace(input).is_some() {
                        return None;
                    }
                }
                if let Some(variable) = variable {
                    self.add(variable, multiplier)?;
                } else {
                    self.constant = self.constant.checked_add(multiplier)?;
                }
            }
            Expr::ToInt(_, input)
                if crate::shared::utils::is_literal(input)
                    && input.domain_of().is_some_and(|domain| domain.is_bool()) =>
            {
                self.terms.push((scale, input.as_ref().clone()))
            }
            Expr::SATInt(_, encoding, inner, (low, high)) => {
                let Expr::AbstractLiteral(_, Matrix(bits, _)) = inner.as_ref() else {
                    return None;
                };
                let start = self.terms.len();
                let structure = match encoding {
                    SATIntEncoding::Direct => {
                        if bits.len() as i128 != i128::from(*high) - i128::from(*low) + 1 {
                            return None;
                        }
                        // Direct and Order retain the domain span; separate structural
                        // constraints exclude sparse-domain gaps.
                        for (index, bit) in bits.iter().enumerate() {
                            self.terms.push((
                                scale.checked_mul(i128::from(*low) + index as i128)?,
                                bit.clone(),
                            ));
                        }
                        Some(PbTermStructure::Choice)
                    }
                    SATIntEncoding::Order => {
                        if bits.len() as i128 != i128::from(*high) - i128::from(*low) + 1 {
                            return None;
                        }
                        self.constant = self
                            .constant
                            .checked_add(scale.checked_mul(i128::from(*low))?)?;
                        for bit in bits.iter().skip(1) {
                            self.terms.push((scale, bit.clone()));
                        }
                        Some(PbTermStructure::Chain)
                    }
                    SATIntEncoding::Offset => {
                        if bits.is_empty() || bits.len() > 32 {
                            return None;
                        }
                        self.constant = self
                            .constant
                            .checked_add(scale.checked_mul(i128::from(*low))?)?;
                        for (index, bit) in bits.iter().enumerate() {
                            self.terms
                                .push((scale.checked_mul(1i128 << index)?, bit.clone()));
                        }
                        binary_structure(scale, 0, i128::from(*high) - i128::from(*low))
                    }
                    // Sparse rank is not a linear numeric view of its code bits.
                    SATIntEncoding::Rank(_) => return None,
                    SATIntEncoding::Log => {
                        if bits.is_empty() || bits.len() > 32 {
                            return None;
                        }
                        for (index, bit) in bits.iter().enumerate() {
                            let weight = scale.checked_mul(1i128 << index)?;
                            self.terms.push((
                                if index + 1 == bits.len() {
                                    weight.checked_neg()?
                                } else {
                                    weight
                                },
                                bit.clone(),
                            ));
                        }
                        binary_structure(scale, i128::from(*low), i128::from(*high))
                    }
                };
                if self.terms.len() > start
                    && let Some(structure) = structure
                {
                    self.groups.push(PbTermGroup {
                        start,
                        end: self.terms.len(),
                        structure,
                    });
                }
            }
            _ => return None,
        }
        Some(())
    }
}
fn binary_structure(scale: i128, low: i128, high: i128) -> Option<PbTermStructure> {
    let a = scale.checked_mul(low)?;
    let b = scale.checked_mul(high)?;
    Some(PbTermStructure::BoundedBinary {
        lower: i64::try_from(a.min(b)).ok()?,
        upper: i64::try_from(a.max(b)).ok()?,
    })
}
fn constant_int(expression: &Expr) -> Option<i32> {
    match expression {
        Expr::Atomic(_, Atom::Literal(Literal::Int(value))) => Some(*value),
        Expr::SATInt(_, _, _, (low, high)) if low == high => Some(*low),
        _ => None,
    }
}
fn has_linear_operation(expression: &Expr) -> bool {
    matches!(
        expression,
        Expr::Sum(..) | Expr::Product(..) | Expr::Minus(..) | Expr::Neg(..) | Expr::ToInt(..)
    )
}

/// Extract ready, asserted linear comparisons before integer circuits consume them.
#[register_rule("SAT", 19000, [Root])]
fn select_pseudo_boolean(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Root(_, children) = expr else {
        return Err(RuleNotApplicable);
    };
    for (index, child) in children.iter().enumerate() {
        let (left, right, relation, strict) = match child {
            Expr::Leq(_, left, right) => (left, right, CardinalityRelation::AtMost, false),
            Expr::Lt(_, left, right) => (left, right, CardinalityRelation::AtMost, true),
            Expr::Geq(_, left, right) => (left, right, CardinalityRelation::AtLeast, false),
            Expr::Gt(_, left, right) => (left, right, CardinalityRelation::AtLeast, true),
            Expr::Eq(_, left, right) => (left, right, CardinalityRelation::Exactly, false),
            _ => continue,
        };
        if !has_linear_operation(left) && !has_linear_operation(right) {
            continue;
        }
        let mut linear = Linear::default();
        if linear.add(left, 1).is_none() || linear.add(right, -1).is_none() {
            continue;
        }
        let Some(mut bound) = linear.constant.checked_neg() else {
            continue;
        };
        if strict {
            let Some(adjusted) = bound.checked_add(if relation == CardinalityRelation::AtMost {
                -1
            } else {
                1
            }) else {
                continue;
            };
            bound = adjusted;
        }
        let Ok(bound) = i64::try_from(bound) else {
            continue;
        };
        let terms = linear
            .terms
            .into_iter()
            .map(|(weight, input)| i64::try_from(weight).map(|weight| (weight, input)))
            .collect::<Result<Vec<_>, _>>();
        let Ok(terms) = terms else {
            continue;
        };
        let mut children = children.clone();
        children.remove(index);
        return Ok(RuleEffect::sat(
            Expr::Root(Metadata::new(), children),
            vec![SatEncodingDecision::PseudoBoolean {
                terms,
                groups: linear.groups,
                relation,
                bound,
                encoding: None,
            }],
            symbols.clone(),
        ));
    }
    Err(RuleNotApplicable)
}
#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{DeclarationPtr, Domain, Moo, Name, Reference};
    use conjure_cp::into_matrix_expr;
    #[test]
    fn integer_views_preserve_values_including_sparse_domain_gaps() {
        let low = -3;
        let high = 2;
        for encoding in [
            SATIntEncoding::Direct,
            SATIntEncoding::Order,
            SATIntEncoding::Log,
            SATIntEncoding::Offset,
        ] {
            let width = if matches!(encoding, SATIntEncoding::Log | SATIntEncoding::Offset) {
                3
            } else {
                6
            };
            let variables: Vec<Expr> = (0..width)
                .map(|index| {
                    Reference::new(DeclarationPtr::new_find(
                        Name::User(format!("v{index}").into()),
                        Domain::bool(),
                    ))
                    .into()
                })
                .collect();
            let represented = Expr::SATInt(
                Metadata::new(),
                encoding.clone(),
                Moo::new(into_matrix_expr!(variables.clone())),
                (low, high),
            );
            let mut linear = Linear::default();
            linear.add(&represented, -2).unwrap();
            assert_eq!(linear.groups.len(), 1);
            let group = &linear.groups[0];
            assert_eq!(group.start, 0);
            assert_eq!(group.end, linear.terms.len());
            assert_eq!(
                group.structure,
                match encoding {
                    SATIntEncoding::Direct => PbTermStructure::Choice,
                    SATIntEncoding::Order => PbTermStructure::Chain,
                    SATIntEncoding::Log => PbTermStructure::BoundedBinary {
                        lower: -4,
                        upper: 6
                    },
                    SATIntEncoding::Offset => PbTermStructure::BoundedBinary {
                        lower: -10,
                        upper: 0
                    },
                    SATIntEncoding::Rank(_) => unreachable!(),
                }
            );
            for value in [low, -1, high] {
                let values: Vec<_> = (0..width)
                    .map(|index| match encoding {
                        SATIntEncoding::Direct => value == low + index as i32,
                        SATIntEncoding::Order => value >= low + index as i32,
                        SATIntEncoding::Log => (value >> index) & 1 != 0,
                        SATIntEncoding::Offset => ((value - low) >> index) & 1 != 0,
                        SATIntEncoding::Rank(_) => unreachable!(),
                    })
                    .collect();
                let actual = linear.constant
                    + linear
                        .terms
                        .iter()
                        .map(|(weight, input)| {
                            let index = variables
                                .iter()
                                .position(|variable| variable == input)
                                .unwrap();
                            weight * i128::from(values[index])
                        })
                        .sum::<i128>();
                assert_eq!(
                    actual,
                    -2 * i128::from(value),
                    "{encoding:?}, value={value}"
                );
            }
        }
    }
    #[test]
    fn arithmetic_keeps_each_integer_group_and_free_boolean_occurrence() {
        let variables: Vec<Expr> = (0..4)
            .map(|index| {
                Reference::new(DeclarationPtr::new_find(
                    Name::User(format!("b{index}").into()),
                    Domain::bool(),
                ))
                .into()
            })
            .collect();
        let integer = |encoding| {
            Expr::SATInt(
                Metadata::new(),
                encoding,
                Moo::new(into_matrix_expr!(variables[..3].to_vec())),
                (0, 2),
            )
        };
        let expression = Expr::Sum(
            Metadata::new(),
            Moo::new(into_matrix_expr!(vec![
                integer(SATIntEncoding::Direct),
                Expr::Neg(Metadata::new(), Moo::new(integer(SATIntEncoding::Order))),
                Expr::ToInt(Metadata::new(), Moo::new(variables[3].clone())),
                integer(SATIntEncoding::Offset),
            ])),
        );
        let mut linear = Linear::default();
        linear.add(&expression, 2).unwrap();
        assert_eq!(
            linear.groups,
            vec![
                PbTermGroup {
                    start: 0,
                    end: 3,
                    structure: PbTermStructure::Choice
                },
                PbTermGroup {
                    start: 3,
                    end: 5,
                    structure: PbTermStructure::Chain
                },
                PbTermGroup {
                    start: 6,
                    end: 9,
                    structure: PbTermStructure::BoundedBinary { lower: 0, upper: 4 }
                },
            ]
        );
    }
    #[test]
    fn nonlinear_products_are_left_for_arithmetic_rules() {
        let variable: Expr = Reference::new(DeclarationPtr::new_find(
            Name::User("b".into()),
            Domain::bool(),
        ))
        .into();
        let integer = Expr::ToInt(Metadata::new(), Moo::new(variable));
        let product = Expr::Product(
            Metadata::new(),
            Moo::new(into_matrix_expr!(vec![integer.clone(), integer])),
        );
        assert!(Linear::default().add(&product, 1).is_none());
    }
}
