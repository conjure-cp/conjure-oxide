//! Comprehension expansion rules

mod expand_native;
mod expand_via_solver;
mod expand_via_solver_ac;
mod via_solver_common;

pub use expand_native::expand_native;
pub use expand_via_solver::expand_via_solver;
pub use expand_via_solver_ac::expand_via_solver_ac;

use conjure_cp::{
    ast::{
        DeclarationPtr, Domain, DomainPtr, Expression as Expr, IntVal, Moo, Name, Range, Reference,
        SymbolTable, UnresolvedDomain,
        comprehension::{Comprehension, ComprehensionQualifier},
        serde::{HasId, ObjId},
    },
    bug, into_matrix_expr,
    rule_engine::{
        ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
    },
    settings::{QuantifiedExpander, comprehension_expander},
};
use std::collections::HashMap;
use uniplate::{Biplate, Uniplate};

/// Rewrite top-level `exists` comprehensions into constraints over fresh machine `find`s.
///
/// `exists` is represented as `or([comprehension])`.
#[register_rule("Base", 2003, [Root])]
fn exists_quantified_to_finds(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Root(metadata, constraints) = expr else {
        return Err(RuleNotApplicable);
    };

    let mut new_symbols = symbols.clone();
    let mut new_constraints = Vec::with_capacity(constraints.len());
    let mut changed = false;

    for constraint in constraints {
        let Some(comprehension) = as_exists_comprehension(constraint) else {
            new_constraints.push(constraint.clone());
            continue;
        };

        let Some(new_constraints_for_exists) =
            rewrite_exists_comprehension_to_constraints(&comprehension, &mut new_symbols)
        else {
            new_constraints.push(constraint.clone());
            continue;
        };

        new_constraints.extend(new_constraints_for_exists);
        changed = true;
    }

    if changed {
        Ok(Reduction::with_symbols(
            Expr::Root(metadata.clone(), new_constraints),
            new_symbols,
        ))
    } else {
        Err(RuleNotApplicable)
    }
}

/// Expand comprehensions using `--comprehension-expander native`.
#[register_rule("Base", 2000, [Comprehension])]
fn expand_comprehension_native(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    if comprehension_expander() != QuantifiedExpander::Native {
        return Err(RuleNotApplicable);
    }

    let Expr::Comprehension(_, comprehension) = expr else {
        return Err(RuleNotApplicable);
    };

    let comprehension = comprehension.as_ref().clone();

    for qual in &comprehension.qualifiers {
        if let ComprehensionQualifier::ExpressionGenerator { .. } = qual {
            return Err(RuleNotApplicable);
        }
    }

    let mut symbols = symbols.clone();
    let results = expand_native(comprehension, &mut symbols)
        .unwrap_or_else(|e| bug!("native comprehension expansion failed: {e}"));
    Ok(Reduction::with_symbols(into_matrix_expr!(results), symbols))
}

/// Expand comprehensions using `--comprehension-expander via-solver`.
///
/// Algorithm sketch:
/// 1. Match one comprehension node.
/// 2. Build a temporary generator submodel from its qualifiers/guards.
/// 3. Materialise quantified declarations as temporary `find` declarations.
/// 4. Wrap that submodel as a standalone temporary model, with search order restricted to the
///    quantified names.
/// 5. Rewrite the temporary model using the configured rewriter and Minion-oriented rules.
/// 6. Solve the rewritten temporary model with Minion and keep only quantified assignments from
///    each solution.
/// 7. Instantiate the original return expression under each quantified assignment.
/// 8. Replace the comprehension by a matrix literal containing all instantiated return values.
#[register_rule("Base", 2000, [Comprehension])]
fn expand_comprehension_via_solver(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    if !matches!(
        comprehension_expander(),
        QuantifiedExpander::ViaSolver | QuantifiedExpander::ViaSolverAc
    ) {
        return Err(RuleNotApplicable);
    }

    let Expr::Comprehension(_, comprehension) = expr else {
        return Err(RuleNotApplicable);
    };

    let comprehension = comprehension.as_ref().clone();

    for qual in &comprehension.qualifiers {
        if let ComprehensionQualifier::ExpressionGenerator { .. } = qual {
            return Err(RuleNotApplicable);
        }
    }

    let results = expand_via_solver(comprehension)
        .unwrap_or_else(|e| bug!("via-solver comprehension expansion failed: {e}"));
    Ok(Reduction::with_symbols(
        into_matrix_expr!(results),
        symbols.clone(),
    ))
}

/// Expand comprehensions inside AC operators using `--comprehension-expander via-solver-ac`.
///
/// Algorithm sketch:
/// 1. Match an AC operator whose single child is a comprehension.
/// 2. Build a temporary generator submodel from the comprehension qualifiers/guards.
/// 3. Add a derived constraint from the return expression to this generator model:
///    localise non-local references, and replace non-quantified fragments with dummy variables so
///    the constraint depends only on locally solvable symbols.
/// 4. Materialise quantified declarations as temporary `find` declarations in the temporary model.
/// 5. Rewrite and solve the temporary model with Minion; keep only quantified assignments.
/// 6. Instantiate the original return expression under those assignments.
/// 7. Rebuild the same AC operator around the instantiated matrix literal.
#[register_rule("Base", 2002)]
fn expand_comprehension_via_solver_ac(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    if comprehension_expander() != QuantifiedExpander::ViaSolverAc {
        return Err(RuleNotApplicable);
    }

    // Is this an ac expression?
    let ac_operator_kind = expr.to_ac_operator_kind().ok_or(RuleNotApplicable)?;

    debug_assert_eq!(
        expr.children().len(),
        1,
        "AC expressions should have exactly one child."
    );

    let comprehension = as_single_comprehension(&expr.children()[0]).ok_or(RuleNotApplicable)?;

    for qual in &comprehension.qualifiers {
        if let ComprehensionQualifier::ExpressionGenerator { .. } = qual {
            return Err(RuleNotApplicable);
        }
    }

    let results =
        expand_via_solver_ac(comprehension, ac_operator_kind).or(Err(RuleNotApplicable))?;

    let new_expr = ac_operator_kind.as_expression(into_matrix_expr!(results));
    Ok(Reduction::with_symbols(new_expr, symbols.clone()))
}

fn as_single_comprehension(expr: &Expr) -> Option<Comprehension> {
    if let Expr::Comprehension(_, comprehension) = expr {
        return Some(comprehension.as_ref().clone());
    }

    let exprs = expr.clone().unwrap_list()?;
    let [Expr::Comprehension(_, comprehension)] = exprs.as_slice() else {
        return None;
    };

    Some(comprehension.as_ref().clone())
}

fn as_exists_comprehension(expr: &Expr) -> Option<Comprehension> {
    let Expr::Or(_, or_child) = expr else {
        return None;
    };

    as_single_comprehension(or_child.as_ref())
}

fn rewrite_exists_comprehension_to_constraints(
    comprehension: &Comprehension,
    symbols: &mut SymbolTable,
) -> Option<Vec<Expr>> {
    let quantified_declarations = quantified_declarations(comprehension)?;

    let mut replacements_by_id: HashMap<ObjId, DeclarationPtr> = HashMap::new();
    let mut replacements_by_name: HashMap<Name, DeclarationPtr> = HashMap::new();

    for decl in quantified_declarations {
        let domain = decl.domain()?;
        let rewritten_domain =
            replace_declaration_ptrs_in_domain(domain, &replacements_by_id, &replacements_by_name);
        let fresh_decl = symbols.gen_find(&rewritten_domain);
        replacements_by_id.insert(decl.id(), fresh_decl.clone());
        replacements_by_name.insert(decl.name().clone(), fresh_decl);
    }

    let mut conjuncts = Vec::new();
    for qualifier in &comprehension.qualifiers {
        if let ComprehensionQualifier::Condition(condition) = qualifier {
            conjuncts.push(replace_declaration_ptrs_in_expr(
                condition.clone(),
                &replacements_by_id,
                &replacements_by_name,
            ));
        }
    }
    conjuncts.push(replace_declaration_ptrs_in_expr(
        comprehension.return_expression.clone(),
        &replacements_by_id,
        &replacements_by_name,
    ));

    Some(conjuncts)
}

fn quantified_declarations(comprehension: &Comprehension) -> Option<Vec<DeclarationPtr>> {
    let quantified_names = comprehension.quantified_vars();
    let symbols = comprehension.symbols();
    quantified_names
        .into_iter()
        .map(|name| symbols.lookup_local(&name))
        .collect()
}

fn replace_declaration_ptrs_in_expr(
    expr: Expr,
    replacements_by_id: &HashMap<ObjId, DeclarationPtr>,
    replacements_by_name: &HashMap<Name, DeclarationPtr>,
) -> Expr {
    expr.transform_bi(&|decl: DeclarationPtr| {
        if let Some(replacement) = replacements_by_id.get(&decl.id()) {
            return replacement.clone();
        }

        let name = decl.name().clone();
        replacements_by_name.get(&name).cloned().unwrap_or(decl)
    })
}

fn replace_declaration_ptrs_in_domain(
    domain: DomainPtr,
    replacements_by_id: &HashMap<ObjId, DeclarationPtr>,
    replacements_by_name: &HashMap<Name, DeclarationPtr>,
) -> DomainPtr {
    let mut rewritten = domain
        .transform_bi(&|expr: Expr| {
            replace_declaration_ptrs_in_expr(expr, replacements_by_id, replacements_by_name)
        })
        .transform_bi(&|reference: Reference| {
            replace_reference(reference, replacements_by_id, replacements_by_name)
        });

    // `Range<T>` does not participate in the generic biplate traversal, so recurse through
    // unresolved domain structure once to rewrite symbolic integer bounds.
    rewrite_int_ranges_in_domain_ptr(&mut rewritten, replacements_by_id, replacements_by_name);

    rewritten
}

fn replace_reference(
    reference: Reference,
    replacements_by_id: &HashMap<ObjId, DeclarationPtr>,
    replacements_by_name: &HashMap<Name, DeclarationPtr>,
) -> Reference {
    let replacement = replacements_by_id
        .get(&reference.ptr().id())
        .cloned()
        .or_else(|| {
            let name = reference.name().clone();
            replacements_by_name.get(&name).cloned()
        });

    replacement.map(Reference::new).unwrap_or(reference)
}

fn rewrite_int_ranges_in_domain_ptr(
    domain: &mut DomainPtr,
    replacements_by_id: &HashMap<ObjId, DeclarationPtr>,
    replacements_by_name: &HashMap<Name, DeclarationPtr>,
) {
    let mut rewritten = domain.as_ref().clone();
    rewrite_int_ranges_in_domain(&mut rewritten, replacements_by_id, replacements_by_name);
    *domain = Moo::new(rewritten);
}

fn rewrite_int_ranges_in_domain(
    domain: &mut Domain,
    replacements_by_id: &HashMap<ObjId, DeclarationPtr>,
    replacements_by_name: &HashMap<Name, DeclarationPtr>,
) {
    let Domain::Unresolved(unresolved) = domain else {
        return;
    };

    rewrite_int_ranges_in_unresolved_domain(
        Moo::make_mut(unresolved),
        replacements_by_id,
        replacements_by_name,
    );
}

fn rewrite_int_ranges_in_unresolved_domain(
    unresolved: &mut UnresolvedDomain,
    replacements_by_id: &HashMap<ObjId, DeclarationPtr>,
    replacements_by_name: &HashMap<Name, DeclarationPtr>,
) {
    match unresolved {
        UnresolvedDomain::Int(ranges) => {
            for range in ranges {
                rewrite_int_range(range, replacements_by_id, replacements_by_name);
            }
        }
        UnresolvedDomain::Set(attr, inner) => {
            rewrite_int_range(&mut attr.size, replacements_by_id, replacements_by_name);
            rewrite_int_ranges_in_domain_ptr(inner, replacements_by_id, replacements_by_name);
        }
        UnresolvedDomain::MSet(attr, inner) => {
            rewrite_int_range(&mut attr.size, replacements_by_id, replacements_by_name);
            rewrite_int_range(
                &mut attr.occurrence,
                replacements_by_id,
                replacements_by_name,
            );
            rewrite_int_ranges_in_domain_ptr(inner, replacements_by_id, replacements_by_name);
        }
        UnresolvedDomain::Matrix(inner, index_domains) => {
            rewrite_int_ranges_in_domain_ptr(inner, replacements_by_id, replacements_by_name);
            for index_domain in index_domains {
                rewrite_int_ranges_in_domain_ptr(
                    index_domain,
                    replacements_by_id,
                    replacements_by_name,
                );
            }
        }
        UnresolvedDomain::Tuple(inner_domains) => {
            for inner_domain in inner_domains {
                rewrite_int_ranges_in_domain_ptr(
                    inner_domain,
                    replacements_by_id,
                    replacements_by_name,
                );
            }
        }
        UnresolvedDomain::Reference(_) => {}
        UnresolvedDomain::Record(entries) => {
            for entry in entries {
                rewrite_int_ranges_in_domain_ptr(
                    &mut entry.domain,
                    replacements_by_id,
                    replacements_by_name,
                );
            }
        }
        UnresolvedDomain::Function(attr, domain, codomain) => {
            rewrite_int_range(&mut attr.size, replacements_by_id, replacements_by_name);
            rewrite_int_ranges_in_domain_ptr(domain, replacements_by_id, replacements_by_name);
            rewrite_int_ranges_in_domain_ptr(codomain, replacements_by_id, replacements_by_name);
        }
        UnresolvedDomain::Relation(attr, domains) => {
            rewrite_int_range(&mut attr.size, replacements_by_id, replacements_by_name);
            for domain in domains {
                rewrite_int_ranges_in_domain_ptr(domain, replacements_by_id, replacements_by_name);
            }
        }
        UnresolvedDomain::EnumeratedType(reference, ranges) => todo!(),
        UnresolvedDomain::UnnamedType(reference) => todo!(),
    }
}

fn rewrite_int_range(
    range: &mut Range<IntVal>,
    replacements_by_id: &HashMap<ObjId, DeclarationPtr>,
    replacements_by_name: &HashMap<Name, DeclarationPtr>,
) {
    match range {
        Range::Single(value) | Range::UnboundedL(value) | Range::UnboundedR(value) => {
            rewrite_int_value(value, replacements_by_id, replacements_by_name);
        }
        Range::Bounded(lower, upper) => {
            rewrite_int_value(lower, replacements_by_id, replacements_by_name);
            rewrite_int_value(upper, replacements_by_id, replacements_by_name);
        }
        Range::Unbounded => {}
    }
}

fn rewrite_int_value(
    int_val: &mut IntVal,
    replacements_by_id: &HashMap<ObjId, DeclarationPtr>,
    replacements_by_name: &HashMap<Name, DeclarationPtr>,
) {
    if let IntVal::Expr(expr) = int_val {
        let rewritten = replace_declaration_ptrs_in_expr(
            (**expr).clone(),
            replacements_by_id,
            replacements_by_name,
        );
        *expr = Moo::new(rewritten);
    }
}
