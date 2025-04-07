use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::hash::Hash;
use std::hash::Hasher;

use uniplate::derive::Uniplate;
use uniplate::{Biplate, Tree, Uniplate};

use super::{Atom, Domain, Expression, Range};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Uniplate, Hash)]
#[uniplate(walk_into=[AbstractLiteral<Literal>])]
#[biplate(to=Atom)]
#[biplate(to=AbstractLiteral<Literal>)]
#[biplate(to=AbstractLiteral<Expression>)]
#[biplate(to=Expression)]
/// A literal value, equivalent to constants in Conjure.
pub enum Literal {
    Int(i32),
    Bool(bool),
    AbstractLiteral(AbstractLiteral<Literal>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbstractLiteral<T: Uniplate + Biplate<AbstractLiteral<T>> + Biplate<T>> {
    Set(Vec<T>),

    /// A 1 dimensional matrix slice with an index domain.
    Matrix(Vec<T>, Domain),

    // a tuple of literals
    Tuple(Vec<T>),
}

impl<T> AbstractLiteral<T>
where
    T: Uniplate + Biplate<AbstractLiteral<T>> + Biplate<T>,
{
    /// Creates a matrix with elements `elems`, with domain `int(1..)`.
    ///
    /// This acts as a variable sized list.
    pub fn matrix_implied_indices(elems: Vec<T>) -> Self {
        AbstractLiteral::Matrix(elems, Domain::IntDomain(vec![Range::UnboundedR(1)]))
    }

    /// If the AbstractLiteral is a list, returns its elements.
    ///
    /// A list is any a matrix with the domain `int(1..)`. This includes matrix literals without
    /// any explicitly specified domain.
    pub fn unwrap_list(&self) -> Option<&Vec<T>> {
        let AbstractLiteral::Matrix(elems, Domain::IntDomain(ranges)) = self else {
            return None;
        };

        let [Range::UnboundedR(1)] = ranges[..] else {
            return None;
        };

        Some(elems)
    }
}

impl<T> Display for AbstractLiteral<T>
where
    T: Uniplate + Biplate<AbstractLiteral<T>> + Biplate<T> + Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AbstractLiteral::Set(elems) => {
                let elems_str: String = elems.iter().map(|x| format!("{x}")).join(",");
                write!(f, "{{{elems_str}}}")
            }
            AbstractLiteral::Matrix(elems, index_domain) => {
                let elems_str: String = elems.iter().map(|x| format!("{x}")).join(",");
                write!(f, "[{elems_str};{index_domain}]")
            }
            AbstractLiteral::Tuple(elems) => {
                let elems_str: String = elems.iter().map(|x| format!("{x}")).join(",");
                write!(f, "({elems_str})")
            }
        }
    }
}

impl Hash for AbstractLiteral<Literal> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            AbstractLiteral::Set(vec) => {
                0.hash(state);
                vec.hash(state);
            }
            AbstractLiteral::Matrix(elems, index_domain) => {
                1.hash(state);
                elems.hash(state);
                index_domain.hash(state);
            }
            //TODO: Tuple is the hashing just incremental?
            AbstractLiteral::Tuple(elems) => {
                2.hash(state);
                elems.hash(state);
            }
        }
    }
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
            AbstractLiteral::Matrix(elems, index_domain) => {
                let index_domain = index_domain.clone();
                let (f1_tree, f1_ctx) = <_ as Biplate<AbstractLiteral<T>>>::biplate(elems);
                (
                    f1_tree,
                    Box::new(move |x| AbstractLiteral::Matrix(f1_ctx(x), index_domain.clone())),
                )
            }
            AbstractLiteral::Tuple(elems) => {
                let (f1_tree, f1_ctx) = <_ as Biplate<AbstractLiteral<T>>>::biplate(elems);
                (
                    f1_tree,
                    Box::new(move |x| AbstractLiteral::Tuple(f1_ctx(x))),
                )
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
        if std::any::TypeId::of::<To>() == std::any::TypeId::of::<AbstractLiteral<U>>() {
            // To ==From => return One(self)

            unsafe {
                // SAFETY: asserted the type equality above
                let self_to = std::mem::transmute::<&AbstractLiteral<U>, &To>(self).clone();
                let tree = Tree::One(self_to.clone());
                let ctx = Box::new(move |x| {
                    let Tree::One(x) = x else {
                        panic!();
                    };

                    std::mem::transmute::<&To, &AbstractLiteral<U>>(&x).clone()
                });

                (tree, ctx)
            }
        } else {
            // walking into T
            match self {
                AbstractLiteral::Set(vec) => {
                    let (f1_tree, f1_ctx) = <_ as Biplate<To>>::biplate(vec);
                    (f1_tree, Box::new(move |x| AbstractLiteral::Set(f1_ctx(x))))
                }
                AbstractLiteral::Matrix(elems, index_domain) => {
                    let index_domain = index_domain.clone();
                    let (f1_tree, f1_ctx) = <_ as Biplate<To>>::biplate(elems);
                    (
                        f1_tree,
                        Box::new(move |x| AbstractLiteral::Matrix(f1_ctx(x), index_domain.clone())),
                    )
                }
                AbstractLiteral::Tuple(elems) => {
                    let (f1_tree, f1_ctx) = <_ as Biplate<To>>::biplate(elems);
                    (
                        f1_tree,
                        Box::new(move |x| AbstractLiteral::Tuple(f1_ctx(x))),
                    )
                }
            }
        }
    }
}

impl TryFrom<Literal> for i32 {
    type Error = &'static str;

    fn try_from(value: Literal) -> Result<Self, Self::Error> {
        match value {
            Literal::Int(i) => Ok(i),
            _ => Err("Cannot convert non-i32 literal to i32"),
        }
    }
}

impl TryFrom<&Literal> for i32 {
    type Error = &'static str;

    fn try_from(value: &Literal) -> Result<Self, Self::Error> {
        match value {
            Literal::Int(i) => Ok(*i),
            _ => Err("Cannot convert non-i32 literal to i32"),
        }
    }
}

impl TryFrom<Literal> for bool {
    type Error = &'static str;

    fn try_from(value: Literal) -> Result<Self, Self::Error> {
        match value {
            Literal::Bool(b) => Ok(b),
            _ => Err("Cannot convert non-bool literal to bool"),
        }
    }
}

impl TryFrom<&Literal> for bool {
    type Error = &'static str;

    fn try_from(value: &Literal) -> Result<Self, Self::Error> {
        match value {
            Literal::Bool(b) => Ok(*b),
            _ => Err("Cannot convert non-bool literal to bool"),
        }
    }
}

impl From<i32> for Literal {
    fn from(i: i32) -> Self {
        Literal::Int(i)
    }
}

impl From<bool> for Literal {
    fn from(b: bool) -> Self {
        Literal::Bool(b)
    }
}

impl AbstractLiteral<Expression> {
    /// If all the elements are literals, returns this as an AbstractLiteral<Literal>.
    /// Otherwise, returns `None`.
    pub fn as_literals(self) -> Option<AbstractLiteral<Literal>> {
        match self {
            AbstractLiteral::Set(_) => todo!(),
            AbstractLiteral::Matrix(items, domain) => {
                let mut literals = vec![];
                for item in items {
                    let literal = match item {
                        Expression::Atomic(_, Atom::Literal(lit)) => Some(lit),
                        Expression::AbstractLiteral(_, abslit) => {
                            Some(Literal::AbstractLiteral(abslit.as_literals()?))
                        }
                        _ => None,
                    }?;
                    literals.push(literal);
                }

                Some(AbstractLiteral::Matrix(literals, domain))
            }
            AbstractLiteral::Tuple(items) => {
                let mut literals = vec![];
                for item in items {
                    let literal = match item {
                        Expression::Atomic(_, Atom::Literal(lit)) => Some(lit),
                        Expression::AbstractLiteral(_, abslit) => {
                            Some(Literal::AbstractLiteral(abslit.as_literals()?))
                        }
                        _ => None,
                    }?;
                    literals.push(literal);
                }

                Some(AbstractLiteral::Tuple(literals))
            }
        }
    }
}

// need display implementations for other types as well
impl Display for Literal {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            Literal::Int(i) => write!(f, "{}", i),
            Literal::Bool(b) => write!(f, "{}", b),
            Literal::AbstractLiteral(l) => write!(f, "{:?}", l),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{into_matrix, matrix};
    use uniplate::Uniplate;

    #[test]
    fn matrix_uniplate_universe() {
        // Can we traverse through matrices with uniplate?
        let my_matrix: AbstractLiteral<Literal> = into_matrix![
            vec![Literal::AbstractLiteral(matrix![Literal::Bool(true);Domain::BoolDomain]); 5];
            Domain::BoolDomain
        ];

        let expected_index_domains = vec![Domain::BoolDomain; 6];
        let actual_index_domains: Vec<Domain> = my_matrix.cata(Arc::new(move |elem, children| {
            let mut res = vec![];
            res.extend(children.into_iter().flatten());
            if let AbstractLiteral::Matrix(_, index_domain) = elem {
                res.push(index_domain);
            }

            res
        }));

        assert_eq!(actual_index_domains, expected_index_domains);
    }
}
