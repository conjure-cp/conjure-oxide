use conjure_core::{
    ast::{Atom, Domain, Expression as Expr, Name, SymbolTable},
    metadata::Metadata,
};

use tracing::instrument;
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
#[instrument]
pub fn to_aux_var(expr: &Expr, symbols: &SymbolTable) -> Option<ToAuxVarOutput> {
    let mut symbols = symbols.clone();

    // No need to put an atom in an aux_var
    if is_atom(expr) {
        return None;
    }

    // Anything that should be bubbled, bubble
    if !expr.is_safe() {
        return None;
    }

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
        return None;
    }

    let Some(domain) = expr.domain_of(&symbols) else {
        tracing::trace!("could not find domain of {}", expr);
        return None;
    };

    let decl = symbols.gensym(&domain);
    let name = (*decl).borrow().name().clone();

    Some(ToAuxVarOutput {
        aux_name: name.clone(),
        aux_decl: Expr::AuxDeclaration(Metadata::new(), name, Box::new(expr.clone())),
        aux_domain: domain,
        symbols,
        _unconstructable: (),
    })
}

/// Output data of `to_aux_var`.
pub struct ToAuxVarOutput {
    aux_name: Name,
    aux_decl: Expr,
    #[allow(dead_code)] // TODO: aux_domain should be used soon, try removing this pragma
    aux_domain: Domain,
    symbols: SymbolTable,
    _unconstructable: (),
}

impl ToAuxVarOutput {
    /// Returns the new auxiliary variable as an `Atom`.
    pub fn as_atom(&self, symbols: &SymbolTable) -> Atom {
        let decl = symbols.lookup(&self.aux_name).unwrap();
        Atom::Reference(self.aux_name(), decl)
    }

    /// Returns the new auxiliary variable as an `Expression`.
    ///
    /// This expression will have default `Metadata`.
    pub fn as_expr(&self, symbols: &SymbolTable) -> Expr {
        Expr::Atomic(Metadata::new(), self.as_atom(symbols))
    }

    /// Returns the top level `Expression` to add to the model.
    pub fn top_level_expr(&self) -> Expr {
        self.aux_decl.clone()
    }

    /// Returns the new `SymbolTable`, modified to contain this auxiliary variable in the symbol table.
    pub fn symbols(&self) -> SymbolTable {
        self.symbols.clone()
    }

    /// Returns the name of the auxiliary variable.
    pub fn aux_name(&self) -> Name {
        self.aux_name.clone()
    }
}
