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
    let mut symbol_table = problem_model.symbols_ptr_unchecked().write();
    let param_table = param_model.symbols_ptr_unchecked().write();

    for (name, decl) in symbol_table.iter_local_mut() {
        let Some(domain) = decl.as_given() else {
            continue;
        };

        let param_decl = param_table.lookup(name);
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

        let ground_domain = domain
            .resolve()
            .ok_or_else(|| anyhow!("Domain of given statement `{name}` cannot be resolved"))?;

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

        tracing::info!("Replaced {name} given with letting.");
    }

    drop(symbol_table);
    Ok(problem_model)
}
