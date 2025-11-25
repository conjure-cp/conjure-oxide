use proc_macro::TokenStream;
use proc_macro2::{Delimiter, Group, TokenStream as TokenStream2, TokenTree};

mod expand;

use expand::{expand_expr, expand_expr_vec};

/// Parses an Essence expression into its corresponding Conjure AST at compile time.
///
/// ## Input
/// The input can be one of the following:
/// - The raw Essence tokens (`essence_expr!(2 + 2)`)
/// - A string literal (`essence_expr!("2 + 2")`)
///
/// The macro may reference variables in the current scope (called "metavars")
/// using the syntax `&<name>`. For example:
/// ```rust
/// use conjure_cp_essence_macros::essence_expr;
/// let x = 42;
/// let expr = essence_expr!(2 + &x);
/// ```
///
///
/// ## Expansion
/// If the input is valid Essence, expands to a valid AST constructor
///
/// ## Note
/// Some characters (e.g. `\`) are valid Essence tokens, but not Rust tokens.
/// If you encounter an error similar to:
///
/// > rustc: unknown start of token: \
///
/// The workaround is to wrap the Essence code in a string literal (e.g. `r"a /\ b"`).
///
/// ## Example
///
/// ```rust
/// use conjure_cp::ast::{Atom, Expression, Moo, Metadata};
/// use conjure_cp::matrix_expr;
/// use conjure_cp_essence_macros::essence_expr;
/// let x = 42;
/// let expr = essence_expr!(2 + &x);
/// assert_eq!(
///     expr,
///     Expression::Sum(Metadata::new(), Moo::new(matrix_expr![
///         Expression::Atomic(Metadata::new(), 2.into()),
///         Expression::Atomic(Metadata::new(), 42.into())
///     ]))
/// );
/// ```
#[proc_macro]
pub fn essence_expr(args: TokenStream) -> TokenStream {
    let ts = TokenStream2::from(args);
    let tt = TokenTree::Group(Group::new(Delimiter::None, ts));
    match expand_expr(&tt) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Parses a sequence of Essence expressions into a vector of Conjure AST instances
///
/// ## Example
/// ```rust
/// use conjure_cp::ast::{Atom, Expression, Moo, Metadata};
/// use conjure_cp::matrix_expr;
/// use conjure_cp_essence_macros::essence_vec;
///
/// let exprs = essence_vec!(2 + 2, false = true);
/// println!("{:?}", exprs);
/// assert_eq!(exprs.len(), 2);
/// assert_eq!(
///     exprs[0],
///     Expression::Sum(Metadata::new(), Moo::new(matrix_expr![
///         Expression::Atomic(Metadata::new(), 2.into()),
///         Expression::Atomic(Metadata::new(), 2.into())
///     ]))
/// );
/// assert_eq!(
///    exprs[1],
///     Expression::Eq(Metadata::new(),
///         Moo::new(Expression::Atomic(Metadata::new(), false.into())),
///         Moo::new(Expression::Atomic(Metadata::new(), true.into()))
///     )
/// );
/// ```
#[proc_macro]
pub fn essence_vec(args: TokenStream) -> TokenStream {
    let ts = TokenStream2::from(args);
    let tt = TokenTree::Group(Group::new(Delimiter::None, ts));
    match expand_expr_vec(&tt) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}
