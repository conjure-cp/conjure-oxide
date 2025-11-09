use crate::ast::domains::ground::GroundDomain;
use crate::ast::domains::range::Range;
use crate::ast::domains::set_attr::SetAttr;
use crate::ast::domains::unresolved::{IntVal, UnresolvedDomain};
use crate::ast::{Moo, ReturnType, Typeable};
use polyquine::Quine;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

/// The integer type used in all domain code (int ranges, set sizes, etc)
pub(crate) type Int = i32;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Quine)]
pub enum Domain {
    /// A fully resolved domain
    Ground(GroundDomain),
    /// A domain which may contain references
    Unresolved(UnresolvedDomain),
}

/// Types that have a [`Domain`].
pub trait HasDomain {
    /// Gets the [`Domain`] of `self`.
    fn domain_of(&self) -> Domain;
}

impl<T: HasDomain> Typeable for T {
    fn return_type(&self) -> Option<ReturnType> {
        self.domain_of().return_type()
    }
}

impl Domain {
    pub fn new_bool() -> Domain {
        Domain::Ground(GroundDomain::Bool)
    }

    pub fn new_int(ranges: Vec<Range<Int>>) -> Domain {
        Domain::Ground(GroundDomain::Int(ranges))
    }

    pub fn new_int_unresolved(ranges: Vec<Range<IntVal>>) -> Domain {
        Domain::Unresolved(UnresolvedDomain::Int(ranges))
    }

    pub fn new_set(attr: SetAttr<Int>, inner_dom: Moo<GroundDomain>) -> Domain {
        Domain::Ground(GroundDomain::Set(attr, inner_dom))
    }

    pub fn new_set_unresolved(attr: SetAttr<IntVal>, inner_dom: Moo<Domain>) -> Domain {
        Domain::Unresolved(UnresolvedDomain::Set(attr, inner_dom))
    }

    pub fn new_matrix(inner_dom: Moo<GroundDomain>, idx_doms: Vec<GroundDomain>) -> Domain {
        Domain::Ground(GroundDomain::Matrix(inner_dom, idx_doms))
    }

    pub fn new_matrix_unresolved(inner_dom: Moo<Domain>, idx_doms: Vec<Domain>) -> Domain {
        Domain::Unresolved(UnresolvedDomain::Matrix(inner_dom, idx_doms))
    }
}

impl Typeable for Domain {
    fn return_type(&self) -> Option<ReturnType> {
        match self {
            Domain::Ground(dom) => dom.return_type(),
            Domain::Unresolved(dom) => dom.return_type(),
        }
    }
}

impl Display for Domain {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            Domain::Ground(gd) => gd.fmt(f),
            Domain::Unresolved(ud) => ud.fmt(f),
        }
    }
}
