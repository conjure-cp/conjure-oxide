//! Order (unary) representation of an integer, for the SAT backend.
//!
//! `x` becomes one Boolean variable per value of its domain's span, where bit `i` means
//! `x >= low + i`. The bits are monotonically decreasing, so comparisons are a single literal --
//! the mirror image of [`IntDirect`](super::super::IntDirect), which makes equality cheap instead.

use crate::shared::representation_prelude::*;
use crate::types::int::{finite_int_bounds, int_domain_to_expr, int_ranges};
use conjure_cp::ast::{Domain, Moo, Reference, SATIntEncoding};
use conjure_cp::into_matrix_expr;
use conjure_cp::settings::SolverFamily;
use std::collections::VecDeque;

register_representation!(
    IntOrder("int_order")
    struct State<T> {
        /// Overall lower and upper bound of the represented domain.
        pub bounds: (i32, i32),
        /// The domain's inclusive intervals, kept so the structural constraint can rule out gaps.
        pub ranges: Vec<(i32, i32)>,
        /// Bit `i` holds `x >= low + i`; the first is therefore always true.
        pub bits: Moo<Vec<T>>
    }
    impl State<DeclarationPtr> {
        /// This variable as an order-encoded `SATInt`, the form the SAT rules operate on.
        pub fn sat_int_expr(&self) -> Expression {
            let bits: Vec<Expression> = self
                .bits
                .iter()
                .map(|declaration| Reference::new(declaration.clone()).into())
                .collect();
            Expression::SATInt(
                Metadata::new(),
                SATIntEncoding::Order,
                Moo::new(into_matrix_expr!(bits)),
                self.bounds,
            )
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |msg: &str| ReprInitError::UnsupportedDomain(
            dom.clone(),
            IntOrder::NAME,
            String::from(msg),
        );

        let ranges = int_ranges(&dom)
            .ok_or_else(|| domain_err("expected a finite ground integer domain"))?;
        let bounds @ (low, high) = finite_int_bounds(&ranges)
            .ok_or_else(|| domain_err("expected a non-empty integer domain"))?;

        let width = usize::try_from(i64::from(high) - i64::from(low) + 1)
            .map_err(|_| domain_err("integer domain is too large for an order encoding"))?;
        let bits = Moo::new(std::iter::repeat_n(Domain::bool(), width).collect());
        Ok(State { bounds, ranges, bits })
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        let bits: Vec<Expression> = state
            .bits
            .iter()
            .map(|declaration| Reference::new(declaration.clone()).into())
            .collect();

        let mut constraints = Vec::new();

        // `x >= low` always holds, which anchors the chain.
        if let Some(first) = bits.first() {
            constraints.push(first.clone());
        }

        // `x >= v` implies `x >= v - 1`, so the bits never turn back on once they turn off.
        for window in bits.windows(2) {
            let [previous, next] = window else { continue };
            let next = Expression::Not(Metadata::new(), Moo::new(next.clone()));
            constraints.push(Expression::Or(
                Metadata::new(),
                Moo::new(into_matrix_expr!(vec![next, previous.clone()])),
            ));
        }

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

        let bits = (low..=high).map(|threshold| Literal::Bool(value >= threshold)).collect();
        Ok(State {
            bounds: state.bounds,
            ranges: state.ranges.clone(),
            bits: Moo::new(bits),
        })
    }
    fn up(state: State<Literal>) -> Literal {
        let (low, high) = state.bounds;
        let mut value = high;
        let mut seen_false = false;
        for (index, bit) in state.bits.iter().enumerate() {
            let set = match bit {
                Literal::Bool(set) => *set,
                Literal::Int(0) => false,
                Literal::Int(1) => true,
                other => bug!("expected a Boolean threshold value, got {other}"),
            };
            match (set, seen_false) {
                // The first threshold that fails puts the value one below it.
                (false, false) => {
                    seen_false = true;
                    value = low + index as i32 - 1;
                }
                (true, true) => bug!("order encoding is not monotone at index {index}"),
                _ => {}
            }
        }
        Literal::Int(value)
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
