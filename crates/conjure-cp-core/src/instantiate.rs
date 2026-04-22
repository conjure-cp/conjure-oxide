use crate::{
    Model,
    ast::{DeclarationKind, DeclarationPtr, declaration::Declaration, eval_constant},
};
use anyhow::anyhow;

/// Instantiate a problem model with values from a parameter model.
///
/// For each `given` declaration in `problem_model`, this looks for a corresponding value `letting`
/// in `param_model`, checks it is a constant and within the given domain, and replaces the `given`
/// with a value-letting in the returned model.
pub fn instantiate_model(problem_model: Model, param_model: Model) -> anyhow::Result<Model> {
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

            let Some(ground_domain) = domain.resolve() else {
                next_pending.push(name);
                continue;
            };

            if !ground_domain.contains(&expr_value).unwrap() {
                return Err(anyhow!(
                    "Domain of given statement `{name}` does not contain letting value"
                ));
            }

            let new_decl = Declaration::new(
                name.clone(),
                DeclarationKind::ValueLetting(expr.clone(), Some(domain.clone())),
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
    Ok(problem_model)
}
