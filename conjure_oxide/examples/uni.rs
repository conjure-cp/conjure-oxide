use conjure_oxide::Metadata;
use std::collections::VecDeque;
use uniplate::derive::Uniplate;
use uniplate::{Biplate, Tree, Uniplate};

#[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
#[uniplate(walk_into=[Atom])]
#[biplate(to=Atom)]
#[biplate(to=AbstractLiteral<Expression>)]
#[biplate(to=AbstractLiteral<Literal>,walk_into=[Atom])]
#[biplate(to=Literal,walk_into=[Atom])]
enum Expression {
    Sum(Metadata, Vec<Expression>),
    AbstractLiteral(Metadata, AbstractLiteral<Expression>),
    Atomic(Metadata, Atom),
}

// even though this doesn't contain AbstractLiteral<Expression>, we need to derive an instance to
// say that it doesn't contain them!
#[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
#[uniplate(walk_into=[Literal])]
#[biplate(to=Literal)]
#[biplate(to=Expression)]
#[biplate(to=AbstractLiteral<Literal>,walk_into=[Literal])]
#[biplate(to=AbstractLiteral<Expression>)]
enum Atom {
    Reference(String),
    Literal(Literal),
}

// even though this doesn't contain AbstractLiteral<Expression>, we need to derive an instance to
// say that it doesn't contain them!
#[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
#[uniplate(walk_into=[AbstractLiteral<Literal>])]
#[biplate(to=Atom)]
#[biplate(to=AbstractLiteral<Literal>)]
#[biplate(to=AbstractLiteral<Expression>)]
#[biplate(to=Expression)]
enum Literal {
    Bool(bool),
    Int(i32),
    AbstractLiteral(AbstractLiteral<Literal>),
}

// These don't work!
// #[derive(Uniplate)]
// #[uniplate(walk_into=[T])]
// #[biplate(to=T)]
#[derive(Debug, Clone, PartialEq, Eq)]
enum AbstractLiteral<T: Uniplate + Biplate<AbstractLiteral<T>> + Biplate<T>> {
    Set(Vec<T>),
    Matrix(Vec<T>), // for sake of example, will be more complicated irl!
}

impl<T> Uniplate for AbstractLiteral<T>
where
    T: Uniplate + Biplate<AbstractLiteral<T>> + Biplate<T>,
{
    fn uniplate(&self) -> (Tree<Self>, Box<dyn Fn(Tree<Self>) -> Self>) {
        // walking into T
        match self {
            AbstractLiteral::Set(vec) => {
                let (f1_tree, f1_ctx) = <_ as Biplate<AbstractLiteral<T>>>::biplate(vec);
                (f1_tree, Box::new(move |x| AbstractLiteral::Set(f1_ctx(x))))
            }
            AbstractLiteral::Matrix(vec) => {
                let (f1_tree, f1_ctx) = <_ as Biplate<AbstractLiteral<T>>>::biplate(vec);
                (f1_tree, Box::new(move |x| AbstractLiteral::Set(f1_ctx(x))))
            }
        }
    }
}

impl<U, To> Biplate<To> for AbstractLiteral<U>
where
    To: Uniplate,
    U: Biplate<To> + Biplate<U> + Biplate<AbstractLiteral<U>>,
{
    fn biplate(&self) -> (Tree<To>, Box<dyn Fn(Tree<To>) -> Self>) {
        // walking into T
        match self {
            AbstractLiteral::Set(vec) => {
                let (f1_tree, f1_ctx) = <_ as Biplate<To>>::biplate(vec);
                (f1_tree, Box::new(move |x| AbstractLiteral::Set(f1_ctx(x))))
            }
            AbstractLiteral::Matrix(vec) => {
                let (f1_tree, f1_ctx) = <_ as Biplate<To>>::biplate(vec);
                (f1_tree, Box::new(move |x| AbstractLiteral::Set(f1_ctx(x))))
            }
        }
    }
}

// If the above kicks up a fuss, could always do it manually for Expression and Literal as below:

// impl<To> Biplate<To> for AbstractLiteral<Expression>
//     where To: Uniplate,
//           Expression: Biplate<To>,
// {
//     fn biplate(&self) -> (Tree<To>, Box<dyn Fn(Tree<To>) -> Self>) {
//         // walking into T
//         match self {
//             AbstractLiteral::Set(vec) => {
//                 let (f1_tree, f1_ctx) = <_ as Biplate<To>>::biplate(vec);
//                 (f1_tree, Box::new(move |x| AbstractLiteral::Set(f1_ctx(x))))
//             }
//             AbstractLiteral::Matrix(vec) => {
//                 let (f1_tree, f1_ctx) = <_ as Biplate<To>>::biplate(vec);
//                 (f1_tree, Box::new(move |x| AbstractLiteral::Set(f1_ctx(x))))
//             }
//         }
//     }
// }

// impl<To> Biplate<To> for AbstractLiteral<Literal>
//     where To: Uniplate,
//           Literal: Biplate<To>,
// {
//     fn biplate(&self) -> (Tree<To>, Box<dyn Fn(Tree<To>) -> Self>) {
//         // walking into T
//         match self {
//             AbstractLiteral::Set(vec) => {
//                 let (f1_tree, f1_ctx) = <_ as Biplate<To>>::biplate(vec);
//                 (f1_tree, Box::new(move |x| AbstractLiteral::Set(f1_ctx(x))))
//             }
//             AbstractLiteral::Matrix(vec) => {
//                 let (f1_tree, f1_ctx) = <_ as Biplate<To>>::biplate(vec);
//                 (f1_tree, Box::new(move |x| AbstractLiteral::Set(f1_ctx(x))))
//             }
//         }
//     }
// }

fn main() {
    let e = Expression::AbstractLiteral(
        Metadata::new(),
        AbstractLiteral::Set(vec![Expression::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Set(vec![
                Literal::Int(1),
                Literal::Int(2),
            ]))),
        )]),
    );

    let lits: VecDeque<Literal> = e.children_bi();
    let exprs: VecDeque<Expression> = e.children();
    let atoms: VecDeque<Atom> = e.children_bi();
    println!("{lits:#?}");
    println!("{exprs:#?}");
    println!("{atoms:#?}");
}
