//! Matrix held whole, as a solver's native array.
//!
//! Z3 has arrays, so a matrix can go to the solver in one piece rather than being taken apart into
//! one variable per entry the way [`MatrixComponents`](super::super::MatrixComponents) does, or
//! squeezed into a single integer the way [`MatrixPacked`](super::super::MatrixPacked) does. Which
//! is better depends on the model: an array keeps the constraint compact and lets the solver reason
//! about a variable index directly, while components expose each entry to the ordinary integer
//! rules.
//!
//! This says nothing about how the entries themselves are held. The variable it leaves behind is
//! still a matrix of integers, so it goes on to pick `lia` or `bv` like any other integer
//! declaration -- the array's element sort is those two choices combined, not something this
//! representation decides on its own.

use crate::shared::representation_prelude::*;
use conjure_cp::ast::GroundDomain;
use conjure_cp::representation::default_impls::domain_size;
use conjure_cp::settings::SolverFamily;
use std::collections::VecDeque;

/// Whether Z3 can represent this domain directly as a sort.
///
/// `MatrixArray` leaves the matrix value intact for the SMT adaptor, so every nested value and
/// index domain must be something that adaptor can lower without another representation pass.
/// Abstract values such as tuples must instead use `MatrixComponents`, after which each component
/// can choose its ordinary abstract-type representation.
fn has_native_z3_sort(domain: &GroundDomain) -> bool {
    match domain {
        GroundDomain::Bool | GroundDomain::Int(_) => true,
        GroundDomain::Matrix(value, indices) => {
            has_native_z3_sort(value)
                && indices
                    .iter()
                    .all(|index| has_native_z3_sort(index.as_ref()))
        }
        GroundDomain::Set(_, element) => has_native_z3_sort(element),
        _ => false,
    }
}

register_representation!(
    MatrixArray("array")
    struct State<T> {
        /// The matrix itself, which the solver holds as one array.
        pub value: T
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |message: &str| ReprInitError::UnsupportedDomain(
            dom.clone(),
            MatrixArray::NAME,
            String::from(message),
        );

        let resolved = dom.resolve().map_err(|_| domain_err("expected a ground matrix domain"))?;
        if !matches!(resolved.as_ref(), GroundDomain::Matrix(..)) {
            return Err(domain_err("expected a matrix domain"));
        }
        if !has_native_z3_sort(resolved.as_ref()) {
            return Err(domain_err(
                "matrix value or index domain has no native Z3 sort",
            ));
        }
        Ok(State { value: resolved.into() })
    }
    fn structural(_state: &State<DeclarationPtr>) -> Vec<Expression> {
        // The representation variable carries the original domain, which the adaptor turns into a
        // restriction over every index; there is nothing further to say here.
        Vec::new()
    }
    fn down(_state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        match value {
            value @ Literal::AbstractLiteral(AbstractLiteral::Matrix(..)) => Ok(State { value }),
            other => Err(ReprDownError::BadValue(other, String::from("expected a matrix literal"))),
        }
    }
    fn up(state: State<Literal>) -> Literal {
        state.value
    }
    fn repr_vars(state: &State<DeclarationPtr>) -> VecDeque<DeclarationPtr> {
        VecDeque::from([state.value.clone()])
    }
    fn compactness(state: &State<DomainPtr>) -> usize {
        // Holding the matrix whole admits exactly the assignments its domain does.
        domain_size(&state.value)
    }
    fn applies(family: SolverFamily) -> bool {
        matches!(family, SolverFamily::Z3)
    }
);

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{Moo, Range};

    fn int_domain(range: Range<i32>) -> GroundDomain {
        GroundDomain::Int(vec![range])
    }

    #[test]
    fn native_z3_sort_rejects_a_matrix_of_tuples() {
        let tuple = GroundDomain::Tuple(vec![
            int_domain(Range::Single(1)).into(),
            int_domain(Range::Bounded(2, 4)).into(),
        ]);
        let matrix = GroundDomain::Matrix(
            Moo::new(tuple),
            vec![Moo::new(int_domain(Range::Bounded(1, 3)))],
        );

        assert!(!has_native_z3_sort(&matrix));
    }

    #[test]
    fn native_z3_sort_accepts_a_matrix_of_sets_of_integers() {
        let set = GroundDomain::Set(
            Default::default(),
            Moo::new(int_domain(Range::Bounded(1, 3))),
        );
        let matrix = GroundDomain::Matrix(
            Moo::new(set),
            vec![Moo::new(int_domain(Range::Bounded(1, 2)))],
        );

        assert!(has_native_z3_sort(&matrix));
    }
}
