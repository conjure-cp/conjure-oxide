use conjure_cp::{
    ast::Metadata,
    ast::{Atom, DeclarationPtr, Expression as Expr, Moo, SymbolTable, categories::Category},
};

use tracing::{instrument, trace};
use uniplate::{Biplate, Uniplate};

/// True iff `expr` is an `Atom`.
pub fn is_atom(expr: &Expr) -> bool {
    matches!(expr, Expr::Atomic(_, _))
}

/// True if `expr` is flat; i.e. it only contains atoms.
pub fn is_flat(expr: &Expr) -> bool {
    for e in expr.children() {
        if !is_atom(&e) {
            return false;
        }
    }
    true
}

/// True if the entire AST is constants.
pub fn is_all_constant(expression: &Expr) -> bool {
    for atom in expression.universe_bi() {
        match atom {
            Atom::Literal(_) => {}
            _ => {
                return false;
            }
        }
    }

    true
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
        if cfg!(debug_assertions) {
            trace!(
                why = "expression has sub-expressions that are not in the decision category",
                "to_aux_var() failed"
            );
        }
        return None;
    }

    // FIXME: why does removing this make tests fail!
    //
    // do not put matrix[e] in auxvar
    //
    // eventually this will rewrite into an indomain constraint, or a single variable.
    //
    // To understand why deferring this until a lower level constraint is chosen is good, consider
    // the comprehension:
    //
    // and([m[i] = i + 1  | i: int(1..5)])
    //
    // Here, if we rewrite inside the comprehension, we will end up making the auxvar
    // __0  = m[i].
    //
    // When we expand the matrix, this will expand to:
    //
    // __0 = m[1]
    // __1 = m[2]
    // __2 = m[3]
    // __3 = m[4]
    // __4 = m[5]
    //
    //
    // These all rewrite to variable references (e.g. m[1] ~> m#matrix_to_atom_1), so these auxvars
    // are redundant. However, we don't know this before expanding, as they are just m[i].
    //
    // In the future, we can do this more fine-grained using categories (e.g. only flatten matrices
    // indexed by expressions with the decision variable category) : however, doing this for
    // all matrix indexing is fine, as they can be rewritten into a lower-level expression, then
    // flattened.
    if let Expr::SafeIndex(_, _, _) = expr {
        if cfg!(debug_assertions) {
            trace!(expr=%expr, why = "expression is an matrix indexing operation", "to_aux_var() failed");
        }
        return None;
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
        aux_expression: Expr::AuxDeclaration(Metadata::new(), decl, Moo::new(expr.clone())),
        symbols,
        _unconstructable: (),
    })
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
        Atom::Reference(self.aux_declaration.clone())
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
