use crate::ast::domains::attrs::SetAttr;
use crate::ast::{
    DeclarationKind, DomainOpError, Expression, FuncAttr, Literal, Metadata, Moo,
    RecordEntryGround, Reference, Typeable,
    domains::{
        GroundDomain,
        domain::{DomainPtr, Int},
        range::Range,
    },
};
use crate::{bug, domain_int, matrix_expr, range};
use conjure_cp_core::ast::pretty::pretty_vec;
use conjure_cp_core::ast::{Name, ReturnType, eval_constant};
use itertools::Itertools;
use polyquine::Quine;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::iter::zip;
use std::ops::Deref;
use uniplate::Uniplate;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine, Uniplate)]
#[path_prefix(conjure_cp::ast)]
#[biplate(to=Expression)]
#[biplate(to=Reference)]
pub enum IntVal {
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
            _ => Err(DomainOpError::NotGround),
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
        let size: Range<Int> = self.size.try_into()?;
        Ok(SetAttr { size })
    }
}

impl From<FuncAttr<Int>> for FuncAttr<IntVal> {
    fn from(value: FuncAttr<Int>) -> Self {
        FuncAttr {
            size: value.size.into(),
            partiality: value.partiality,
            jectivity: value.jectivity,
        }
    }
}

impl TryInto<FuncAttr<Int>> for FuncAttr<IntVal> {
    type Error = DomainOpError;

    fn try_into(self) -> Result<FuncAttr<Int>, Self::Error> {
        let size: Range<Int> = self.size.try_into()?;
        Ok(FuncAttr {
            size,
            jectivity: self.jectivity,
            partiality: self.partiality,
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
            DeclarationKind::ValueLetting(expr) => match expr.return_type() {
                ReturnType::Int => Some(IntVal::Reference(re.clone())),
                _ => None,
            },
            DeclarationKind::Given(dom) => match dom.return_type() {
                ReturnType::Int => Some(IntVal::Reference(re.clone())),
                _ => None,
            },
            DeclarationKind::GivenQuantified(inner) => match inner.domain().return_type() {
                ReturnType::Int => Some(IntVal::Reference(re.clone())),
                _ => None,
            },
            DeclarationKind::DomainLetting(_)
            | DeclarationKind::RecordField(_)
            | DeclarationKind::DecisionVariable(_) => None,
        }
    }

    pub fn new_expr(value: Moo<Expression>) -> Option<IntVal> {
        if value.return_type() != ReturnType::Int {
            return None;
        }
        Some(IntVal::Expr(value))
    }

    pub fn resolve(&self) -> Option<Int> {
        match self {
            IntVal::Const(value) => Some(*value),
            IntVal::Expr(expr) => match eval_constant(expr)? {
                Literal::Int(v) => Some(v),
                _ => bug!("Expected integer expression, got: {expr}"),
            },
            IntVal::Reference(re) => match re.ptr.kind().deref() {
                DeclarationKind::ValueLetting(expr) => match eval_constant(expr)? {
                    Literal::Int(v) => Some(v),
                    _ => bug!("Expected integer expression, got: {expr}"),
                },
                // If this is an int given we will be able to resolve it eventually, but not yet
                DeclarationKind::Given(_) | DeclarationKind::GivenQuantified(..) => None,
                DeclarationKind::DomainLetting(_)
                | DeclarationKind::RecordField(_)
                | DeclarationKind::DecisionVariable(_) => bug!(
                    "Expected integer expression, given, or letting inside int domain; Got: {re}"
                ),
            },
        }
    }
}

impl From<IntVal> for Expression {
    fn from(value: IntVal) -> Self {
        match value {
            IntVal::Const(val) => val.into(),
            IntVal::Reference(re) => re.into(),
            IntVal::Expr(expr) => expr.as_ref().clone(),
        }
    }
}

impl From<IntVal> for Moo<Expression> {
    fn from(value: IntVal) -> Self {
        match value {
            IntVal::Const(val) => Moo::new(val.into()),
            IntVal::Reference(re) => Moo::new(re.into()),
            IntVal::Expr(expr) => expr,
        }
    }
}

impl std::ops::Neg for IntVal {
    type Output = IntVal;

    fn neg(self) -> Self::Output {
        match self {
            IntVal::Const(val) => IntVal::Const(-val),
            IntVal::Reference(_) | IntVal::Expr(_) => {
                IntVal::Expr(Moo::new(Expression::Neg(Metadata::new(), self.into())))
            }
        }
    }
}

impl<T> std::ops::Add<T> for IntVal
where
    T: Into<Expression>,
{
    type Output = IntVal;

    fn add(self, rhs: T) -> Self::Output {
        let lhs: Expression = self.into();
        let rhs: Expression = rhs.into();
        let sum = matrix_expr!(lhs, rhs; domain_int!(1..));
        IntVal::Expr(Moo::new(Expression::Sum(Metadata::new(), Moo::new(sum))))
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

impl FuncAttr<IntVal> {
    pub fn resolve(&self) -> Option<FuncAttr<Int>> {
        Some(FuncAttr {
            size: self.size.resolve()?,
            partiality: self.partiality.clone(),
            jectivity: self.jectivity.clone(),
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

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine, Uniplate)]
#[path_prefix(conjure_cp::ast)]
#[biplate(to=Expression)]
#[biplate(to=Reference)]
#[biplate(to=IntVal)]
#[biplate(to=DomainPtr)]
#[biplate(to=RecordEntry)]
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
    /// A record
    Record(Vec<RecordEntry>),
    /// A function with attributes, domain, and range
    Function(FuncAttr<IntVal>, DomainPtr, DomainPtr),
}

impl UnresolvedDomain {
    pub fn resolve(&self) -> Option<GroundDomain> {
        match self {
            UnresolvedDomain::Int(rngs) => rngs
                .iter()
                .map(Range::<IntVal>::resolve)
                .collect::<Option<_>>()
                .map(GroundDomain::Int),
            UnresolvedDomain::Set(attr, inner) => {
                Some(GroundDomain::Set(attr.resolve()?, inner.resolve()?))
            }
            UnresolvedDomain::Matrix(inner, idx_doms) => {
                let inner_gd = inner.resolve()?;
                idx_doms
                    .iter()
                    .map(DomainPtr::resolve)
                    .collect::<Option<_>>()
                    .map(|idx| GroundDomain::Matrix(inner_gd, idx))
            }
            UnresolvedDomain::Tuple(inners) => inners
                .iter()
                .map(DomainPtr::resolve)
                .collect::<Option<_>>()
                .map(GroundDomain::Tuple),
            UnresolvedDomain::Record(entries) => entries
                .iter()
                .map(|f| {
                    f.domain.resolve().map(|gd| RecordEntryGround {
                        name: f.name.clone(),
                        domain: gd,
                    })
                })
                .collect::<Option<_>>()
                .map(GroundDomain::Record),
            UnresolvedDomain::Reference(re) => re
                .ptr
                .as_domain_letting()
                .unwrap_or_else(|| {
                    bug!("Reference domain should point to domain letting, but got {re}")
                })
                .resolve()
                .map(Moo::unwrap_or_clone),
            UnresolvedDomain::Function(attr, dom, cdom) => {
                if let Some(attr_gd) = attr.resolve()
                    && let Some(dom_gd) = dom.resolve()
                    && let Some(cdom_gd) = cdom.resolve()
                {
                    return Some(GroundDomain::Function(attr_gd, dom_gd, cdom_gd));
                }
                None
            }
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
                Err(DomainOpError::WrongType)
            }
            (UnresolvedDomain::Set(_, in1), UnresolvedDomain::Set(_, in2)) => {
                Ok(UnresolvedDomain::Set(SetAttr::default(), in1.union(in2)?))
            }
            (UnresolvedDomain::Set(_, _), _) | (_, UnresolvedDomain::Set(_, _)) => {
                Err(DomainOpError::WrongType)
            }
            (UnresolvedDomain::Matrix(in1, idx1), UnresolvedDomain::Matrix(in2, idx2))
                if idx1 == idx2 =>
            {
                Ok(UnresolvedDomain::Matrix(in1.union(in2)?, idx1.clone()))
            }
            (UnresolvedDomain::Matrix(_, _), _) | (_, UnresolvedDomain::Matrix(_, _)) => {
                Err(DomainOpError::WrongType)
            }
            (UnresolvedDomain::Tuple(lhs), UnresolvedDomain::Tuple(rhs))
                if lhs.len() == rhs.len() =>
            {
                let mut merged = Vec::new();
                for (l, r) in zip(lhs, rhs) {
                    merged.push(l.union(r)?)
                }
                Ok(UnresolvedDomain::Tuple(merged))
            }
            (UnresolvedDomain::Tuple(_), _) | (_, UnresolvedDomain::Tuple(_)) => {
                Err(DomainOpError::WrongType)
            }
            // TODO: Could we support unions of reference domains symbolically?
            (UnresolvedDomain::Reference(_), _) | (_, UnresolvedDomain::Reference(_)) => {
                Err(DomainOpError::NotGround)
            }
            // TODO: Could we define semantics for merging record domains?
            #[allow(unreachable_patterns)] // Technically redundant but logically makes sense
            (UnresolvedDomain::Record(_), _) | (_, UnresolvedDomain::Record(_)) => {
                Err(DomainOpError::WrongType)
            }
            #[allow(unreachable_patterns)]
            // Technically redundant but logically clearer to have both
            (UnresolvedDomain::Function(_, _, _), _) | (_, UnresolvedDomain::Function(_, _, _)) => {
                Err(DomainOpError::WrongType)
            }
        }
    }

    pub fn element_domain(&self) -> Option<DomainPtr> {
        match self {
            UnresolvedDomain::Set(_, inner_dom) => Some(inner_dom.clone()),
            UnresolvedDomain::Matrix(_, _) => {
                todo!("Unwrap one dimension of the domain")
            }
            _ => None,
        }
    }
}

impl Typeable for UnresolvedDomain {
    fn return_type(&self) -> ReturnType {
        match self {
            UnresolvedDomain::Reference(re) => re.return_type(),
            UnresolvedDomain::Int(_) => ReturnType::Int,
            UnresolvedDomain::Set(_attr, inner) => ReturnType::Set(Box::new(inner.return_type())),
            UnresolvedDomain::Matrix(inner, _idx) => {
                ReturnType::Matrix(Box::new(inner.return_type()))
            }
            UnresolvedDomain::Tuple(inners) => {
                let mut inner_types = Vec::new();
                for inner in inners {
                    inner_types.push(inner.return_type());
                }
                ReturnType::Tuple(inner_types)
            }
            UnresolvedDomain::Record(entries) => {
                let mut entry_types = Vec::new();
                for entry in entries {
                    entry_types.push(entry.domain.return_type());
                }
                ReturnType::Record(entry_types)
            }
            UnresolvedDomain::Function(_, dom, cdom) => {
                ReturnType::Function(Box::new(dom.return_type()), Box::new(cdom.return_type()))
            }
        }
    }
}

impl Display for UnresolvedDomain {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            UnresolvedDomain::Reference(re) => write!(f, "{re}"),
            UnresolvedDomain::Int(ranges) => {
                if ranges.iter().all(Range::is_lower_or_upper_bounded) {
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
            UnresolvedDomain::Function(attribute, domain, codomain) => {
                write!(f, "function {} {} --> {} ", attribute, domain, codomain)
            }
        }
    }
}
