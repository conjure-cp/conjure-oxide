pub mod pretty;
pub mod serde;

mod atom;
mod declaration;
mod domains;
mod expressions;
mod literals;
mod model;
mod name;
mod submodel;
mod symbol_table;
mod types;
mod variables;

pub use atom::Atom;
pub use declaration::*;
pub use domains::Domain;
pub use domains::Range;
pub use expressions::Expression;
pub use literals::Literal;
pub use model::*;
pub use name::Name;
pub use submodel::SubModel;
pub use symbol_table::SymbolTable;
pub use types::*;
pub use variables::DecisionVariable;

/// Creates a new vector literal expression (`Expression::VecLit`)
///
/// Syntax is the same as the [`vec!`] macro.
#[macro_export]
macro_rules! vec_lit {
    // copied from the std vec! macro
    () => (
        $crate::ast::Expression::VecLit($crate::metadata::Metadata::new(),std::vec::Vec::new())
    );

    ($elem:expr; $n:expr) => (
        $crate::ast::Expression::VecLit($crate::metadata::Metadata::new(),std::vec::from_elem($elem, $n))
    );

    ($x:expr) => (
        $crate::ast::Expression::VecLit($crate::metadata::Metadata::new(),vec![$x])
    );
    ($($x:expr),*) => (
        $crate::ast::Expression::VecLit($crate::metadata::Metadata::new(),vec![$($x),*])
    );
    ($($x:expr,)*) => (
        $crate::vec_lit![$($x),*]
    )
}

/// Creates a new boxed vector literal expression (`Expression::VecLit`)
///
/// Syntax is the same as the [`vec!`] macro.
#[macro_export]
macro_rules! boxed_vec_lit {
    ($($x:tt)*) => (
            std::boxed::Box::new($crate::vec_lit![$($x)*])
    );

}
/// Creates a new vector literal expression (`Expression::VecLit`) from an input vector.
///
/// ```
/// # use crate::ast::Expression;
/// # use crate::ast::Atomic;
/// let a: Vec<Expression> = vec![1.into(),2.into(),3.into()];
/// let e: Expression = into_vec_lit![a];
/// ```
#[macro_export]
macro_rules! into_vec_lit {
    ($x: expr) => {
        $crate::ast::Expression::VecLit($crate::metadata::Metadata::new(), $x)
    };
}

/// Creates a new Boxed vector literal expression (`Expression::VecLit`) from an input vector.
///
/// ```
/// # use crate::ast::Expression;
/// # use crate::ast::Atomic;
/// let a: Vec<Expression> = vec![1.into(),2.into(),3.into()];
/// let e: Box<Expression> = into_boxed_vec_lit![a];
/// ```
#[macro_export]
macro_rules! into_boxed_vec_lit {
    ($x: expr) => {
        std::boxed::Box::new($crate::into_vec_lit![$x])
    };
}
