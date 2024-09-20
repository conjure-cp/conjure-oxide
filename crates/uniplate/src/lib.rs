//! Uniplate provides simple and low-boilerplate ways to traverse and manipulate data structures.
//! A port of Haskell's [Uniplate](https://hackage.haskell.org/package/uniplate) in Rust.
//!
//!
//! # Getting Started
//!
//! *Adapted from (Mitchell and Runciman 2009)*
//!
//! Consider the abstract syntax tree for a simple calculator language:
//!
//! ```rust
//! enum Expr {
//!     Add(Box<Expr>, Box<Expr>),
//!     Sub(Box<Expr>, Box<Expr>),
//!     Mul(Box<Expr>, Box<Expr>),
//!     Div(Box<Expr>, Box<Expr>),
//!     Val(i32),
//!     Var(String),
//!     Neg(Box<Expr>),
//! }
//! ```
//!
//! Say we want to list all the used variable names inside a given expression:
//!
//! ```rust
//! # use uniplate::test_common::paper::Expr::*;
//! # use uniplate::test_common::paper::Expr;
//! fn vars(expr: &Expr) -> Vec<String>{
//!     match expr {
//!         Add(a,b) => {
//!             [vars(a),vars(b)].concat()
//!         },
//!         Sub(a,b) => {
//!             [vars(a),vars(b)].concat()
//!         },
//!         Mul(a,b) => {
//!             [vars(a),vars(b)].concat()
//!         },
//!         Div(a,b) => {
//!             [vars(a),vars(b)].concat()
//!         },
//!         Val(a) => {
//!             Vec::new()
//!         },
//!         Var(a) => {
//!             vec![a.clone()]
//!         },
//!         Neg(a) =>{
//!             vars(a)
//!         }
//!     }
//! }
//! ```
//!
//! Functions like these are annoying to write: the first 4 constructors are basically identical,
//! adding a new expression type requires a new line to be added to all match statement, and this
//! code cannot be shared with similar functions (e.g. one that change all the variable names).
//!
//!
//! With Uniplate, this boilerplate can be eliminated:
//!
//! ```rust
//! # use uniplate::test_common::paper::Expr::*;
//! # use uniplate::test_common::paper::Expr;
//! use uniplate::biplate::Biplate;
//! fn vars(expr: &Expr) -> Vec<String>{
//!     <Expr as Biplate<String>>::universe_bi(expr).into_iter().collect()
//! }
//! ```
//!
//! The functionality of Uniplate comes from two main traits: [`Uniplate`](biplate::Uniplate) and
//! [`Biplate<T>`](biplate::Biplate).
//!
//! * The [`Uniplate`](biplate::Uniplate) of `Expr` operates over all nested `Expr`s.
//! * The [`Biplate<T>`](biplate::Biplate) of `Expr` operates over all nested values of some given type `T` within the
//!   expression tree.
//!
//! These traits provide traversal operations (e.g. [`children`](Uniplate::children)) as well as
//! functional programming constructs such as [`map`](Uniplate::map) and [`fold`](Uniplate::fold).
//! See the trait documentation for the full list of operations provided.
//!
//! The easiest way to use Uniplate is with the derive macro.
//!
//! ## Derive Macro
//!
//! When no arguments are provided, the macro derives a Uniplate instance:
//!
//! ```rust
//! use uniplate::Uniplate;
//! #[derive(Clone,PartialEq,Eq,Debug,Uniplate)]
//! enum Expr {
//!     Add(Box<Expr>, Box<Expr>),
//!     Sub(Box<Expr>, Box<Expr>),
//!     Mul(Box<Expr>, Box<Expr>),
//!     Div(Box<Expr>, Box<Expr>),
//!     Val(i32),
//!     Var(String),
//!     Neg(Box<Expr>),
//! }
//! ```
//!
//! To derive Biplate instances, use the `#[biplate]` attribute:
//!
//! ```rust
//! use uniplate::Uniplate;
//! #[derive(Clone,PartialEq,Eq,Debug,Uniplate)]
//! #[biplate(to=String)]
//! #[biplate(to=i32)]
//! enum Expr {
//!     Add(Box<Expr>, Box<Expr>),
//!     Sub(Box<Expr>, Box<Expr>),
//!     Mul(Box<Expr>, Box<Expr>),
//!     Div(Box<Expr>, Box<Expr>),
//!     Val(i32),
//!     Var(String),
//!     Neg(Box<Expr>),
//! }
//! ```
//!
//! ## Multi-type traversals
//!
//! Lets expand our calculator language to include statements as well as expressions:
//!
//! ```rust
//! enum Expr {
//!     Add(Box<Expr>, Box<Expr>),
//!     Sub(Box<Expr>, Box<Expr>),
//!     Mul(Box<Expr>, Box<Expr>),
//!     Div(Box<Expr>, Box<Expr>),
//!     Val(i32),
//!     Var(String),
//!     Neg(Box<Expr>),
//! }
//!
//! enum Stmt {
//!     Assign(String, Expr),
//!     Sequence(Vec<Stmt>),
//!     If(Expr, Box<Stmt>, Box<Stmt>),
//!     While(Expr, Box<Stmt>),
//! }
//! ```
//!
//! When looking for `Strings` in a given `Stmt`, we may want to identify not only the strings
//! directly contained within a `Stmt`, but also any strings contained inside an `Expr` inside a
//! `Stmt`.
//!
//! For example:
//!
//! ```
//! # use uniplate::test_common::paper::Expr::*;
//! # use uniplate::test_common::paper::Expr::*;
//! # use uniplate::test_common::paper::Stmt;
//! # use uniplate::test_common::paper::Stmt::*;
//! use uniplate::biplate::Biplate;
//!
//! let stmt = Assign("x".into(), Add(Box::new(Var("y".into())),Box::new(Val(10))));
//! let strings = <Stmt as Biplate<String>>::universe_bi(&stmt);
//!
//! assert!(strings.contains(&"x".into()));
//!
//! // Despite being inside an Expr::String, "y" is found by Biplate
//! assert!(strings.contains(&"y".into()));
//!
//! assert_eq!(strings.len(), 2);
//! ```
//!
//!
//! To do this, a list of types to "walk into" can be given as an argument to the Biplate
//! declaration:
//! ```rust
//! use uniplate::Uniplate;
//! #[derive(Clone,PartialEq,Eq,Debug,Uniplate)]
//! #[uniplate()]
//! #[biplate(to=String,walk_into=[Expr])]
//! #[biplate(to=Stmt)]
//! enum Expr {
//!     Add(Box<Expr>, Box<Expr>),
//!     Sub(Box<Expr>, Box<Expr>),
//!     Mul(Box<Expr>, Box<Expr>),
//!     Div(Box<Expr>, Box<Expr>),
//!     Val(i32),
//!     Var(String),
//!     Neg(Box<Expr>),
//! }
//!
//! // Uniplate also supports walk_into.
//! // In this case, it doesn't do much.
//! #[derive(Clone,PartialEq,Eq,Debug,Uniplate)]
//! #[biplate(to=String, walk_into=[Expr])]
//! #[biplate(to=Expr,walk_into=[Expr])]
//! #[uniplate(walk_into=[Expr])]
//! enum Stmt {
//!     Assign(String, Expr),
//!     Sequence(Vec<Stmt>),
//!     If(Expr, Box<Stmt>, Box<Stmt>),
//!     While(Expr, Box<Stmt>),
//! }
//! ```
//!
//!
//! # Bibliography
//!
//! The techniques implemented in this crate originate from the following:
//!  
//! * [The Uniplate Haskell Library](https://hackage.haskell.org/package/uniplate).
//!
//! * Neil Mitchell and Colin Runciman. 2007. Uniform boilerplate and list processing. In
//! Proceedings of the ACM SIGPLAN workshop on Haskell workshop (Haskell '07). Association for
//! Computing Machinery, New York, NY, USA, 49–60. <https://doi.org/10.1145/1291201.1291208>
//! [(free copy)](https://www.cs.york.ac.uk/plasma/publications/pdf/MitchellRuncimanHW07.pdf)
//!
//! * Huet G. The Zipper. Journal of Functional Programming. 1997;7(5):549–54. <https://doi.org/10.1017/S0956796897002864>
//! [(free copy)](https://www.cambridge.org/core/services/aop-cambridge-core/content/view/0C058890B8A9B588F26E6D68CF0CE204/S0956796897002864a.pdf/zipper.pdf)

pub mod biplate;
pub mod impls;
mod tree;
//pub mod uniplate;

pub use tree::Tree;

#[doc(hidden)]
pub mod test_common;

pub use uniplate_derive::*;

extern crate self as uniplate;

/// Generates a Biplate and Uniplate instance for an unplateable type.
#[macro_export]
macro_rules! derive_unplateable {
    ($t:ty) => {
        impl Uniplate for $t {
            fn uniplate(&self) -> (Tree<Self>, Box<dyn Fn(Tree<Self>) -> Self>) {
                let val = self.clone();
                (::uniplate::Tree::Zero, Box::new(move |_| val.clone()))
            }
        }

        impl Biplate<$t> for $t {
            fn biplate(&self) -> (Tree<$t>, Box<dyn Fn(Tree<$t>) -> $t>) {
                let val = self.clone();
                (
                    ::uniplate::Tree::One(val.clone()),
                    Box::new(move |_| val.clone()),
                )
            }
        }
    };
}

// Generates a Biplate and Uniplate instance for an iterable type.
#[macro_export]
macro_rules! derive_iter {
    ($iter_ty:ident) => {
        impl<T, F> Biplate<T> for $iter_ty<F>
        where
            T: Clone + Eq + Uniplate + Sized + 'static,
            F: Clone + Eq + Uniplate + Biplate<T> + Sized + 'static,
        {
            fn biplate(&self) -> (Tree<T>, Box<(dyn Fn(Tree<T>) -> $iter_ty<F>)>) {
                if (self.is_empty()) {
                    let val = self.clone();
                    return (Tree::Zero, Box::new(move |_| val.clone()));
                }

                // this is an example of the special biplate case discussed in the paper.
                // T == F: return all types F in the Vector.
                if std::any::TypeId::of::<T>() == std::any::TypeId::of::<F>() {
                    unsafe {
                        // need to cast from F to T
                        let children: Tree<T> = Tree::Many(
                            self.clone()
                                .into_iter()
                                .map(|x: F| {
                                    // possibly unsafe, definitely stupid, but seems to be the only way here?
                                    let x: T = std::mem::transmute::<&F, &T>(&x).clone();
                                    Tree::One(x)
                                })
                                .collect(),
                        );

                        let ctx: Box<dyn Fn(Tree<T>) -> $iter_ty<F>> =
                            Box::new(move |new_tree: Tree<T>| {
                                let Tree::Many(xs) = new_tree else {
                                    todo!();
                                };
                                xs.into_iter()
                                    .map(|x| {
                                        let Tree::One(x) = x else {
                                            todo!();
                                        };
                                        let x: F = std::mem::transmute::<&T, &F>(&x).clone();
                                        x
                                    })
                                    .collect()
                            });

                        return (children, ctx);
                    }
                }

                // T != F: return all type T's contained in the type F's in the vector
                let mut child_trees: im::Vector<Tree<T>> = im::Vector::new();
                let mut child_ctxs: Vec<Box<dyn Fn(Tree<T>) -> F>> = Vec::new();
                for item in self {
                    let (tree, plate) = <F as Biplate<T>>::biplate(item);
                    child_trees.push_back(tree);
                    child_ctxs.push(plate);
                }

                let tree = Tree::Many(child_trees);
                let ctx = Box::new(move |new_tree: Tree<T>| {
                    let mut out = Vec::<F>::new();
                    let Tree::Many(new_trees) = new_tree else {
                        todo!()
                    };
                    for (child_tree, child_ctx) in std::iter::zip(new_trees, &child_ctxs) {
                        out.push(child_ctx(child_tree));
                    }
                    out.into_iter().collect::<$iter_ty<F>>()
                });
                (tree, ctx)
            }
        }

        // Traversal Biplate
        impl<T> Uniplate for $iter_ty<T>
        where
            T: Clone + Eq + Uniplate + Sized + 'static,
        {
            fn uniplate(&self) -> (Tree<Self>, Box<dyn Fn(Tree<Self>) -> Self>) {
                let val = self.clone();
                (Zero, Box::new(move |_| val.clone()))
            }
        }
    };
}

#[macro_export]
macro_rules! unreachable {
    ($from:ident,$to:ident) => {
        impl ::uniplate::biplate::Biplate<$to> for $from {
            fn biplate(
                &self,
            ) -> (
                ::uniplate::Tree<$to>,
                Box<dyn Fn(::uniplate::Tree<$to>) -> $from>,
            ) {
                let val = self.clone();
                (::uniplate::Tree::Zero, Box::new(move |_| val.clone()))
            }
        }
    };
}
