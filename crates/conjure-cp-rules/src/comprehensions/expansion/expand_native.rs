use std::collections::HashMap;

use conjure_cp::{
    ast::{
        Atom, DeclarationKind, DeclarationPtr, Expression, Literal, Metadata, Name, SymbolTable,
        comprehension::Comprehension,
        eval_constant, run_partial_evaluator,
        serde::{HasId as _, ObjId},
    },
    bug,
    solver::SolverError,
};
use uniplate::Biplate as _;

/// Expands the comprehension without calling an external solver.
///
/// Quantified variables are enumerated with native Rust loops over their finite domains. Guards
/// are evaluated using constant and partial evaluators after substitution.
pub fn expand_native(
    comprehension: Comprehension,
    symtab: &mut SymbolTable,
) -> Result<Vec<Expression>, SolverError> {
    let generator_symbols = comprehension.generator_submodel.symbols().clone();
    let quantified_vars = comprehension.quantified_vars.clone();

    let mut quantified_domains = Vec::with_capacity(quantified_vars.len());
    for name in &quantified_vars {
        let decl = generator_symbols.lookup_local(name).ok_or_else(|| {
            SolverError::ModelInvalid(format!(
                "quantified variable '{name}' is missing from generator symbol table"
            ))
        })?;

        let domain = decl.domain().ok_or_else(|| {
            SolverError::ModelInvalid(format!("quantified variable '{name}' has no domain"))
        })?;
        let resolved = domain.resolve().ok_or_else(|| {
            SolverError::ModelFeatureNotSupported(format!(
                "quantified variable '{name}' has unresolved domain: {domain}"
            ))
        })?;

        let values: Vec<Literal> = resolved
            .values()
            .map_err(|err| {
                SolverError::ModelFeatureNotSupported(format!(
                    "quantified variable '{name}' has non-enumerable domain: {err}"
                ))
            })?
            .collect();

        quantified_domains.push(values);
    }

    let mut assignments = HashMap::new();
    let mut expanded = Vec::new();

    enumerate_assignments(
        0,
        &quantified_vars,
        &quantified_domains,
        &mut assignments,
        &mut |assignment| {
            for guard in comprehension.generator_submodel.constraints() {
                match evaluate_guard(guard, assignment) {
                    Some(true) => {}
                    Some(false) => return Ok(()),
                    None => {
                        return Err(SolverError::ModelInvalid(format!(
                            "native comprehension expansion could not evaluate guard: {guard}"
                        )));
                    }
                }
            }

            let return_expression_submodel = comprehension.return_expression_submodel.clone();
            let child_symtab = return_expression_submodel.symbols().clone();
            let return_expression = return_expression_submodel.into_single_expression();

            let return_expression = substitute_quantified_literals(return_expression, assignment);
            let return_expression = simplify_expression(return_expression);

            // Copy machine-name declarations from comprehension-local return-expression scope.
            let mut machine_name_translations: HashMap<ObjId, DeclarationPtr> = HashMap::new();
            for (name, decl) in child_symtab.into_iter_local() {
                if assignment.get(&name).is_some()
                    && matches!(
                        &decl.kind() as &DeclarationKind,
                        DeclarationKind::Given(_) | DeclarationKind::Quantified(_)
                    )
                {
                    continue;
                }

                let Name::Machine(_) = &name else {
                    bug!(
                        "the symbol table of the return expression of a comprehension should only contain machine names"
                    );
                };

                let id = decl.id();
                let new_decl = symtab.gensym(&decl.domain().unwrap());
                machine_name_translations.insert(id, new_decl);
            }

            #[allow(clippy::arc_with_non_send_sync)]
            let return_expression = return_expression.transform_bi(&move |atom: Atom| {
                if let Atom::Reference(ref decl) = atom
                    && let id = decl.id()
                    && let Some(new_decl) = machine_name_translations.get(&id)
                {
                    Atom::Reference(conjure_cp::ast::Reference::new(new_decl.clone()))
                } else {
                    atom
                }
            });

            expanded.push(return_expression);
            Ok(())
        },
    )?;

    Ok(expanded)
}

fn enumerate_assignments(
    index: usize,
    quantified_vars: &[Name],
    quantified_domains: &[Vec<Literal>],
    assignment: &mut HashMap<Name, Literal>,
    on_assignment: &mut impl FnMut(&HashMap<Name, Literal>) -> Result<(), SolverError>,
) -> Result<(), SolverError> {
    if index == quantified_vars.len() {
        return on_assignment(assignment);
    }

    let name = &quantified_vars[index];
    for lit in &quantified_domains[index] {
        assignment.insert(name.clone(), lit.clone());
        enumerate_assignments(
            index + 1,
            quantified_vars,
            quantified_domains,
            assignment,
            on_assignment,
        )?;
    }
    assignment.remove(name);
    Ok(())
}

fn evaluate_guard(guard: &Expression, assignment: &HashMap<Name, Literal>) -> Option<bool> {
    let substituted = substitute_quantified_literals(guard.clone(), assignment);
    let simplified = simplify_expression(substituted);
    match eval_constant(&simplified)? {
        Literal::Bool(value) => Some(value),
        _ => None,
    }
}

fn substitute_quantified_literals(
    expr: Expression,
    assignment: &HashMap<Name, Literal>,
) -> Expression {
    expr.transform_bi(&|atom: Atom| {
        let Atom::Reference(ref decl) = atom else {
            return atom;
        };

        let Some(lit) = assignment.get(&decl.name()) else {
            return atom;
        };

        Atom::Literal(lit.clone())
    })
}

fn simplify_expression(mut expr: Expression) -> Expression {
    // Keep applying evaluators to a fixed point, or until no changes are made.
    for _ in 0..128 {
        let next = expr.clone().transform_bi(&|subexpr: Expression| {
            if let Some(lit) = eval_constant(&subexpr) {
                return Expression::Atomic(Metadata::new(), Atom::Literal(lit));
            }
            if let Ok(reduction) = run_partial_evaluator(&subexpr) {
                return reduction.new_expression;
            }
            subexpr
        });

        if next == expr {
            break;
        }
        expr = next;
    }
    expr
}
