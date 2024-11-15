use tracing::instrument;
use uniplate::{Biplate, Uniplate};

use crate::{
    ast::{DecisionVariable, Domain, Expression as Expr, Factor, Name, ReturnType},
    bug,
    metadata::Metadata,
    Model,
};

/// True iff `expr` is a `Factor`.
pub fn is_factor(expr: &Expr) -> bool {
    matches!(expr, Expr::FactorE(_, _))
}

/// True if `expr` is flat; i.e. it only contains factors.
pub fn is_flat(expr: &Expr) -> bool {
    for e in expr.children() {
        if !is_factor(&e) {
            return false;
        }
    }
    true
}

/// True if the entire AST is constants.
pub fn is_all_constant(expression: &Expr) -> bool {
    for factor in <Expr as Biplate<Factor>>::universe_bi(expression) {
        match factor {
            Factor::Literal(_) => {}
            Factor::Reference(_) => {
                return false;
            }
        }
    }

    true
}

/// Creates a single `Expr` from a `Vec<Expr>` by forming the conjunction.
///
/// If it contains a single element, that element is returned, otherwise the conjunction of the
/// elements is returned.
///
/// # Returns
///
/// - Some: the expression list is non empty, and all expressions are booleans (so can be
/// conjuncted).
///
/// - None: the expression list is empty, or not all expressions are booleans.
pub fn exprs_to_conjunction(exprs: &Vec<Expr>) -> Option<Expr> {
    match exprs.as_slice() {
        [] => None,
        [a] if a.return_type() == Some(ReturnType::Bool) => Some(a.clone()),
        [_, _, ..] => {
            for expr in exprs {
                if expr.return_type() != Some(ReturnType::Bool) {
                    return None;
                }
            }
            Some(Expr::And(Metadata::new(), exprs.clone()))
        }
        _ => None,
    }
}

/// Creates a new auxiliary variable using the given expression.
///
/// # Returns
///
/// * `None` if `Expr` is a `Factor`, or `Expr` does not have a domain (for example, if it is a `Bubble`).
///
/// * `Some(ToAuxVarOutput)` if successful, containing:
///     
///     + A new model, modified to include the auxiliary variable in the symbol table.
///     + A new top level expression, containing the declaration of the auxiliary variable.
///     + A reference to the auxiliary variable to replace the existing expression with.
///
#[instrument]
pub fn to_aux_var(expr: &Expr, m: &Model) -> Option<ToAuxVarOutput> {
    let mut m = m.clone();

    // No need to put a factor in an aux_var
    if is_factor(expr) {
        return None;
    }

    // Anything that should be bubbled, bubble
    if (expr.can_be_undefined()) {
        return None;
    }

    let name = m.gensym();

    let Some(domain) = expr.domain_of(&m.variables) else {
        tracing::trace!("could not find domain of {}", expr);
        return None;
    };

    m.add_variable(name.clone(), DecisionVariable::new(domain.clone()));

    Some(ToAuxVarOutput {
        aux_name: name.clone(),
        aux_decl: Expr::AuxDeclaration(Metadata::new(), name, Box::new(expr.clone())),
        aux_domain: domain,
        new_model: m,
        _unconstructable: (),
    })
}

/// Output data of `to_aux_var`.
pub struct ToAuxVarOutput {
    aux_name: Name,
    aux_decl: Expr,
    aux_domain: Domain,
    new_model: Model,
    _unconstructable: (),
}

impl ToAuxVarOutput {
    /// Returns the new auxiliary variable as a `Factor`.
    pub fn as_factor(&self) -> Factor {
        Factor::Reference(self.aux_name())
    }

    /// Returns the new auxiliary variable as an `Expression`.
    ///
    /// This expression will have default `Metadata`.
    pub fn as_expr(&self) -> Expr {
        Expr::FactorE(Metadata::new(), self.as_factor())
    }

    /// Returns the top level `Expression` to add to the model.
    pub fn top_level_expr(&self) -> Expr {
        self.aux_decl.clone()
    }

    /// Returns the new `Model`, modified to contain this auxiliary variable in the symbol table.
    ///
    /// Like `Reduction`, this new model does not include the new top level expression. To get
    /// this, use [`top_level_expr()`](`ToAuxVarOutput::top_level_expr()`).
    pub fn model(&self) -> Model {
        self.new_model.clone()
    }

    /// Returns the name of the auxiliary variable.
    pub fn aux_name(&self) -> Name {
        self.aux_name.clone()
    }
}
