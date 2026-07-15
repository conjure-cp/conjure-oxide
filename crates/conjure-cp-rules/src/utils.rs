use std::collections::VecDeque;

use conjure_cp::ast::{
    AbstractLiteral, Atom, DeclarationPtr, DomainPtr, Expression as Expr, Literal, Metadata, Moo,
    Name, SymbolTable,
    categories::Category,
    comprehension::{Comprehension, ComprehensionQualifier},
};
use conjure_cp::rule_engine::RuleEffect;

use tracing::{instrument, trace};
use uniplate::{Biplate, Uniplate};

/// True iff `expr` is an `Atom`.
pub fn is_atom(expr: &Expr) -> bool {
    matches!(expr, Expr::Atomic(_, _))
}

/// True iff `expr` is an `Atom` or `Not(Atom)`.
pub fn is_literal(expr: &Expr) -> bool {
    match expr {
        Expr::Atomic(_, _) => true,
        Expr::Not(_, inner) => matches!(**inner, Expr::Atomic(_, _)),
        _ => false,
    }
}

/// True if `expr` is flat; i.e. it only contains atoms.
pub fn is_flat(expr: &Expr) -> bool {
    expr.children().iter().all(is_atom)
}

/// Rewrites the direct expression children of `expr`, preserving the number of children.
///
/// Returns the rebuilt expression and the number of children marked as changed by `rewrite`.
pub fn rewrite_children(
    expr: &Expr,
    mut rewrite: impl FnMut(Expr) -> (Expr, bool),
) -> (Expr, usize) {
    let mut num_changed = 0;
    let children: VecDeque<Expr> = expr
        .children()
        .into_iter()
        .map(|child| {
            let (new_child, changed) = rewrite(child);
            if changed {
                num_changed += 1;
            }
            new_child
        })
        .collect();

    (expr.with_children(children), num_changed)
}

/// Returns the only direct `Vec<Expr>` child of `expr`, if it has exactly one.
pub fn single_vec_child(expr: &Expr) -> Option<Vec<Expr>> {
    let mut child_vecs: VecDeque<Vec<Expr>> = expr.children_bi();
    if child_vecs.len() == 1 {
        child_vecs.pop_front()
    } else {
        None
    }
}

/// Rebuilds `expr` with a replacement for its only direct `Vec<Expr>` child.
pub fn with_single_vec_child(expr: &Expr, child: Vec<Expr>) -> Expr {
    expr.with_children_bi(VecDeque::from([child]))
}

/// Returns the arity of a tuple constant expression, if this expression is one.
pub fn constant_tuple_len(expr: &Expr) -> Option<usize> {
    match expr {
        Expr::AbstractLiteral(_, AbstractLiteral::Tuple(elems)) => Some(elems.len()),
        Expr::Atomic(_, Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Tuple(elems)))) => {
            Some(elems.len())
        }
        _ => None,
    }
}

/// Returns record field names of a record constant expression, if this expression is one.
pub fn constant_record_names(expr: &Expr) -> Option<Vec<Name>> {
    match expr {
        Expr::AbstractLiteral(_, AbstractLiteral::Record(entries)) => {
            Some(entries.iter().map(|x| x.name.clone()).collect())
        }
        Expr::Atomic(
            _,
            Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Record(entries))),
        ) => Some(entries.iter().map(|x| x.name.clone()).collect()),
        _ => None,
    }
}

/// Converts a vector of expressions to a vector of atoms.
///
/// # Returns
///
/// `Some(Vec<Atom>)` if the vectors direct children expressions are all atomic, otherwise `None`.
#[allow(dead_code)]
pub fn expressions_to_atoms(exprs: &Vec<Expr>) -> Option<Vec<Atom>> {
    let mut atoms: Vec<Atom> = vec![];
    for expr in exprs {
        let Expr::Atomic(_, atom) = expr else {
            return None;
        };
        atoms.push(atom.clone());
    }

    Some(atoms)
}

/// Creates a new auxiliary variable using the given expression.
///
/// # Returns
///
/// * `None` if `Expr` is a `Atom`, or `Expr` does not have a domain (for example, if it is a `Bubble`).
///
/// * `Some(ToAuxVarOutput)` if successful, containing:
///
///     + A new symbol table, modified to include the auxiliary variable.
///     + A new top level expression, containing the declaration of the auxiliary variable.
///     + A reference to the auxiliary variable to replace the existing expression with.
///
#[instrument(skip_all, fields(expr = %expr))]
pub fn to_aux_var(expr: &Expr, symbols: &SymbolTable) -> Option<ToAuxVarOutput> {
    let domain = to_aux_var_domain(expr)?;
    Some(materialise_aux_var(expr, symbols, &domain))
}

fn to_aux_var_domain(expr: &Expr) -> Option<DomainPtr> {
    // No need to put an atom in an aux_var
    if is_atom(expr) {
        if cfg!(debug_assertions) {
            trace!(why = "expression is an atom", "to_aux_var() failed");
        }
        return None;
    }

    // Anything that should be bubbled, bubble
    if !expr.is_safe() {
        if cfg!(debug_assertions) {
            trace!(why = "expression is unsafe", "to_aux_var() failed");
        }
        return None;
    }

    // Do not put abstract literals containing expressions into aux vars.
    //
    // e.g. for `[1,2,3,f/2,e][e]`, the lhs should not be put in an aux var.
    //
    // instead, we should flatten the elements inside this abstract literal, or wait for it to be
    // turned into an atom, or an abstract literal containing only literals - e.g. through an index
    // or slice operation.
    //
    if let Expr::AbstractLiteral(_, _) = expr {
        if cfg!(debug_assertions) {
            trace!(
                why = "expression is an abstract literal",
                "to_aux_var() failed"
            );
        }
        return None;
    }

    // Only flatten an expression if it contains decision variables or decision variables with some
    // constants.
    //
    // i.e. dont flatten things containing givens, quantified variables, just constants, etc.
    let categories = expr.universe_categories();

    assert!(!categories.is_empty());

    if !(categories.len() == 1 && categories.contains(&Category::Decision)
        || categories.len() == 2
            && categories.contains(&Category::Decision)
            && categories.contains(&Category::Constant))
    {
        if let Expr::ElementId(_, _, value) = expr {
            let value_categories = value.universe_categories();
            if !(value_categories.len() == 1 && value_categories.contains(&Category::Decision)
                || value_categories.len() == 2
                    && value_categories.contains(&Category::Decision)
                    && value_categories.contains(&Category::Constant))
            {
                if cfg!(debug_assertions) {
                    trace!(
                        why =
                            "expression has sub-expressions that are not in the decision category",
                        "to_aux_var() failed"
                    );
                }
                return None;
            }
        } else {
            if cfg!(debug_assertions) {
                trace!(
                    why = "expression has sub-expressions that are not in the decision category",
                    "to_aux_var() failed"
                );
            }
            return None;
        }
    }

    // Avoid introducing auxvars for generic matrix indexing (can create many redundant auxvars
    // before comprehension expansion). However, keep list indexing eligible so Minion lowering
    // can introduce `element` constraints in non-equality contexts.
    if let Expr::SafeIndex(_, subject, indices) = expr {
        let index_has_element_id = indices
            .iter()
            .any(|index| matches!(index, Expr::ElementId(..)));
        let can_lower_via_element = subject.clone().unwrap_list().is_some()
            && indices.iter().all(|i| matches!(i, Expr::Atomic(_, _)));

        if !can_lower_via_element && !index_has_element_id {
            if cfg!(debug_assertions) {
                trace!(expr=%expr, why = "matrix indexing is not element-lowerable", "to_aux_var() failed");
            }
            return None;
        }
    }

    let Some(domain) = expr.domain_of() else {
        if cfg!(debug_assertions) {
            trace!(expr=%expr, why = "could not find the domain of the expression", "to_aux_var() failed");
        }
        return None;
    };

    Some(domain)
}

fn materialise_aux_var(expr: &Expr, symbols: &SymbolTable, domain: &DomainPtr) -> ToAuxVarOutput {
    let mut symbols = symbols.clone();
    let decl = symbols.gen_find(domain);

    if cfg!(debug_assertions) {
        trace!(expr=%expr, "to_auxvar() succeeded in putting expr into an auxvar");
    }

    ToAuxVarOutput {
        aux_declaration: decl.clone(),
        aux_expression: Expr::AuxDeclaration(
            Metadata::new(),
            conjure_cp::ast::Reference::new(decl),
            Moo::new(expr.clone()),
        ),
        symbols,
        _unconstructable: (),
    }
}

/// Defers auxiliary variable allocation until a selected rule is materialised.
pub fn defer_aux_var(
    expr: &Expr,
    build: impl Fn(ToAuxVarOutput) -> RuleEffect + Send + Sync + 'static,
) -> Option<RuleEffect> {
    let domain = to_aux_var_domain(expr)?;
    let expr = expr.clone();

    Some(RuleEffect::deferred(move |symbols| {
        let aux = materialise_aux_var(&expr, symbols, &domain);
        build(aux)
    }))
}

/// Output data of `to_aux_var`.
pub struct ToAuxVarOutput {
    aux_declaration: DeclarationPtr,
    aux_expression: Expr,
    symbols: SymbolTable,
    _unconstructable: (),
}

impl ToAuxVarOutput {
    /// Returns the new auxiliary variable as an `Atom`.
    pub fn as_atom(&self) -> Atom {
        Atom::Reference(conjure_cp::ast::Reference::new(
            self.aux_declaration.clone(),
        ))
    }

    /// Returns the new auxiliary variable as an `Expression`.
    ///
    /// This expression will have default `Metadata`.
    pub fn as_expr(&self) -> Expr {
        Expr::Atomic(Metadata::new(), self.as_atom())
    }

    /// Returns the top level `Expression` to add to the model.
    pub fn top_level_expr(&self) -> Expr {
        self.aux_expression.clone()
    }

    /// Returns the new `SymbolTable`, modified to contain this auxiliary variable in the symbol table.
    pub fn symbols(&self) -> SymbolTable {
        self.symbols.clone()
    }
}

/// Clone comprehension with expression generator into its own detached comprehension scope
/// and rewrite all uses of the original quantified declaration to a fresh branch-local
/// expression generator.
pub fn replace_expression_generator_source(
    comp: &Comprehension,
    gen_decl: &DeclarationPtr,
    replacement_expr: Expr,
) -> (Comprehension, DeclarationPtr) {
    let replacement_ptr =
        DeclarationPtr::new_quantified_expr(gen_decl.name().clone(), replacement_expr);
    let mut comprehension = comp.clone();

    // detach the scope so rewriting this branch does not mutate the original
    // comprehension through shared pointers
    comprehension.symbols = comprehension.symbols.detach();

    // rewrite all uses of the original quantified declaration to the branch-local
    // generator declaration
    comprehension.return_expression =
        comprehension
            .return_expression
            .transform_bi(&|decl: DeclarationPtr| {
                if decl == *gen_decl {
                    replacement_ptr.clone()
                } else {
                    decl
                }
            });

    comprehension.qualifiers = comprehension
        .qualifiers
        .into_iter()
        .map(|qualifier| {
            qualifier.transform_bi(&|decl: DeclarationPtr| {
                if decl == *gen_decl {
                    replacement_ptr.clone()
                } else {
                    decl
                }
            })
        })
        .collect();

    // keep the detached local scope in sync with the rewritten generator
    // declarations used by this branch
    comprehension
        .symbols
        .write()
        .update_insert(replacement_ptr.clone());
    for qualifier in &comprehension.qualifiers {
        match qualifier {
            ComprehensionQualifier::ExpressionGenerator { ptr }
            | ComprehensionQualifier::Generator { ptr } => {
                comprehension.symbols.write().update_insert(ptr.clone());
            }
            ComprehensionQualifier::Condition(_) => {}
        }
    }

    (comprehension, replacement_ptr)
}
