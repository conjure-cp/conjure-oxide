#[doc(hidden)]
pub extern crate self as conjure_cp_core;

#[doc(hidden)]
pub use ast::Model;

pub mod ast;

// NOTE: this module defines the bug! macro, which is exported at the crate level, and has no other
// contents.
mod bug;

pub mod context;
pub mod error;
pub mod parse;
pub mod representation;
pub mod rule_engine;
pub mod solver;
pub mod stats;

/// Creates a [`Domain::Int`](ast::Domain::Int).
///
/// # Examples
/// ```
/// use conjure_cp_core::{domain_int,range,ast::Domain};
///
/// let a = 2*10;
/// assert_eq!(domain_int!(1..5,a+2,), Domain::Int(vec![range!(1..5),range!(a+2)]));
/// assert_eq!(domain_int!(), Domain::Int(vec![]));
/// ```
#[macro_export]
macro_rules! domain_int {
    () => {$crate::ast::Domain::Int(vec![])};

    // when parsing expressions, rust groups 1..2 into a single token tree, (1..2)
    // however, we want it to be three seperate token trees [1,..,2] for parsing.
    // use defile to turn it back into 3 token trees
    ($($e:expr),+ $(,)?) => {::defile::defile! { $crate::ast::Domain::Int(vec![$($crate::range!(@$e)),+]) } };
}

/// Creates a [`Range`](ast::Range).
///
/// # Examples
///
/// ```
/// use conjure_cp_core::{range,ast::Range};
///
/// let a = 2*10;
/// assert_eq!(range!(..a),Range::UnboundedL(a));
/// assert_eq!(range!(2*5..),Range::UnboundedR(10));
/// assert_eq!(range!(..10),Range::UnboundedL(10));
/// assert_eq!(range!(1..5),Range::Bounded(1,5));
/// assert_eq!(range!(Some(10).unwrap()),Range::Single(10));
/// ```
#[macro_export]
macro_rules! range {
    // decl macros have no lookahead, hence this nonsense with pushdown automata.

    // @hasLowerbound: have atleast one token of the lower bound on the stack
    (@hasLowerBound [$($lower:tt)+] -> ..) => {$crate::ast::Range::UnboundedR($crate::as_expr!($($lower)+))};
    (@hasLowerBound [$($lower:tt)+] -> .. $($tail:tt)+) => {$crate::ast::Range::Bounded($crate::as_expr!($($lower)+),$crate::as_expr!($($tail)+))};
    (@hasLowerBound [$($lower:tt)+] -> $b:tt $($tail:tt)*) => {range!(@hasLowerBound [$($lower)+ $b] -> $($tail)*)};
    (@hasLowerBound [$($lower:tt)+] ->) => {$crate::ast::Range::Single($crate::as_expr!($($lower)+))};

    // initial tokens
    (.. $($a:tt)+) => {$crate::ast::Range::UnboundedL($crate::as_expr!($($a)+))};

    ($a:tt $($tail:tt)*) => {range!(@hasLowerBound [$a] -> $($tail)*)};

}

// coorce a tt fragment into a expr fragment
// https://lukaswirth.dev/tlborm/decl-macros/building-blocks/ast-coercion.html
#[macro_export]
#[doc(hidden)]
macro_rules! as_expr {
    ($e:expr) => {
        $e
    };
}
