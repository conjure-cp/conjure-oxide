use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::hash::Hash;
use std::hash::Hasher;
use ustr::Ustr;

use polyquine::Quine;
use uniplate::{Biplate, Tree, Uniplate};

use crate::ast::Metadata;
use crate::ast::pretty::pretty_vec;

use super::domains::HasDomain;
use super::{Atom, Domain, Expression, Range, records::RecordValue};
use super::{Moo, ReturnType, SetAttr, Typeable};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Uniplate, Hash, Quine)]
#[uniplate(walk_into=[AbstractLiteral<Literal>])]
#[biplate(to=Atom)]
#[biplate(to=AbstractLiteral<Literal>)]
#[biplate(to=AbstractLiteral<Expression>)]
#[biplate(to=RecordValue<Literal>)]
#[biplate(to=RecordValue<Expression>)]
#[biplate(to=Expression)]
#[path_prefix(conjure_cp::ast)]
/// A literal value, equivalent to constants in Conjure.
pub enum Literal {
    Int(i32),
    Bool(bool),
    //abstract literal variant ends in Literal, but that's ok
    #[allow(clippy::enum_variant_names)]
    AbstractLiteral(AbstractLiteral<Literal>),
}

impl HasDomain for Literal {
    fn domain_of(&self) -> Domain {
        match self {
            Literal::Int(i) => Domain::Int(vec![Range::Single(*i)]),
            Literal::Bool(_) => Domain::Bool,
            Literal::AbstractLiteral(abstract_literal) => abstract_literal.domain_of(),
        }
    }
}

// make possible values of an AbstractLiteral a closed world to make the trait bounds more sane (particularly in Uniplate instances!!)
pub trait AbstractLiteralValue:
    Clone + Eq + PartialEq + Display + Uniplate + Biplate<RecordValue<Self>> + 'static
{
}
impl AbstractLiteralValue for Expression {}
impl AbstractLiteralValue for Literal {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Quine)]
#[path_prefix(conjure_cp::ast)]
pub enum AbstractLiteral<T: AbstractLiteralValue> {
    Set(Vec<T>),

    /// A 1 dimensional matrix slice with an index domain.
    Matrix(Vec<T>, Box<Domain>),

    // a tuple of literals
    Tuple(Vec<T>),

    Record(Vec<RecordValue<T>>),
}

// TODO: use HasDomain instead once Expression::domain_of returns Domain not Option<Domain>
impl AbstractLiteral<Expression> {
    pub fn domain_of(&self) -> Option<Domain> {
        match self {
            AbstractLiteral::Set(items) => {
                // ensure that all items have a domain, or return None
                let item_domains: Vec<Domain> = items
                    .iter()
                    .map(|x| x.domain_of())
                    .collect::<Option<Vec<Domain>>>()?;

                // union all item domains together
                let mut item_domain_iter = item_domains.iter().cloned();
                let first_item = item_domain_iter.next()?;
                let item_domain = item_domains
                    .iter()
                    .try_fold(first_item, |x: Domain, y| x.union(y))
                    .expect("taking the union of all item domains of a set literal should succeed");

                Some(Domain::Set(SetAttr::None, Box::new(item_domain)))
            }

            AbstractLiteral::Matrix(items, _) => {
                // ensure that all items have a domain, or return None
                let item_domains: Vec<Domain> = items
                    .iter()
                    .map(|x| x.domain_of())
                    .collect::<Option<Vec<Domain>>>()?;

                // union all item domains together
                let mut item_domain_iter = item_domains.iter().cloned();

                let first_item = item_domain_iter.next()?;

                let item_domain = item_domains
                    .iter()
                    .try_fold(first_item, |x: Domain, y| x.union(y))
                    .expect(
                        "taking the union of all item domains of a matrix literal should succeed",
                    );

                let mut new_index_domain = vec![];

                // flatten index domains of n-d matrix into list
                let mut e = Expression::AbstractLiteral(Metadata::new(), self.clone());
                while let Expression::AbstractLiteral(_, AbstractLiteral::Matrix(elems, idx)) = e {
                    assert!(
                        !matches!(idx.as_ref(), Domain::Matrix(_, _)),
                        "n-dimensional matrix literals should be represented as a matrix inside a matrix"
                    );
                    new_index_domain.push(idx.as_ref().clone());
                    e = elems[0].clone();
                }
                Some(Domain::Matrix(Box::new(item_domain), new_index_domain))
            }
            AbstractLiteral::Tuple(_) => None,
            AbstractLiteral::Record(_) => None,
        }
    }
}

impl HasDomain for AbstractLiteral<Literal> {
    fn domain_of(&self) -> Domain {
        Domain::from_literal_vec(vec![Literal::AbstractLiteral(self.clone())])
            .expect("abstract literals should be correctly typed")
    }
}

impl Typeable for AbstractLiteral<Expression> {
    fn return_type(&self) -> Option<ReturnType> {
        match self {
            AbstractLiteral::Set(items) if items.is_empty() => {
                Some(ReturnType::Set(Box::new(ReturnType::Unknown)))
            }
            AbstractLiteral::Set(items) => {
                let item_type = items[0].return_type()?;

                // if any items do not have a type, return none.
                let item_types: Option<Vec<ReturnType>> =
                    items.iter().map(|x| x.return_type()).collect();

                let item_types = item_types?;

                assert!(
                    item_types.iter().all(|x| x == &item_type),
                    "all items in a set should have the same type"
                );

                Some(ReturnType::Set(Box::new(item_type)))
            }
            AbstractLiteral::Matrix(items, _) if items.is_empty() => {
                Some(ReturnType::Matrix(Box::new(ReturnType::Unknown)))
            }
            AbstractLiteral::Matrix(items, _) => {
                let item_type = items[0].return_type()?;

                // if any items do not have a type, return none.
                let item_types: Option<Vec<ReturnType>> =
                    items.iter().map(|x| x.return_type()).collect();

                let item_types = item_types?;

                assert!(
                    item_types.iter().all(|x| x == &item_type),
                    "all items in a matrix should have the same type. items: {items} types: {types:#?}",
                    items = pretty_vec(items),
                    types = items
                        .iter()
                        .map(|x| x.return_type())
                        .collect::<Vec<Option<ReturnType>>>()
                );

                Some(ReturnType::Matrix(Box::new(item_type)))
            }
            AbstractLiteral::Tuple(items) => {
                let mut item_types = vec![];
                for item in items {
                    item_types.push(item.return_type()?);
                }
                Some(ReturnType::Tuple(item_types))
            }
            AbstractLiteral::Record(items) => {
                let mut item_types = vec![];
                for item in items {
                    item_types.push(item.value.return_type()?);
                }
                Some(ReturnType::Record(item_types))
            }
        }
    }
}

impl<T> AbstractLiteral<T>
where
    T: AbstractLiteralValue,
{
    /// Creates a matrix with elements `elems`, with domain `int(1..)`.
    ///
    /// This acts as a variable sized list.
    pub fn matrix_implied_indices(elems: Vec<T>) -> Self {
        AbstractLiteral::Matrix(elems, Box::new(Domain::Int(vec![Range::UnboundedR(1)])))
    }

    /// If the AbstractLiteral is a list, returns its elements.
    ///
    /// A list is any a matrix with the domain `int(1..)`. This includes matrix literals without
    /// any explicitly specified domain.
    pub fn unwrap_list(&self) -> Option<&Vec<T>> {
        let AbstractLiteral::Matrix(elems, domain) = self else {
            return None;
        };

        let Domain::Int(ranges) = domain.as_ref() else {
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
    T: AbstractLiteralValue,
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
            AbstractLiteral::Record(entries) => {
                let entries_str: String = entries
                    .iter()
                    .map(|entry| format!("{}: {}", entry.name, entry.value))
                    .join(",");
                write!(f, "{{{entries_str}}}")
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
            AbstractLiteral::Tuple(elems) => {
                2.hash(state);
                elems.hash(state);
            }
            AbstractLiteral::Record(entries) => {
                3.hash(state);
                entries.hash(state);
            }
        }
    }
}

impl<T> Uniplate for AbstractLiteral<T>
where
    T: AbstractLiteralValue + Biplate<AbstractLiteral<T>>,
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
            AbstractLiteral::Record(entries) => {
                let (f1_tree, f1_ctx) = <_ as Biplate<AbstractLiteral<T>>>::biplate(entries);
                (
                    f1_tree,
                    Box::new(move |x| AbstractLiteral::Record(f1_ctx(x))),
                )
            }
        }
    }
}

impl<U, To> Biplate<To> for AbstractLiteral<U>
where
    To: Uniplate,
    U: AbstractLiteralValue + Biplate<AbstractLiteral<U>> + Biplate<To>,
    RecordValue<U>: Biplate<AbstractLiteral<U>> + Biplate<To>,
{
    fn biplate(&self) -> (Tree<To>, Box<dyn Fn(Tree<To>) -> Self>) {
        if std::any::TypeId::of::<To>() == std::any::TypeId::of::<AbstractLiteral<U>>() {
            // To ==From => return One(self)

            unsafe {
                // SAFETY: asserted the type equality above
                let self_to = std::mem::transmute::<&AbstractLiteral<U>, &To>(self).clone();
                let tree = Tree::One(self_to);
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
                    let (f1_tree, f1_ctx) = <Vec<U> as Biplate<To>>::biplate(elems);
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
                AbstractLiteral::Record(entries) => {
                    let (f1_tree, f1_ctx) = <_ as Biplate<To>>::biplate(entries);
                    (
                        f1_tree,
                        Box::new(move |x| AbstractLiteral::Record(f1_ctx(x))),
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

impl TryFrom<Box<Literal>> for i32 {
    type Error = &'static str;

    fn try_from(value: Box<Literal>) -> Result<Self, Self::Error> {
        (*value).try_into()
    }
}

impl TryFrom<&Box<Literal>> for i32 {
    type Error = &'static str;

    fn try_from(value: &Box<Literal>) -> Result<Self, Self::Error> {
        TryFrom::<&Literal>::try_from(value.as_ref())
    }
}

impl TryFrom<&Moo<Literal>> for i32 {
    type Error = &'static str;

    fn try_from(value: &Moo<Literal>) -> Result<Self, Self::Error> {
        TryFrom::<&Literal>::try_from(value.as_ref())
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

impl From<Literal> for Ustr {
    fn from(value: Literal) -> Self {
        // TODO: avoid the temporary-allocation of a string by format! here?
        Ustr::from(&format!("{value}"))
    }
}

impl AbstractLiteral<Expression> {
    /// If all the elements are literals, returns this as an AbstractLiteral<Literal>.
    /// Otherwise, returns `None`.
    pub fn into_literals(self) -> Option<AbstractLiteral<Literal>> {
        match self {
            AbstractLiteral::Set(_) => todo!(),
            AbstractLiteral::Matrix(items, domain) => {
                let mut literals = vec![];
                for item in items {
                    let literal = match item {
                        Expression::Atomic(_, Atom::Literal(lit)) => Some(lit),
                        Expression::AbstractLiteral(_, abslit) => {
                            Some(Literal::AbstractLiteral(abslit.into_literals()?))
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
                            Some(Literal::AbstractLiteral(abslit.into_literals()?))
                        }
                        _ => None,
                    }?;
                    literals.push(literal);
                }

                Some(AbstractLiteral::Tuple(literals))
            }
            AbstractLiteral::Record(entries) => {
                let mut literals = vec![];
                for entry in entries {
                    let literal = match entry.value {
                        Expression::Atomic(_, Atom::Literal(lit)) => Some(lit),
                        Expression::AbstractLiteral(_, abslit) => {
                            Some(Literal::AbstractLiteral(abslit.into_literals()?))
                        }
                        _ => None,
                    }?;

                    literals.push((entry.name, literal));
                }
                Some(AbstractLiteral::Record(
                    literals
                        .into_iter()
                        .map(|(name, literal)| RecordValue {
                            name,
                            value: literal,
                        })
                        .collect(),
                ))
            }
        }
    }
}

// need display implementations for other types as well
impl Display for Literal {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            Literal::Int(i) => write!(f, "{i}"),
            Literal::Bool(b) => write!(f, "{b}"),
            Literal::AbstractLiteral(l) => write!(f, "{l:?}"),
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{into_matrix, matrix};
    use uniplate::Uniplate;

    #[test]
    fn matrix_uniplate_universe() {
        // Can we traverse through matrices with uniplate?
        let my_matrix: AbstractLiteral<Literal> = into_matrix![
            vec![Literal::AbstractLiteral(matrix![Literal::Bool(true);Domain::Bool]); 5];
            Domain::Bool
        ];

        let expected_index_domains = vec![Domain::Bool; 6];
        let actual_index_domains: Vec<Domain> = my_matrix.cata(&move |elem, children| {
            let mut res = vec![];
            res.extend(children.into_iter().flatten());
            if let AbstractLiteral::Matrix(_, index_domain) = elem {
                res.push(*index_domain);
            }

            res
        });

        assert_eq!(actual_index_domains, expected_index_domains);
    }
}
