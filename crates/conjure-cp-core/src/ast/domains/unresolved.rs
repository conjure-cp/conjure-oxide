use crate::ast::domains::set_attr::SetAttr;
use crate::ast::{
    DeclarationKind, Expression, Moo, RecordEntry, Reference, Typeable,
    domains::{
        domain::{Domain, Int},
        range::Range,
    },
};
use crate::bug;
use conjure_cp_core::ast::ReturnType;
use conjure_cp_core::ast::pretty::pretty_vec;
use itertools::Itertools;
use polyquine::Quine;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::ops::Deref;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Quine)]
#[path_prefix(conjure_cp::ast)]
pub(super) enum IntVal {
    Const(Int),
    #[polyquine_skip]
    Reference(Reference),
    Expr(Moo<Expression>),
}

impl From<Int> for IntVal {
    fn from(value: Int) -> Self {
        Self::Const(value)
    }
}

impl Display for IntVal {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            IntVal::Const(val) => write!(f, "{val}"),
            IntVal::Reference(re) => write!(f, "{re}"),
            IntVal::Expr(expr) => write!(f, "({expr})"),
        }
    }
}

impl IntVal {
    pub fn new_ref(re: &Reference) -> Option<IntVal> {
        match re.ptr.kind().deref() {
            DeclarationKind::ValueLetting(expr) => match expr.return_type() {
                Some(ReturnType::Int) => Some(IntVal::Reference(re.clone())),
                _ => None,
            },
            DeclarationKind::Given(dom) => match dom.return_type() {
                Some(ReturnType::Int) => Some(IntVal::Reference(re.clone())),
                _ => None,
            },
            DeclarationKind::DomainLetting(_)
            | DeclarationKind::RecordField(_)
            | DeclarationKind::DecisionVariable(_) => None,
        }
    }

    pub fn new_expr(value: Moo<Expression>) -> Option<IntVal> {
        if value.return_type().is_none() {
            return None;
        };
        todo!()
    }

    pub fn resolve(&self) -> Option<Int> {
        match self {
            IntVal::Const(value) => Some(*value),
            IntVal::Expr(expr) => todo!(),
            IntVal::Reference(re) => match re.ptr.kind().deref() {
                DeclarationKind::ValueLetting(expr) => todo!(),
                // If this is an int given we will be able to resolve it eventually, but not yet
                DeclarationKind::Given(_) => None,
                DeclarationKind::DomainLetting(_)
                | DeclarationKind::RecordField(_)
                | DeclarationKind::DecisionVariable(_) => bug!(
                    "Expected integer expression, given, or letting inside int domain; Got: {re}"
                ),
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Quine)]
pub enum UnresolvedDomain {
    Int(Vec<Range<IntVal>>),
    /// A set of elements drawn from the inner domain
    Set(SetAttr<IntVal>, Moo<Domain>),
    /// A n-dimensional matrix with a value domain and n-index domains
    Matrix(Moo<Domain>, Vec<Domain>),
    /// A tuple of N elements, each with its own domain
    Tuple(Vec<Domain>),
    /// A reference to a domain letting
    #[polyquine_skip]
    Reference(Reference),
    Record(Vec<RecordEntry<Domain>>),
}

impl Typeable for UnresolvedDomain {
    fn return_type(&self) -> Option<ReturnType> {
        match self {
            UnresolvedDomain::Reference(re) => re.return_type(),
            UnresolvedDomain::Int(_) => Some(ReturnType::Int),
            UnresolvedDomain::Set(_attr, inner) => {
                inner.return_type().map(|ty| ReturnType::Set(Box::new(ty)))
            }
            UnresolvedDomain::Matrix(inner, _idx) => inner
                .return_type()
                .map(|ty| ReturnType::Matrix(Box::new(ty))),
            UnresolvedDomain::Tuple(inners) => {
                let mut inner_types = Vec::new();
                for inner in inners {
                    inner_types.push(inner.return_type()?);
                }
                Some(ReturnType::Tuple(inner_types))
            }
            UnresolvedDomain::Record(entries) => {
                let mut entry_types = Vec::new();
                for entry in entries {
                    entry_types.push(entry.domain.return_type()?);
                }
                Some(ReturnType::Record(entry_types))
            }
        }
    }
}

impl Display for UnresolvedDomain {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            UnresolvedDomain::Reference(re) => write!(f, "{re}"),
            UnresolvedDomain::Int(ranges) => {
                if ranges.iter().all(Range::is_bounded) {
                    let rngs: String = ranges.iter().map(|r| format!("{r}")).join(", ");
                    write!(f, "int({})", rngs)
                } else {
                    write!(f, "int")
                }
            }
            UnresolvedDomain::Set(attrs, inner_dom) => write!(f, "set {attrs} of {inner_dom}"),
            UnresolvedDomain::Matrix(value_domain, index_domains) => {
                write!(
                    f,
                    "matrix indexed by [{}] of {value_domain}",
                    pretty_vec(&index_domains.iter().collect_vec())
                )
            }
            UnresolvedDomain::Tuple(domains) => {
                write!(
                    f,
                    "tuple of ({})",
                    pretty_vec(&domains.iter().collect_vec())
                )
            }
            UnresolvedDomain::Record(entries) => {
                write!(
                    f,
                    "record of ({})",
                    pretty_vec(
                        &entries
                            .iter()
                            .map(|entry| format!("{}: {}", entry.name, entry.domain))
                            .collect_vec()
                    )
                )
            }
        }
    }
}
