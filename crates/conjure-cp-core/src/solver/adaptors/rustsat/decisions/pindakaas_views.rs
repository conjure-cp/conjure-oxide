//! Native Pindakaas integer views over Boolean representation operands.
use super::*;
use crate::ast::sat_decision::{PbTermGroup, PbTermStructure};
use pindakaas::{BoolVal, constraint::linear::LinExp, decision::integer::IntVar};
use std::collections::BTreeMap;

struct BinaryView {
    variable: IntVar,
    factor: i64,
    constant: i128,
}

/// Narrow contributions after the caller's coefficient-sum and nontrivial-bound checks.
fn library_weight(value: i128) -> i64 {
    i64::try_from(value).expect("normalised PB contribution exceeds the validated library range")
}

impl Compiler<'_> {
    /// Preserve applicable representation groups, leaving other contributions as Boolean terms.
    pub(super) fn structured_pb_expression(
        &mut self,
        mut bound: i128,
        mut coefficients: BTreeMap<Lit, i128>,
        terms: &[(i64, Term)],
        groups: &[PbTermGroup],
    ) -> Result<(LinExp, i64), pindakaas::Unsatisfiable> {
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
                    // Choice/chain guarantees refer to these original polarities.
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
                PbTermStructure::Choice => (self.choice_view(&inputs, &weights)?, 1),
                PbTermStructure::Chain => (self.chain_view(&inputs, &weights)?, 1),
                PbTermStructure::BoundedBinary { lower, upper } => {
                    // Another occurrence can change the original bound's meaning.
                    if inputs.iter().any(|literal| {
                        occurrences[literal] != 1 || !coefficients.contains_key(literal)
                    }) {
                        continue;
                    }
                    let Some(view) = self.binary_view(&inputs, &weights, lower, upper)? else {
                        continue;
                    };
                    bound -= view.constant;
                    (view.variable, view.factor)
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
            .map(|(literal, weight)| (pind_lit(literal), library_weight(weight)))
            .collect();
        expression += LinExp::from_terms(&free);
        Ok((expression, library_weight(bound)))
    }

    /// Merge equal-valued alternatives into the exactly-one indicators a direct view needs.
    fn direct_view(
        &mut self,
        values: BTreeMap<i128, Vec<Term>>,
    ) -> Result<IntVar, pindakaas::Unsatisfiable> {
        let walk = values
            .into_iter()
            .map(|(value, terms)| (library_weight(value), pind_term(self.combine(false, terms))))
            .collect::<Vec<_>>();
        let mut sink = PindakaasSink {
            instance: self.instance,
            guard: None,
        };
        IntVar::from_direct_walk(&mut sink, walk)
    }

    /// An AMO group also permits no selected input, contributing zero.
    fn choice_view(
        &mut self,
        inputs: &[Lit],
        weights: &[i128],
    ) -> Result<IntVar, pindakaas::Unsatisfiable> {
        let mut values = BTreeMap::<i128, Vec<Term>>::new();
        let selected = self.combine(
            false,
            inputs
                .iter()
                .map(|literal| Term::Literal(*literal))
                .collect(),
        );
        values.entry(0).or_default().push(selected.negated());
        for (&literal, &weight) in inputs.iter().zip(weights) {
            values
                .entry(weight)
                .or_default()
                .push(Term::Literal(literal));
        }
        self.direct_view(values)
    }

    /// Monotone prefix sums use order views; mixed signed steps use direct boundary indicators.
    fn chain_view(
        &mut self,
        inputs: &[Lit],
        weights: &[i128],
    ) -> Result<IntVar, pindakaas::Unsatisfiable> {
        let walk = if weights.iter().all(|weight| *weight >= 0) {
            let mut sum = 0;
            let mut walk = vec![(0, BoolVal::Const(true))];
            for (&literal, &weight) in inputs.iter().zip(weights) {
                sum += weight;
                if weight != 0 {
                    walk.push((library_weight(sum), pind_lit(literal).into()));
                }
            }
            Some(walk)
        } else if weights.iter().all(|weight| *weight <= 0) {
            let mut sum: i128 = weights.iter().sum();
            let mut walk = vec![(library_weight(sum), BoolVal::Const(true))];
            for (&literal, &weight) in inputs.iter().zip(weights).rev() {
                sum -= weight;
                if weight != 0 {
                    walk.push((library_weight(sum), pind_lit(!literal).into()));
                }
            }
            Some(walk)
        } else {
            None
        };
        if let Some(walk) = walk {
            let mut sink = PindakaasSink {
                instance: self.instance,
                guard: None,
            };
            return IntVar::from_order_walk(&mut sink, walk);
        }
        // Mixed signed steps can revisit values rather than increasing monotonically.
        let mut values = BTreeMap::<i128, Vec<Term>>::new();
        values.entry(0).or_default().push(Term::Literal(!inputs[0]));
        let mut sum = 0;
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
            values.entry(sum).or_default().push(selected);
        }
        self.direct_view(values)
    }

    /// Supply scaled binary bits and enforce their bounds before domain simplification.
    fn binary_view(
        &mut self,
        inputs: &[Lit],
        weights: &[i128],
        lower: i64,
        upper: i64,
    ) -> Result<Option<BinaryView>, pindakaas::Unsatisfiable> {
        let factor = weights[0].abs();
        if factor == 0
            || weights.iter().enumerate().any(|(index, weight)| {
                1i128
                    .checked_shl(index as u32)
                    .and_then(|power| factor.checked_mul(power))
                    != Some(weight.abs())
            })
        {
            return Ok(None);
        }
        let constant: i128 = weights.iter().copied().filter(|weight| *weight < 0).sum();
        let lower = i128::from(lower) - constant;
        let upper = i128::from(upper) - constant;
        let capacity: i128 = weights.iter().map(|weight| weight.abs()).sum();
        if lower < 0 || upper > capacity || lower % factor != 0 || upper % factor != 0 {
            return Ok(None);
        }
        let upper = library_weight(upper / factor);
        let width = (i64::BITS - upper.leading_zeros()) as usize;
        let bits: Vec<_> = inputs
            .iter()
            .zip(weights)
            .take(width)
            .map(|(&literal, &weight)| pind_lit(if weight < 0 { !literal } else { literal }).into())
            .collect();
        // The hint may describe the domain constraint itself. Constrain the supplied
        // bits independently, including unsigned bits trimmed above the domain's width.
        for (&literal, &weight) in inputs.iter().zip(weights).skip(width) {
            self.assert(Term::Literal(if weight < 0 { literal } else { !literal }));
        }
        let mut sink = PindakaasSink {
            instance: self.instance,
            guard: None,
        };
        let variable = IntVar::from_binary_encoding(
            &mut sink,
            library_weight(lower / factor)..=upper,
            &bits,
            0,
        )?;
        variable.constrain(&mut sink)?;
        Ok(Some(BinaryView {
            variable,
            factor: library_weight(factor),
            constant,
        }))
    }
}
