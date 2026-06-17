//! Normalisation of optimisation objectives before rewriting.

use crate::ast::{Atom, Expression, Metadata, Model, Moo, Reference};
use crate::bug;

/// Introduces a find variable for a non-atomic optimisation objective and links it with an
/// aux declaration constraint.
///
/// Objectives that are already atoms (for example `minimising z`) are left unchanged.
pub fn introduce_objective_auxiliary(mut model: Model) -> Model {
    let Some(objective) = model.objective.as_ref() else {
        return model;
    };

    if matches!(&objective.expression, Expression::Atomic(_, _)) {
        return model;
    }

    let expr = objective.expression.clone();

    let Some(domain) = expr.domain_of() else {
        bug!(
            "objective expression has no domain and could not be introduced as an auxiliary variable: {expr}"
        );
    };

    let mut symbols = model.symbols().clone();
    let decl = symbols.gen_find(&domain);
    let aux_reference = Expression::Atomic(Metadata::new(), Atom::new_ref(decl.clone()));
    let aux_constraint = Expression::AuxDeclaration(
        Metadata::new(),
        Reference::new(decl),
        Moo::new(expr),
    );

    model.symbols_mut().extend(symbols);
    model.add_constraint(aux_constraint);
    model
        .objective
        .as_mut()
        .expect("objective should still be present")
        .expression = aux_reference;

    model
}
