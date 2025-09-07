use proc_macro::TokenStream;
use proc_macro2::{Delimiter, Group, TokenStream as TokenStream2, TokenTree};

mod expand;
mod expression;

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
/// ```
/// use conjure_cp_essence_macros::essence_expr;
/// let x = 42;
/// essence_expr!(2 + &x);
/// ```
///
/// ## Expansion
/// If the input is valid Essence, expands to an Expression constructor
///
///## Limitations
/// The expression cannot reference Essence variables by name.
/// This is because variables are tied to a Declaration within a particular Essence scope,
/// and we don't have this information inside the macro.
/// However, you can capture expressions that already exist in the current scope as metavars.
///
/// This won't work (how do we know what 'x' is?)
/// ```compile_fail
/// use conjure_cp_essence_macros::essence_expr;
/// essence_expr!(2 + x);
/// ```
///
/// This will work:
/// ```rust
/// use conjure_cp_essence_macros::essence_expr;
/// use conjure_cp::ast::{DeclarationPtr, Domain, Range, Name, SymbolTable};
///
/// // Set up the symbol table and add the variable `x` to it
/// let mut symbol_table = SymbolTable::new();
/// let declaration = DeclarationPtr::new_var(
///     Name::User("x".into()),
///     Domain::Int(vec![Range::Bounded(1,5)])
/// );
/// symbol_table.insert(declaration).unwrap();
///
/// // Retrieve the declaration pointer for `x`
/// let x = symbol_table.lookup(&Name::user("x")).unwrap();
///
/// // Reference `x` in the macro
/// essence_expr!(2 + &x);
/// ```
///
/// ## Note
///
/// Some characters (e.g. `\`) are valid Essence tokens, but not Rust tokens.
/// If you encounter an error similar to:
///
/// > rustc: unknown start of token: \
///
/// The workaround is to wrap the Essence code in a string literal (e.g. `r"&a /\ &b"`).
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
/// use conjure_cp_essence_macros::{essence_expr, essence_vec};
///
/// let a = essence_expr!(2);
/// let b = essence_expr!(true);
/// let exprs = essence_vec!(&a + 2, &b = true);
/// println!("{:?}", exprs);
/// assert_eq!(exprs.len(), 2);
/// assert_eq!(
///     exprs[0],
///     Expression::Sum(Metadata::new(), Moo::new(matrix_expr![
///         a,
///         Expression::Atomic(Metadata::new(), 2.into())
///     ]))
/// );
/// assert_eq!(
///    exprs[1],
///     Expression::Eq(Metadata::new(),
///         Moo::new(b),
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
