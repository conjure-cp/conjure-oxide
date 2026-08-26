//! Linear-integer-arithmetic representation of an integer, for the SMT backend.
//!
//! Unlike the SAT encodings this does not decompose the variable: the solver has integers, so the
//! representation is the identity, and all it records is which Z3 sort the variable should get.
//! Making it a representation is what lets the choice be explored by the heuristics, and what lets
//! a variable be channelled into [`SmtBv`](super::super::SmtBv) when a model wants both.

use crate::shared::representation_prelude::*;
use conjure_cp::ast::GroundDomain;
use conjure_cp::settings::SolverFamily;
use std::collections::VecDeque;

register_representation!(
    SmtLia("lia")
    struct State<T> {
        /// The variable itself, in the integer theory.
        pub value: T
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        if !holds_integers(&dom) {
            return Err(ReprInitError::UnsupportedDomain(
                dom.clone(),
                SmtLia::NAME,
                String::from("expected an integer domain, or a matrix of them"),
            ));
        }
        Ok(State { value: dom })
    }
    fn structural(_state: &State<DeclarationPtr>) -> Vec<Expression> {
        // The representation variable carries the original domain, which the adaptor turns into
        // its own range restriction; there is nothing further to say here.
        Vec::new()
    }
    fn down(_state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        match value {
            value @ (Literal::Int(_) | Literal::AbstractLiteral(AbstractLiteral::Matrix(..))) => {
                Ok(State { value })
            }
            other => Err(ReprDownError::BadValue(
                other,
                String::from("expected an integer, or a matrix of them"),
            )),
        }
    }
    fn up(state: State<Literal>) -> Literal {
        state.value
    }
    fn repr_vars(state: &State<DeclarationPtr>) -> VecDeque<DeclarationPtr> {
        VecDeque::from([state.value.clone()])
    }
    fn compactness(state: &State<DomainPtr>) -> usize {
        // The solver reasons over exactly the declared values, so the domain is the whole story.
        conjure_cp::representation::default_impls::domain_size(&state.value)
    }
    fn applies(family: SolverFamily) -> bool {
        matches!(family, SolverFamily::Z3)
    }
);

/// True if `dom` is an integer domain, or a matrix whose entries ultimately are.
///
/// The integer theories say how a declaration's integers are held, which is as meaningful for a
/// matrix the solver keeps whole as an array as it is for a lone integer -- in that case it is the
/// array's element sort. A matrix taken apart into components never reaches here; each component
/// picks for itself.
fn holds_integers(dom: &DomainPtr) -> bool {
    fn ground_holds_integers(dom: &GroundDomain) -> bool {
        match dom {
            GroundDomain::Int(_) => true,
            GroundDomain::Matrix(inner, _) => ground_holds_integers(inner),
            _ => false,
        }
    }

    dom.as_ground().is_some_and(ground_holds_integers)
}
