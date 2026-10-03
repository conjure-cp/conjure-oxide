//! Unsigned displacement from the domain minimum, for the SAT backend.
use crate::shared::representation_prelude::*;
use crate::types::int::unsigned::{
    canonical_ranges, read_code, unsigned_bound, unsigned_capacity, unsigned_width,
};
use crate::types::int::{finite_int_bounds, int_domain_to_expr, int_ranges};
use conjure_cp::ast::{Domain, Moo, Reference, SATIntEncoding};
use conjure_cp::into_matrix_expr;
use conjure_cp::settings::SolverFamily;
use std::collections::VecDeque;

register_representation!(
    IntOffset("int_offset")
    struct State<T> {
        /// Minimum and maximum semantic values.
        pub bounds: (i32, i32),
        /// Sorted disjoint inclusive intervals of allowed values.
        pub ranges: Vec<(i32, i32)>,
        /// Unsigned code bits, least significant first.
        pub bits: Moo<Vec<T>>
    }
    impl State<DeclarationPtr> {
        /// Semantic integer operand retaining its unsigned representation.
        pub fn sat_int_expr(&self) -> Expression {
            let bits: Vec<Expression> = self.bits.iter()
                .map(|decl| Reference::new(decl.clone()).into()).collect();
            Expression::SATInt(Metadata::new(), SATIntEncoding::Offset,
                Moo::new(into_matrix_expr!(bits)), self.bounds)
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let error = || ReprInitError::UnsupportedDomain(dom.clone(), IntOffset::NAME,
            "expected a non-empty finite ground integer domain".into());
        let ranges = canonical_ranges(int_ranges(&dom).ok_or_else(error)?);
        let bounds = finite_int_bounds(&ranges).ok_or_else(error)?;
        let capacity = unsigned_capacity(&ranges, false);
        let bits = Moo::new(std::iter::repeat_n(Domain::bool(), unsigned_width(capacity - 1)).collect());
        Ok(State { bounds, ranges, bits })
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        let mut constraints = vec![unsigned_bound(&state.bits, unsigned_capacity(&state.ranges, false) - 1)];
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
        let code = i64::from(value) - i64::from(state.bounds.0);
        let bits = (0..state.bits.len()).map(|i| Literal::Bool((code >> i) & 1 != 0)).collect();
        Ok(State { bounds: state.bounds, ranges: state.ranges.clone(), bits: Moo::new(bits) })
    }
    fn up(state: State<Literal>) -> Literal {
        let code = read_code(&state.bits);
        let value = i64::from(state.bounds.0) + code as i64;
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
