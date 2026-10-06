//! Unsigned rank among allowed domain values, for the SAT backend.
use crate::shared::representation_prelude::*;
use crate::types::int::unsigned::{
    canonical_ranges, rank_of, read_code, unsigned_bound, unsigned_capacity, unsigned_width,
    value_at_rank,
};
use crate::types::int::{finite_int_bounds, int_ranges};
use conjure_cp::ast::{Domain, Moo, Reference, SATIntEncoding};
use conjure_cp::into_matrix_expr;
use conjure_cp::settings::SolverFamily;
use std::collections::VecDeque;

register_representation!(
    IntRank("rank")
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
            Expression::SATInt(Metadata::new(), SATIntEncoding::Rank(self.ranges.clone()),
                Moo::new(into_matrix_expr!(bits)), self.bounds)
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let error = || ReprInitError::UnsupportedDomain(dom.clone(), IntRank::NAME,
            "expected a non-empty finite ground integer domain".into());
        let ranges = canonical_ranges(int_ranges(&dom).ok_or_else(error)?);
        let bounds = finite_int_bounds(&ranges).ok_or_else(error)?;
        let capacity = unsigned_capacity(&ranges, true);
        let bits = Moo::new(std::iter::repeat_n(Domain::bool(), unsigned_width(capacity - 1)).collect());
        Ok(State { bounds, ranges, bits })
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        vec![unsigned_bound(&state.bits, unsigned_capacity(&state.ranges, true) - 1)]
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::Int(value) = value else {
            return Err(ReprDownError::BadValue(value, "expected an integer".into()));
        };
        if !state.ranges.iter().any(|(low, high)| (*low..=*high).contains(&value)) {
            return Err(ReprDownError::BadValue(Literal::Int(value), "value outside the domain".into()));
        }
        let code = rank_of(&state.ranges, value).expect("membership checked") as i64;
        let bits = (0..state.bits.len()).map(|i| Literal::Bool((code >> i) & 1 != 0)).collect();
        Ok(State { bounds: state.bounds, ranges: state.ranges.clone(), bits: Moo::new(bits) })
    }
    fn up(state: State<Literal>) -> Literal {
        let code = read_code(&state.bits);
        let value = i64::from(value_at_rank(&state.ranges, code).expect("invalid SAT rank"));
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
