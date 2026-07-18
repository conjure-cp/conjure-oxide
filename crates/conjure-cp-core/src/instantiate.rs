use crate::{
    Model,
    ast::{DeclarationKind, DeclarationPtr, Literal, declaration::Declaration, eval_constant},
};
use anyhow::anyhow;

/// Instantiate a problem model with values from a parameter model.
///
/// For each `given` declaration in `problem_model`, this looks for a corresponding value `letting`
/// in `param_model`, checks it is a constant and within the given domain, and replaces the `given`
/// with a value-letting in the returned model.
pub fn instantiate_model(mut problem_model: Model, param_model: Model) -> anyhow::Result<Model> {
    let symbol_table = problem_model.symbols_ptr_unchecked().write();
    let param_table = param_model.symbols_ptr_unchecked().write();
    let mut pending_givens = symbol_table
        .iter_local()
        .filter_map(|(name, decl)| decl.as_given().map(|_| name.clone()))
        .collect::<Vec<_>>();

    while !pending_givens.is_empty() {
        let mut next_pending = Vec::new();
        let mut made_progress = false;

        for name in pending_givens {
            let mut decl = symbol_table
                .lookup_local(&name)
                .ok_or_else(|| anyhow!("Given declaration `{name}` not found in problem model"))?;

            let Some(domain) = decl.as_given() else {
                continue;
            };

            let param_decl = param_table.lookup(&name);
            let expr = param_decl
                .as_ref()
                .and_then(DeclarationPtr::as_value_letting)
                .ok_or_else(|| {
                    anyhow!(
                        "Given declaration `{name}` does not have corresponding letting in parameter file"
                    )
                })?;

            let expr_value = eval_constant(&expr)
                .ok_or_else(|| anyhow!("Letting expression `{expr}` cannot be evaluated"))?;

            let Ok(ground_domain) = domain.resolve() else {
                next_pending.push(name);
                continue;
            };

            if !ground_domain.contains(&expr_value)? {
                return Err(anyhow!(
                    "Domain of given statement `{name}` does not contain letting value"
                ));
            }

            // The given domain is a validity check, but after instantiation the parameter is a
            // constant. Keep the tighter domain inferred from its value when possible so bounds
            // derived from instantiated parameters (for example optimisation auxiliaries) stay
            // finite.
            let instantiated_domain = expr.domain_of().unwrap_or_else(|| domain.clone());
            let new_decl = Declaration::new(
                name.clone(),
                DeclarationKind::ValueLetting(expr.clone(), Some(instantiated_domain)),
            );
            drop(domain);
            decl.replace(new_decl);
            made_progress = true;

            tracing::info!("Replaced {name} given with letting.");
        }

        if next_pending.is_empty() {
            break;
        }

        if !made_progress {
            return Err(anyhow!(
                "Domain of given statement `{}` cannot be resolved",
                next_pending[0]
            ));
        }

        pending_givens = next_pending;
    }

    drop(symbol_table);
    validate_instantiation_conditions(&mut problem_model)?;
    Ok(problem_model)
}

/// Evaluate and remove all top-level `where` conditions after parameter instantiation.
pub fn validate_instantiation_conditions(model: &mut Model) -> anyhow::Result<()> {
    for condition in model.take_instantiation_conditions() {
        match eval_constant(&condition) {
            Some(Literal::Bool(true)) => {}
            Some(Literal::Bool(false)) => {
                return Err(anyhow!(
                    "invalid instance: where condition `{condition}` evaluated to false"
                ));
            }
            Some(value) => {
                return Err(anyhow!(
                    "where condition `{condition}` evaluated to non-boolean value `{value}`"
                ));
            }
            None => {
                return Err(anyhow!(
                    "could not evaluate where condition `{condition}` after parameter instantiation"
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Atom, Domain, GroundDomain, Metadata, Name, Range};

    #[test]
    fn instantiated_given_uses_the_tighter_value_domain() {
        let name = Name::user("n");
        let mut problem = Model::default();
        problem
            .add_symbol(DeclarationPtr::new_given(
                name.clone(),
                Domain::int(vec![Range::UnboundedR(1)]),
            ))
            .unwrap();

        let mut parameters = Model::default();
        parameters
            .add_symbol(DeclarationPtr::new_value_letting(
                name.clone(),
                crate::ast::Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(7))),
            ))
            .unwrap();

        let instantiated = instantiate_model(problem, parameters).unwrap();
        let declaration = instantiated
            .symbols()
            .lookup(&name)
            .expect("instantiated parameter should exist");

        assert_eq!(
            declaration.domain().unwrap().resolve().unwrap().as_ref(),
            &GroundDomain::Int(vec![Range::Single(7)])
        );
    }
}
