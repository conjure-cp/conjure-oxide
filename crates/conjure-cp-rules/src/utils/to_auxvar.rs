use crate::utils::is_atom;
use conjure_cp::ast::categories::Category;
use conjure_cp::ast::{Atom, DeclarationPtr, Expression, Metadata, Moo, SymbolTable};
use tracing::{instrument, trace};

/// Creates a new auxiliary variable using the given expression.
///
/// # Returns
///
/// * `None` if `Expression` is a `Atom`, or `Expression` does not have a domain (for example, if it is a `Bubble`).
///
/// * `Some(ToAuxVarOutput)` if successful, containing:
///
///     + A new symbol table, modified to include the auxiliary variable.
///     + A new top level expression, containing the declaration of the auxiliary variable.
///     + A reference to the auxiliary variable to replace the existing expression with.
///
#[instrument(skip_all, fields(expr = %expr))]
pub fn to_aux_var(expr: &Expression, symbols: &SymbolTable) -> Option<ToAuxVarOutput> {
    let mut symbols = symbols.clone();

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
    if let Expression::AbstractLiteral(_, _) = expr {
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
        if cfg!(debug_assertions) {
            trace!(
                why = "expression has sub-expressions that are not in the decision category",
                "to_aux_var() failed"
            );
        }
        return None;
    }

    // Avoid introducing auxvars for generic matrix indexing (can create many redundant auxvars
    // before comprehension expansion). However, keep list indexing eligible so Minion lowering
    // can introduce `element` constraints in non-equality contexts.
    if let Expression::SafeIndex(_, subject, indices) = expr {
        let can_lower_via_element = subject.clone().unwrap_list().is_some()
            && indices
                .iter()
                .all(|i| matches!(i, Expression::Atomic(_, _)));

        if !can_lower_via_element {
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

    let decl = symbols.gensym(&domain);

    if cfg!(debug_assertions) {
        trace!(expr=%expr, "to_auxvar() succeeded in putting expr into an auxvar");
    }

    Some(ToAuxVarOutput {
        aux_declaration: decl.clone(),
        aux_expression: Expression::AuxDeclaration(
            Metadata::new(),
            conjure_cp::ast::Reference::new(decl),
            Moo::new(expr.clone()),
        ),
        symbols,
        _unconstructable: (),
    })
}

/// Output data of `to_aux_var`.
pub struct ToAuxVarOutput {
    aux_declaration: DeclarationPtr,
    aux_expression: Expression,
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

    /// Returns the new auxiliary variable as an `Expressionession`.
    ///
    /// This expression will have default `Metadata`.
    pub fn as_expr(&self) -> Expression {
        Expression::Atomic(Metadata::new(), self.as_atom())
    }

    /// Returns the top level `Expressionession` to add to the model.
    pub fn top_level_expr(&self) -> Expression {
        self.aux_expression.clone()
    }

    /// Returns the new `SymbolTable`, modified to contain this auxiliary variable in the symbol table.
    pub fn symbols(&self) -> SymbolTable {
        self.symbols.clone()
    }
}
