//! Two's-complement bit-vector representation of an integer, for the SAT backend.
//!
//! `x` becomes `n` Boolean variables holding `x` in two's complement, least significant bit
//! first. `n` is the smallest width that can hold every value of `x`'s domain, so the encoding is
//! logarithmic in the size of the domain -- much smaller than [`IntDirect`](super::super::IntDirect)
//! or [`IntOrder`](super::super::IntOrder), at the cost of needing adder circuits for arithmetic.

use crate::shared::representation_prelude::*;
use crate::types::int::{finite_int_bounds, int_domain_to_expr, int_ranges};
use conjure_cp::ast::{Domain, Moo, Reference, SATIntEncoding};
use conjure_cp::into_matrix_expr;
use conjure_cp::settings::SolverFamily;
use std::collections::VecDeque;

register_representation!(
    IntLog("int_log")
    struct State<T> {
        /// Overall lower and upper bound of the represented domain.
        pub bounds: (i32, i32),
        /// The domain's inclusive intervals, kept so the structural constraint can rule out gaps.
        pub ranges: Vec<(i32, i32)>,
        /// Two's-complement bits, least significant first; the last is the sign bit.
        pub bits: Moo<Vec<T>>
    }
    impl State<DeclarationPtr> {
        /// This variable as a log-encoded `SATInt`, the form the SAT rules operate on.
        pub fn sat_int_expr(&self) -> Expression {
            let bits: Vec<Expression> = self
                .bits
                .iter()
                .map(|declaration| Reference::new(declaration.clone()).into())
                .collect();
            Expression::SATInt(
                Metadata::new(),
                SATIntEncoding::Log,
                Moo::new(into_matrix_expr!(bits)),
                self.bounds,
            )
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |msg: &str| ReprInitError::UnsupportedDomain(
            dom.clone(),
            IntLog::NAME,
            String::from(msg),
        );

        let ranges = int_ranges(&dom)
            .ok_or_else(|| domain_err("expected a finite ground integer domain"))?;
        let bounds @ (low, high) = finite_int_bounds(&ranges)
            .ok_or_else(|| domain_err("expected a non-empty integer domain"))?;

        let width = log_width(low, high);
        let bits = Moo::new(std::iter::repeat_n(Domain::bool(), width).collect());
        Ok(State { bounds, ranges, bits })
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        // The bit vector can hold values outside the domain -- both the gaps between ranges and,
        // where the domain does not fill the width, values beyond its bounds. Say so explicitly.
        vec![int_domain_to_expr(state.sat_int_expr(), &state.ranges)]
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::Int(value) = value else {
            return Err(ReprDownError::BadValue(value, String::from("expected an integer")));
        };

        let bits = (0..state.bits.len())
            .map(|index| Literal::Bool((value >> index) & 1 != 0))
            .collect();

        Ok(State {
            bounds: state.bounds,
            ranges: state.ranges.clone(),
            bits: Moo::new(bits),
        })
    }
    fn up(state: State<Literal>) -> Literal {
        let width = state.bits.len();
        let mut value: i32 = 0;
        for (index, bit) in state.bits.iter().enumerate() {
            let set = match bit {
                Literal::Bool(set) => *set,
                Literal::Int(0) => false,
                Literal::Int(1) => true,
                other => bug!("expected a Boolean bit value, got {other}"),
            };
            if set {
                value |= 1 << index;
            }
        }

        // Reinterpret the top bit as the sign, so the bits read back as two's complement.
        let sign_bit = 1i32 << (width - 1);
        if value & sign_bit != 0 {
            value -= sign_bit << 1;
        }
        Literal::Int(value)
    }
    fn repr_vars(state: &State<DeclarationPtr>) -> VecDeque<DeclarationPtr> {
        state.bits.iter().cloned().collect()
    }
    fn compactness(state: &State<DomainPtr>) -> usize {
        1usize << state.bits.len().min(usize::BITS as usize - 1)
    }
    fn applies(family: SolverFamily) -> bool {
        matches!(family, SolverFamily::Sat)
    }
);

/// The narrowest two's-complement width that holds every value in `low..=high`.
fn log_width(low: i32, high: i32) -> usize {
    (1..=32)
        .find(|&width| {
            let min_possible = -(1i64 << (width - 1));
            let max_possible = (1i64 << (width - 1)) - 1;
            (low as i64) >= min_possible && (high as i64) <= max_possible
        })
        .unwrap_or(32)
}
