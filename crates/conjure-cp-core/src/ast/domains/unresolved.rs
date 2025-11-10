use crate::ast::domains::set_attr::SetAttr;
use crate::ast::{
    DeclarationKind, DomainOpError, Expression, MaybeTypeable, Moo, RecordEntryGround, Reference,
    domains::{
        GroundDomain,
        domain::{Domain, DomainPtr, Int},
        range::Range,
    },
};
use crate::bug;
use conjure_cp_core::ast::pretty::pretty_vec;
use conjure_cp_core::ast::{Name, ReturnType};
use itertools::Itertools;
use polyquine::Quine;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::iter::zip;
use std::ops::Deref;
use uniplate::Uniplate;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine)]
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

impl TryInto<Int> for IntVal {
    type Error = DomainOpError;

    fn try_into(self) -> Result<Int, Self::Error> {
        match self {
            IntVal::Const(val) => Ok(val),
            _ => Err(DomainOpError::InputContainsReference),
        }
    }
}

impl From<Range<Int>> for Range<IntVal> {
    fn from(value: Range<Int>) -> Self {
        match value {
            Range::Single(x) => Range::Single(x.into()),
            Range::Bounded(l, r) => Range::Bounded(l.into(), r.into()),
            Range::UnboundedL(r) => Range::UnboundedL(r.into()),
            Range::UnboundedR(l) => Range::UnboundedR(l.into()),
            Range::Unbounded => Range::Unbounded,
        }
    }
}

impl TryInto<Range<Int>> for Range<IntVal> {
    type Error = DomainOpError;

    fn try_into(self) -> Result<Range<Int>, Self::Error> {
        match self {
            Range::Single(x) => Ok(Range::Single(x.try_into()?)),
            Range::Bounded(l, r) => Ok(Range::Bounded(l.try_into()?, r.try_into()?)),
            Range::UnboundedL(r) => Ok(Range::UnboundedL(r.try_into()?)),
            Range::UnboundedR(l) => Ok(Range::UnboundedR(l.try_into()?)),
            Range::Unbounded => Ok(Range::Unbounded),
        }
    }
}

impl From<SetAttr<Int>> for SetAttr<IntVal> {
    fn from(value: SetAttr<Int>) -> Self {
        SetAttr {
            size: value.size.into(),
        }
    }
}

impl TryInto<SetAttr<Int>> for SetAttr<IntVal> {
    type Error = DomainOpError;

    fn try_into(self) -> Result<SetAttr<Int>, Self::Error> {
        Ok(SetAttr {
            size: self.size.try_into()?,
        })
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
            DeclarationKind::ValueLetting(expr) => match expr.maybe_return_type() {
                Some(ReturnType::Int) => Some(IntVal::Reference(re.clone())),
                _ => None,
            },
            DeclarationKind::Given(dom) => match dom.maybe_return_type() {
                Some(ReturnType::Int) => Some(IntVal::Reference(re.clone())),
                _ => None,
            },
            DeclarationKind::DomainLetting(_)
            | DeclarationKind::RecordField(_)
            | DeclarationKind::DecisionVariable(_) => None,
        }
    }

    pub fn new_expr(value: Moo<Expression>) -> Option<IntVal> {
        if value.maybe_return_type().is_none() {
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

impl Range<IntVal> {
    pub fn resolve(&self) -> Option<Range<Int>> {
        match self {
            Range::Single(x) => Some(Range::Single(x.resolve()?)),
            Range::Bounded(l, r) => Some(Range::Bounded(l.resolve()?, r.resolve()?)),
            Range::UnboundedL(r) => Some(Range::UnboundedL(r.resolve()?)),
            Range::UnboundedR(l) => Some(Range::UnboundedR(l.resolve()?)),
            Range::Unbounded => Some(Range::Unbounded),
        }
    }
}

impl SetAttr<IntVal> {
    pub fn resolve(&self) -> Option<SetAttr<Int>> {
        Some(SetAttr {
            size: self.size.resolve()?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Uniplate, Quine)]
#[path_prefix(conjure_cp::ast)]
pub struct RecordEntry {
    pub name: Name,
    pub domain: DomainPtr,
}

impl RecordEntry {
    pub fn resolve(self) -> Option<RecordEntryGround> {
        Some(RecordEntryGround {
            name: self.name,
            domain: self.domain.resolve()?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Quine)]
pub enum UnresolvedDomain {
    Int(Vec<Range<IntVal>>),
    /// A set of elements drawn from the inner domain
    Set(SetAttr<IntVal>, DomainPtr),
    /// A n-dimensional matrix with a value domain and n-index domains
    Matrix(DomainPtr, Vec<DomainPtr>),
    /// A tuple of N elements, each with its own domain
    Tuple(Vec<DomainPtr>),
    /// A reference to a domain letting
    #[polyquine_skip]
    Reference(Reference),
    Record(Vec<RecordEntry>),
}

impl UnresolvedDomain {
    pub fn resolve(&self) -> Option<GroundDomain> {
        match self {
            UnresolvedDomain::Int(rngs) => {
                match rngs.iter().map(Range::<IntVal>::resolve).collect() {
                    Some(int_rngs) => Some(GroundDomain::Int(int_rngs)),
                    _ => None,
                }
            }
            UnresolvedDomain::Set(attr, inner) => {
                Some(GroundDomain::Set(attr.resolve()?, inner.resolve()?))
            }
            UnresolvedDomain::Matrix(inner, idx_doms) => {
                match idx_doms.iter().map(DomainPtr::resolve).collect() {
                    Some(idx_gds) => Some(GroundDomain::Matrix(inner.resolve()?, idx_gds)),
                    _ => None,
                }
            }
            UnresolvedDomain::Tuple(inners) => {
                match inners.iter().map(DomainPtr::resolve).collect() {
                    Some(inners_gds) => Some(GroundDomain::Tuple(inners_gds)),
                    _ => None,
                }
            }
            UnresolvedDomain::Record(entries) => match entries
                .iter()
                .map(|f| {
                    f.domain.resolve().map(|gd| RecordEntryGround {
                        name: f.name.clone(),
                        domain: gd,
                    })
                })
                .collect()
            {
                Some(entries_gds) => Some(GroundDomain::Record(entries_gds)),
                _ => None,
            },
            UnresolvedDomain::Reference(_) => None,
        }
    }

    pub(super) fn union_unresolved(
        &self,
        other: &UnresolvedDomain,
    ) -> Result<UnresolvedDomain, DomainOpError> {
        match (self, other) {
            (UnresolvedDomain::Int(lhs), UnresolvedDomain::Int(rhs)) => {
                let merged = lhs.iter().chain(rhs.iter()).cloned().collect_vec();
                Ok(UnresolvedDomain::Int(merged))
            }
            (UnresolvedDomain::Int(_), _) | (_, UnresolvedDomain::Int(_)) => {
                Err(DomainOpError::InputWrongType)
            }
            (UnresolvedDomain::Set(_, in1), UnresolvedDomain::Set(_, in2)) => Ok(
                UnresolvedDomain::Set(SetAttr::default(), Moo::new(in1.union(in2)?)),
            ),
            (UnresolvedDomain::Set(_, _), _) | (_, UnresolvedDomain::Set(_, _)) => {
                Err(DomainOpError::InputWrongType)
            }
            (UnresolvedDomain::Matrix(in1, idx1), UnresolvedDomain::Matrix(in2, idx2))
                if idx1 == idx2 =>
            {
                Ok(UnresolvedDomain::Matrix(
                    Moo::new(in1.union(in2)?),
                    idx1.clone(),
                ))
            }
            (UnresolvedDomain::Matrix(_, _), _) | (_, UnresolvedDomain::Matrix(_, _)) => {
                Err(DomainOpError::InputWrongType)
            }
            (UnresolvedDomain::Tuple(lhs), UnresolvedDomain::Tuple(rhs))
                if lhs.len() == rhs.len() =>
            {
                let mut merged = Vec::new();
                for (l, r) in zip(lhs, rhs) {
                    merged.push(Moo::new(l.union(r)?))
                }
                Ok(UnresolvedDomain::Tuple(merged))
            }
            (UnresolvedDomain::Tuple(_), _) | (_, UnresolvedDomain::Tuple(_)) => {
                Err(DomainOpError::InputWrongType)
            }
            // TODO: Could we support unions of reference domains symbolically?
            (UnresolvedDomain::Reference(_), _) | (_, UnresolvedDomain::Reference(_)) => {
                Err(DomainOpError::InputContainsReference)
            }
            // TODO: Could we define semantics for merging record domains?
            (UnresolvedDomain::Record(_), _) | (_, UnresolvedDomain::Record(_)) => {
                Err(DomainOpError::InputWrongType)
            }
        }
    }
}

impl MaybeTypeable for UnresolvedDomain {
    fn maybe_return_type(&self) -> Option<ReturnType> {
        match self {
            UnresolvedDomain::Reference(re) => re.maybe_return_type(),
            UnresolvedDomain::Int(_) => Some(ReturnType::Int),
            UnresolvedDomain::Set(_attr, inner) => inner
                .maybe_return_type()
                .map(|ty| ReturnType::Set(Box::new(ty))),
            UnresolvedDomain::Matrix(inner, _idx) => inner
                .maybe_return_type()
                .map(|ty| ReturnType::Matrix(Box::new(ty))),
            UnresolvedDomain::Tuple(inners) => {
                let mut inner_types = Vec::new();
                for inner in inners {
                    inner_types.push(inner.maybe_return_type()?);
                }
                Some(ReturnType::Tuple(inner_types))
            }
            UnresolvedDomain::Record(entries) => {
                let mut entry_types = Vec::new();
                for entry in entries {
                    entry_types.push(entry.domain.maybe_return_type()?);
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
