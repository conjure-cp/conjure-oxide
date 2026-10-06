//! Sign-and-magnitude bit-vector representation of an integer, for the SAT backend.
//!
//! `x` becomes the bits of `|x|` in binary, least significant first, followed by one sign bit that
//! is set exactly when `x` is negative. Zero has a single code: the negative zero code is ruled
//! out, so every solution is counted once.
use crate::shared::representation_prelude::*;
use crate::types::int::unsigned::{
    canonical_ranges, read_code, signed_magnitude_bounds, unsigned_bound, unsigned_width,
};
use crate::types::int::{finite_int_bounds, int_domain_to_expr, int_ranges};
use conjure_cp::ast::{Domain, Metadata, Moo, Reference, SATIntEncoding};
use conjure_cp::into_matrix_expr;
use conjure_cp::settings::SolverFamily;
use std::collections::VecDeque;

/// The largest magnitude any value of the domain has.
fn max_magnitude(bounds: (i32, i32)) -> u64 {
    i64::from(bounds.0)
        .unsigned_abs()
        .max(i64::from(bounds.1).unsigned_abs())
}

register_representation!(
    IntSignMagnitude("sign_magnitude")
    struct State<T> {
        /// Minimum and maximum semantic values.
        pub bounds: (i32, i32),
        /// Sorted disjoint inclusive intervals of allowed values.
        pub ranges: Vec<(i32, i32)>,
        /// Magnitude bits, least significant first, then the sign bit.
        pub bits: Moo<Vec<T>>
    }
    impl State<DeclarationPtr> {
        /// Semantic integer operand retaining its sign-and-magnitude layout.
        pub fn sat_int_expr(&self) -> Expression {
            let bits: Vec<Expression> = self.bits.iter()
                .map(|decl| Reference::new(decl.clone()).into()).collect();
            Expression::SATInt(Metadata::new(), SATIntEncoding::SignMagnitude,
                Moo::new(into_matrix_expr!(bits)), self.bounds)
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let error = || ReprInitError::UnsupportedDomain(dom.clone(), IntSignMagnitude::NAME,
            "expected a non-empty finite ground integer domain".into());
        let ranges = canonical_ranges(int_ranges(&dom).ok_or_else(error)?);
        let bounds = finite_int_bounds(&ranges).ok_or_else(error)?;
        let magnitude_width = unsigned_width(max_magnitude(bounds));
        let bits = Moo::new(std::iter::repeat_n(Domain::bool(), magnitude_width + 1).collect());
        Ok(State { bounds, ranges, bits })
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        let (sign, magnitude) = state.bits.split_last().expect("a sign bit");
        let sign: Expression = Reference::new(sign.clone()).into();
        let mut constraints = vec![unsigned_bound(magnitude, max_magnitude(state.bounds))];
        constraints.extend(signed_magnitude_bounds(magnitude, sign.clone(), state.bounds));
        // Negative zero: the sign may only be set when some magnitude bit is.
        let magnitude_bits: Vec<Expression> = magnitude.iter()
            .map(|decl| Reference::new(decl.clone()).into()).collect();
        constraints.push(Expression::Imply(
            Metadata::new(),
            Moo::new(sign),
            Moo::new(Expression::Or(Metadata::new(), Moo::new(into_matrix_expr!(magnitude_bits)))),
        ));
        constraints.push(int_domain_to_expr(state.sat_int_expr(), &state.ranges));
        constraints
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::Int(value) = value else {
            return Err(ReprDownError::BadValue(value, "expected an integer".into()));
        };
        if !state.ranges.iter().any(|(low, high)| (*low..=*high).contains(&value)) {
            return Err(ReprDownError::BadValue(Literal::Int(value), "value outside the domain".into()));
        }
        let magnitude = i64::from(value).unsigned_abs();
        let mut bits: Vec<Literal> = (0..state.bits.len() - 1)
            .map(|i| Literal::Bool((magnitude >> i) & 1 != 0))
            .collect();
        bits.push(Literal::Bool(value < 0));
        Ok(State { bounds: state.bounds, ranges: state.ranges.clone(), bits: Moo::new(bits) })
    }
    fn up(state: State<Literal>) -> Literal {
        let (sign, magnitude) = state.bits.split_last().expect("a sign bit");
        let magnitude = read_code(magnitude) as i64;
        let negative = read_code(std::slice::from_ref(sign)) == 1;
        let value = if negative { -magnitude } else { magnitude };
        Literal::Int(i32::try_from(value).expect("invalid SAT integer code"))
    }
    fn repr_vars(state: &State<DeclarationPtr>) -> VecDeque<DeclarationPtr> {
        state.bits.iter().cloned().collect()
    }
    fn compactness(state: &State<DomainPtr>) -> usize {
        1usize << state.bits.len().min(usize::BITS as usize - 1)
    }
    fn integer_encoding() -> bool { true }
    fn applies(family: SolverFamily) -> bool { matches!(family, SolverFamily::Sat) }
);
