//! One-hot representation of an integer, for the SAT backend.
//!
//! `x` becomes one Boolean variable per value of its domain's span: bit `i` is true exactly when
//! `x` takes the `i`th value. Equality and membership become a single literal, at the cost of a
//! variable per value -- linear in the domain size, against
//! [`IntLog`](super::super::IntLog)'s logarithmic.

use crate::shared::representation_prelude::*;
use crate::types::int::{finite_int_bounds, int_domain_to_expr, int_ranges};
use conjure_cp::ast::{Domain, Moo, Reference, SATIntEncoding};
use conjure_cp::into_matrix_expr;
use conjure_cp::settings::SolverFamily;
use std::collections::VecDeque;

register_representation!(
    IntDirect("int_direct")
    struct State<T> {
        /// Overall lower and upper bound of the represented domain.
        pub bounds: (i32, i32),
        /// The domain's inclusive intervals, kept so the structural constraint can rule out gaps.
        pub ranges: Vec<(i32, i32)>,
        /// One bit per value in `bounds`, in ascending order of value.
        pub bits: Moo<Vec<T>>
    }
    impl State<DeclarationPtr> {
        /// This variable as a directly encoded `SATInt`, the form the SAT rules operate on.
        pub fn sat_int_expr(&self) -> Expression {
            let bits: Vec<Expression> = self
                .bits
                .iter()
                .map(|declaration| Reference::new(declaration.clone()).into())
                .collect();
            Expression::SATInt(
                Metadata::new(),
                SATIntEncoding::Direct,
                Moo::new(into_matrix_expr!(bits)),
                self.bounds,
            )
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |msg: &str| ReprInitError::UnsupportedDomain(
            dom.clone(),
            IntDirect::NAME,
            String::from(msg),
        );

        let ranges = int_ranges(&dom)
            .ok_or_else(|| domain_err("expected a finite ground integer domain"))?;
        let bounds @ (low, high) = finite_int_bounds(&ranges)
            .ok_or_else(|| domain_err("expected a non-empty integer domain"))?;

        let width = usize::try_from(i64::from(high) - i64::from(low) + 1)
            .map_err(|_| domain_err("integer domain is too large for a one-hot encoding"))?;
        let bits = Moo::new(std::iter::repeat_n(Domain::bool(), width).collect());
        Ok(State { bounds, ranges, bits })
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        let bits: Vec<Expression> = state
            .bits
            .iter()
            .map(|declaration| Reference::new(declaration.clone()).into())
            .collect();

        // At most one value is taken. Pairwise, matching what the encoding's operation rules
        // assume when they read a bit as "x equals this value".
        let mut constraints = Vec::new();
        for (index, bit) in bits.iter().enumerate() {
            for other in &bits[index + 1..] {
                let bit = Expression::Not(Metadata::new(), Moo::new(bit.clone()));
                let other = Expression::Not(Metadata::new(), Moo::new(other.clone()));
                constraints.push(Expression::Or(
                    Metadata::new(),
                    Moo::new(into_matrix_expr!(vec![bit, other])),
                ));
            }
        }

        // ...and the value taken is one the domain allows, which also forces at least one bit.
        constraints.push(int_domain_to_expr(state.sat_int_expr(), &state.ranges));
        constraints
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::Int(value) = value else {
            return Err(ReprDownError::BadValue(value, String::from("expected an integer")));
        };

        let (low, high) = state.bounds;
        if value < low || value > high {
            return Err(ReprDownError::BadValue(
                Literal::Int(value),
                format!("expected a value between {low} and {high}"),
            ));
        }

        let bits = (low..=high).map(|candidate| Literal::Bool(candidate == value)).collect();
        Ok(State {
            bounds: state.bounds,
            ranges: state.ranges.clone(),
            bits: Moo::new(bits),
        })
    }
    fn up(state: State<Literal>) -> Literal {
        let (low, _) = state.bounds;
        let mut found: Option<i32> = None;
        for (index, bit) in state.bits.iter().enumerate() {
            let set = match bit {
                Literal::Bool(set) => *set,
                Literal::Int(0) => false,
                Literal::Int(1) => true,
                other => bug!("expected a Boolean occurrence value, got {other}"),
            };
            if set {
                let value = low + index as i32;
                if let Some(previous) = found {
                    bug!("one-hot encoding has both {previous} and {value} set");
                }
                found = Some(value);
            }
        }
        Literal::Int(found.unwrap_or_else(|| bug!("one-hot encoding has no value set")))
    }
    fn repr_vars(state: &State<DeclarationPtr>) -> VecDeque<DeclarationPtr> {
        state.bits.iter().cloned().collect()
    }
    fn compactness(state: &State<DomainPtr>) -> usize {
        1usize.checked_shl(state.bits.len() as u32).unwrap_or(usize::MAX)
    }
    fn applies(family: SolverFamily) -> bool {
        matches!(family, SolverFamily::Sat)
    }
);
