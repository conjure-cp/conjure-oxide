//! Preserve linear constraints and numeric relations as semantic library decisions.
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
        match expression {
            Expr::Atomic(_, Atom::Literal(Literal::Int(value))) => {
                self.constant = self
                    .constant
                    .checked_add(scale.checked_mul(i128::from(*value))?)?
            }
            Expr::Atomic(_, Atom::Reference(reference)) => {
                use crate::types::int::{
                    IntDirect, IntLog, IntOffset, IntOrder, IntRank, IntSignMagnitude,
                };
                let represented = if let Some(state) = reference.get_repr_as::<IntDirect>() {
                    state.sat_int_expr()
                } else if let Some(state) = reference.get_repr_as::<IntOrder>() {
                    state.sat_int_expr()
                } else if let Some(state) = reference.get_repr_as::<IntOffset>() {
                    state.sat_int_expr()
                } else if let Some(state) = reference.get_repr_as::<IntRank>() {
                    state.sat_int_expr()
                } else if let Some(state) = reference.get_repr_as::<IntSignMagnitude>() {
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
                let bits = inner.unwrap_list_cow()?;
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
                    SATIntEncoding::Offset | SATIntEncoding::Rank(_) => {
                        if let SATIntEncoding::Rank(ranges) = encoding
                            && ranges.as_slice() != [(*low, *high)]
                        {
                            return None;
                        }
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
                    SATIntEncoding::SignMagnitude => {
                        if bits.len() < 2 || bits.len() > 33 {
                            return None;
                        }
                        let sign = bits.last()?;
                        for (index, magnitude) in bits[..bits.len() - 1].iter().enumerate() {
                            let weight = scale.checked_mul(1i128 << index)?;
                            self.terms.push((weight, magnitude.clone()));
                            // x = magnitude - 2*(sign AND magnitude). The compiler
                            // reuses RustSAT gates and the selected library PB encoder.
                            let negative = match (sign, magnitude) {
                                (Expr::Atomic(_, Atom::Literal(Literal::Bool(false))), _)
                                | (_, Expr::Atomic(_, Atom::Literal(Literal::Bool(false)))) => {
                                    false.into()
                                }
                                (Expr::Atomic(_, Atom::Literal(Literal::Bool(true))), _) => {
                                    magnitude.clone()
                                }
                                (_, Expr::Atomic(_, Atom::Literal(Literal::Bool(true)))) => {
                                    sign.clone()
                                }
                                _ => Expr::And(
                                    Metadata::new(),
                                    conjure_cp::ast::Moo::new(conjure_cp::into_matrix_expr!(vec![
                                        sign.clone(),
                                        magnitude.clone()
                                    ])),
                                ),
                            };
                            self.terms.push((weight.checked_mul(-2)?, negative));
                        }
                        // The conjunctions depend on the magnitude bits: this is
                        // not an independent bounded-binary input group.
                        None
                    }
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
/// Extract an actual-value view without folding representation bounds to constants.
pub(super) fn integer_view(
    expression: &Expr,
) -> Option<conjure_cp::ast::sat_decision::SatIntegerView> {
    use conjure_cp::ast::{Moo, sat_decision::SatIntegerView};
    if expression
        .domain_of()
        .is_some_and(|domain| domain.is_bool())
        && crate::shared::utils::is_literal(expression)
    {
        return Some(SatIntegerView {
            constant: 0,
            terms: vec![(1, expression.clone())],
            groups: vec![],
            choices: Some(vec![
                (0, Expr::Not(Metadata::new(), Moo::new(expression.clone()))),
                (1, expression.clone()),
            ]),
        });
    }
    let mut linear = Linear::default();
    linear.add(expression, 1)?;
    let constant = i64::try_from(linear.constant).ok()?;
    let terms = linear
        .terms
        .into_iter()
        .map(|(weight, input)| i64::try_from(weight).ok().map(|weight| (weight, input)))
        .collect::<Option<Vec<_>>>()?;
    // Literal code bits establish the actual value; singleton bounds alone do not.
    if terms
        .iter()
        .all(|(_, input)| matches!(input, Expr::Atomic(_, Atom::Literal(Literal::Bool(_)))))
    {
        let value = terms
            .iter()
            .try_fold(i128::from(constant), |value, (weight, input)| {
                let Expr::Atomic(_, Atom::Literal(Literal::Bool(bit))) = input else {
                    unreachable!()
                };
                value.checked_add(i128::from(*weight) * i128::from(*bit))
            })
            .and_then(|value| i64::try_from(value).ok())?;
        return Some(SatIntegerView {
            constant: value,
            terms: vec![],
            groups: vec![],
            choices: Some(vec![(value, true.into())]),
        });
    }
    let choices = if terms.is_empty() {
        Some(vec![(constant, true.into())])
    } else if linear.groups.len() == 1
        && linear.groups[0].start == 0
        && linear.groups[0].end == terms.len()
        && linear.groups[0].structure == PbTermStructure::Choice
    {
        Some(
            terms
                .iter()
                .map(|(weight, input)| {
                    constant
                        .checked_add(*weight)
                        .map(|value| (value, input.clone()))
                })
                .collect::<Option<Vec<_>>>()?,
        )
    } else {
        None
    };
    Some(SatIntegerView {
        constant,
        terms,
        groups: linear.groups,
        choices,
    })
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
        Expr::SATInt(..) => {
            // Inspect literal code bits: singleton bounds alone do not make a value constant.
            let view = integer_view(expression)?;
            view.terms
                .is_empty()
                .then(|| i32::try_from(view.constant).ok())
                .flatten()
        }
        _ => None,
    }
}
fn has_linear_operation(expression: &Expr) -> bool {
    matches!(
        expression,
        Expr::Sum(..) | Expr::Product(..) | Expr::Minus(..) | Expr::Neg(..) | Expr::ToInt(..)
    )
}

/// Preserve products of Boolean indicators as an indicator of their conjunction.
#[register_rule("SAT", 18900, [Product])]
fn product_boolean_indicators(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Product(_, factors) = expr else {
        return Err(RuleNotApplicable);
    };
    let inputs = super::boolean::count_inputs(factors).ok_or(RuleNotApplicable)?;
    Ok(RuleEffect::pure(Expr::ToInt(
        Metadata::new(),
        conjure_cp::ast::Moo::new(Expr::And(
            Metadata::new(),
            conjure_cp::ast::Moo::new(conjure_cp::into_matrix_expr!(inputs)),
        )),
    )))
}

/// Extract ready, asserted linear comparisons before integer circuits consume them.
#[register_rule("SAT", 19000, [Root])]
fn select_pseudo_boolean(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    super::asserted::select_asserted(expr, symbols, |child| {
        let (left, right, relation, strict) = match child {
            Expr::Leq(_, left, right) => (left, right, CardinalityRelation::AtMost, false),
            Expr::Lt(_, left, right) => (left, right, CardinalityRelation::AtMost, true),
            Expr::Geq(_, left, right) => (left, right, CardinalityRelation::AtLeast, false),
            Expr::Gt(_, left, right) => (left, right, CardinalityRelation::AtLeast, true),
            Expr::Eq(_, left, right) => (left, right, CardinalityRelation::Exactly, false),
            _ => return None,
        };
        if !has_linear_operation(left) && !has_linear_operation(right) {
            return None;
        }
        let mut linear = Linear::default();
        linear.add(left, 1)?;
        linear.add(right, -1)?;
        let mut bound = linear.constant.checked_neg()?;
        if strict {
            bound = bound.checked_add(if relation == CardinalityRelation::AtMost {
                -1
            } else {
                1
            })?;
        }
        let bound = i64::try_from(bound).ok()?;
        let terms = linear
            .terms
            .into_iter()
            .map(|(weight, input)| i64::try_from(weight).map(|weight| (weight, input)))
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        Some(SatEncodingDecision::PseudoBoolean {
            terms,
            groups: linear.groups,
            relation,
            bound,
            encoding: None,
        })
    })
}
fn ready_count_comparison(expr: &Expr) -> bool {
    let (left, right) = match expr {
        Expr::Eq(_, left, right)
        | Expr::Neq(_, left, right)
        | Expr::Lt(_, left, right)
        | Expr::Leq(_, left, right)
        | Expr::Gt(_, left, right)
        | Expr::Geq(_, left, right) => (left, right),
        _ => return false,
    };
    let count = |expression: &Expr| {
        matches!(expression, Expr::Sum(_, inputs)
        if super::boolean::count_inputs(inputs).is_some())
    };
    (count(left) && constant_int(right).is_some()) || (count(right) && constant_int(left).is_some())
}

/// Reify Boolean counts within ready Boolean contexts using cardinality and AMO providers.
#[register_rule("SAT", 18600, [Root])]
fn select_guarded_count(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    if !matches!(expr, Expr::Root(..)) {
        return Err(RuleNotApplicable);
    }
    guarded_count(expr, true, symbols).ok_or(RuleNotApplicable)
}

fn guarded_count(expr: &Expr, asserted: bool, symbols: &SymbolTable) -> Option<RuleEffect> {
    if !asserted && ready_count_comparison(expr) {
        return count_relation(expr, symbols).ok();
    }
    // Only traverse Boolean contexts and literal matrices, never a binder or undefined value.
    match expr {
        Expr::Root(_, entries) => guarded_children(expr, entries.iter(), true, symbols),
        Expr::AbstractLiteral(_, Matrix(entries, _)) => {
            guarded_children(expr, entries.iter(), asserted, symbols)
        }
        Expr::And(_, entries)
            if matches!(entries.as_ref(), Expr::AbstractLiteral(_, Matrix(..))) =>
        {
            guarded_children(expr, std::iter::once(entries.as_ref()), asserted, symbols)
        }
        Expr::Or(_, entries)
            if matches!(entries.as_ref(), Expr::AbstractLiteral(_, Matrix(..))) =>
        {
            guarded_children(expr, std::iter::once(entries.as_ref()), false, symbols)
        }
        Expr::Not(_, child) | Expr::ToInt(_, child) => {
            guarded_children(expr, std::iter::once(child.as_ref()), false, symbols)
        }
        Expr::Imply(_, left, right)
        | Expr::Iff(_, left, right)
        | Expr::Eq(_, left, right)
        | Expr::Neq(_, left, right)
        | Expr::Lt(_, left, right)
        | Expr::Leq(_, left, right)
        | Expr::Gt(_, left, right)
        | Expr::Geq(_, left, right) => guarded_children(
            expr,
            [left.as_ref(), right.as_ref()].into_iter(),
            false,
            symbols,
        ),
        _ => None,
    }
}

fn guarded_children<'a>(
    expr: &Expr,
    children: impl Iterator<Item = &'a Expr>,
    asserted: bool,
    symbols: &SymbolTable,
) -> Option<RuleEffect> {
    use uniplate::Uniplate;
    for (index, child) in children.enumerate() {
        if let Some(mut effect) = guarded_count(child, asserted, symbols) {
            let mut replacements = expr.children();
            replacements[index] = effect.new_expression;
            effect.new_expression = expr.with_children(replacements);
            return Some(effect);
        }
    }
    None
}

/// Preserve numeric equality and comparison, including nested Boolean uses, for library encoding.
#[register_rule("SAT", 18500, [Eq, Neq, Lt, Leq, Gt, Geq])]
fn select_integer_relation(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    // Root chooses asserted bounds; disequality and guarded counts require equivalence decisions.
    if ready_count_comparison(expr) && !matches!(expr, Expr::Neq(..)) {
        return Err(RuleNotApplicable);
    }
    if ready_count_comparison(expr) {
        return count_relation(expr, symbols);
    }
    integer_relation(expr, symbols)
}

fn count_relation(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    use conjure_cp::ast::sat_decision::IntegerRelation;
    let (left, right, mut relation) = match expr {
        Expr::Eq(_, left, right) => (left, right, IntegerRelation::Equal),
        Expr::Neq(_, left, right) => (left, right, IntegerRelation::NotEqual),
        Expr::Lt(_, left, right) => (left, right, IntegerRelation::Less),
        Expr::Leq(_, left, right) => (left, right, IntegerRelation::LessEqual),
        Expr::Gt(_, left, right) => (left, right, IntegerRelation::Greater),
        Expr::Geq(_, left, right) => (left, right, IntegerRelation::GreaterEqual),
        _ => return Err(RuleNotApplicable),
    };
    let (sum, bound) = if let Expr::Sum(_, sum) = left.as_ref() {
        (sum, constant_int(right).ok_or(RuleNotApplicable)?)
    } else if let Expr::Sum(_, sum) = right.as_ref() {
        relation = match relation {
            IntegerRelation::Less => IntegerRelation::Greater,
            IntegerRelation::LessEqual => IntegerRelation::GreaterEqual,
            IntegerRelation::Greater => IntegerRelation::Less,
            IntegerRelation::GreaterEqual => IntegerRelation::LessEqual,
            other => other,
        };
        (sum, constant_int(left).ok_or(RuleNotApplicable)?)
    } else {
        return Err(RuleNotApplicable);
    };
    let inputs = super::boolean::count_inputs(sum).ok_or(RuleNotApplicable)?;
    let bound = i64::from(bound);
    let mut symbols = symbols.clone();
    let output = super::boolean::create_bool_aux(&mut symbols);
    Ok(RuleEffect::sat(
        output.clone(),
        vec![SatEncodingDecision::CountRelation {
            output,
            inputs,
            relation,
            bound,
            encoding: None,
            amo_encoding: None,
        }],
        symbols,
    ))
}

fn integer_relation(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    use conjure_cp::ast::sat_decision::IntegerRelation;
    let (left, right, relation) = match expr {
        Expr::Eq(_, left, right) => (left, right, IntegerRelation::Equal),
        Expr::Neq(_, left, right) => (left, right, IntegerRelation::NotEqual),
        Expr::Lt(_, left, right) => (left, right, IntegerRelation::Less),
        Expr::Leq(_, left, right) => (left, right, IntegerRelation::LessEqual),
        Expr::Gt(_, left, right) => (left, right, IntegerRelation::Greater),
        Expr::Geq(_, left, right) => (left, right, IntegerRelation::GreaterEqual),
        _ => return Err(RuleNotApplicable),
    };
    // Boolean equality stays in the Boolean family. Ready SATInt views are already numeric.
    if !matches!(left.as_ref(), Expr::SATInt(..))
        && !left.domain_of().is_some_and(|domain| domain.is_int())
    {
        return Err(RuleNotApplicable);
    }
    let mut linear = Linear::default();
    linear.add(left, 1).ok_or(RuleNotApplicable)?;
    linear.add(right, -1).ok_or(RuleNotApplicable)?;
    let bound = linear
        .constant
        .checked_neg()
        .and_then(|n| i64::try_from(n).ok())
        .ok_or(RuleNotApplicable)?;
    let terms = linear
        .terms
        .into_iter()
        .map(|(weight, input)| i64::try_from(weight).map(|weight| (weight, input)))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| RuleNotApplicable)?;
    let mut symbols = symbols.clone();
    let output = super::boolean::create_bool_aux(&mut symbols);
    Ok(RuleEffect::sat(
        output.clone(),
        vec![SatEncodingDecision::IntegerRelation {
            output,
            terms,
            groups: linear.groups,
            relation,
            bound,
            encoding: None,
        }],
        symbols,
    ))
}

/// Preserve the objective's actual-value view and shared PB choice for solve-time tightening.
#[register_rule("SAT", 18400, [SatObjective])]
fn select_sat_objective(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::SatObjective(_, minimise, value) = expr else {
        return Err(RuleNotApplicable);
    };
    let value = integer_view(value).ok_or(RuleNotApplicable)?;
    Ok(RuleEffect::sat(
        true.into(),
        vec![SatEncodingDecision::Objective {
            minimise: *minimise,
            value,
            encoding: None,
        }],
        symbols.clone(),
    ))
}
