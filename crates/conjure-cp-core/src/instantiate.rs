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
///
/// This is done in two passes so that given domains that reference other givens (e.g.
/// `given items : matrix indexed by [int(1..n)] of ...` where `n` is also a given) can be
/// resolved after all givens have been substituted.
pub fn instantiate_model(problem_model: Model, param_model: Model) -> anyhow::Result<Model> {
    let mut symbol_table = problem_model.symbols_ptr_unchecked().write();
    let param_table = param_model.symbols_ptr_unchecked().write();

    // Pass 1: look up every parameter value and replace each given with a value-letting.
    // Domain validation is deferred to pass 2 so that domains referencing other givens can
    // be resolved once all givens have been substituted.
    let mut replaced: Vec<DeclarationPtr> = Vec::new();

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

        let new_decl = Declaration::new(
            name.clone(),
            DeclarationKind::ValueLetting(expr.clone(), Some(domain.clone())),
        );
        drop(domain);
        drop(expr);

        decl.replace(new_decl);
        replaced.push(decl.clone());
        tracing::info!("Replaced {name} given with letting.");
    }

    // Pass 2: now that every given has been replaced, resolve domains and validate values.
    for decl in &replaced {
        let (expr, domain) = match &*decl.kind() {
            DeclarationKind::ValueLetting(expr, Some(domain)) => (expr.clone(), domain.clone()),
            _ => unreachable!("pass 1 always creates ValueLetting with Some(domain)"),
        };

        let expr_value = eval_constant(&expr)
            .ok_or_else(|| anyhow!("Letting expression `{expr}` cannot be evaluated"))?;

        let ground_domain = domain
            .resolve()
            .map_err(|e| anyhow!("Domain of given statement `{decl}` cannot be resolved: {e}"))?;

        if !ground_domain.contains(&expr_value)? {
            return Err(anyhow!(
                "Domain of given statement `{decl}` does not contain letting value"
            ));
        }
    }

    drop(symbol_table);
    Ok(problem_model)
}
