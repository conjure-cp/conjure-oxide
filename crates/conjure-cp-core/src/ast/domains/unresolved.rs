use std::fmt::{Display, Formatter};
use std::iter::zip;

use crate::ast::domains::attrs::MSetAttr;
use crate::ast::domains::attrs::PartitionAttr;
use crate::ast::domains::attrs::PermutationAttr;
use crate::ast::domains::attrs::SetAttr;
use crate::ast::domains::ground::{
    FieldGround, int_type_string, representation_attribute, write_int_domain,
};
use crate::ast::records::Field;
use crate::ast::{
    DomainOpError, Expression, FuncAttr, Moo, Reference, RelAttr, ReturnType, SequenceAttr,
    Typeable,
    domains::{DomainPtr, GroundDomain, int_val::IntVal, range::Range},
    pretty::pretty_vec,
};
use crate::bug;

use funcmap::{FuncMap, TryFuncMap};
use itertools::Itertools;
use polyquine::Quine;
use serde::{Deserialize, Serialize};
use uniplate::Uniplate;

pub(super) type FieldUnresolved = Field<DomainPtr>;

impl From<FieldGround> for FieldUnresolved {
    fn from(v: FieldGround) -> Self {
        v.func_map(DomainPtr::from)
    }
}

impl TryFrom<FieldUnresolved> for FieldGround {
    type Error = DomainOpError;
    fn try_from(v: FieldUnresolved) -> Result<Self, Self::Error> {
        v.try_func_map(DomainPtr::try_into)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine, Uniplate)]
#[path_prefix(conjure_cp::ast)]
#[biplate(to=Expression)]
#[biplate(to=Reference)]
#[biplate(to=IntVal)]
#[biplate(to=DomainPtr)]
/// Variants use the project-wide type/domain ordering; keep broad matches in the same order.
pub enum UnresolvedDomain {
    /// An integer domain with an optional representation preference
    Int(Vec<Range<IntVal>>, Option<String>),
    /// An integer domain given by the values of a collection, as in `int([i | i <- nums])`.
    ///
    /// The collection may be built from `given` declarations, so it stays an expression until
    /// those are instantiated and it can be evaluated.
    IntFromValues(Moo<Expression>),
    /// A tuple of N elements, each with its own domain, and an optional representation preference
    Tuple(Vec<DomainPtr>, Option<String>),
    /// A record, with an optional representation preference
    Record(Vec<FieldUnresolved>, Option<String>),
    /// A variant domain with its domain options (reusing field entries), and an optional
    /// representation preference
    Variant(Vec<FieldUnresolved>, Option<String>),
    /// A n-dimensional matrix with a value domain and n-index domains, and an optional
    /// representation preference
    Matrix(DomainPtr, Vec<DomainPtr>, Option<String>),
    Sequence(SequenceAttr<IntVal>, DomainPtr),
    /// A set of elements drawn from the inner domain
    Set(SetAttr<IntVal>, DomainPtr),
    MSet(MSetAttr<IntVal>, DomainPtr),
    /// A function with attributes, domain, and range
    Function(FuncAttr<IntVal>, DomainPtr, DomainPtr),
    /// A relation as a set of tuples
    Relation(RelAttr<IntVal>, Vec<DomainPtr>),
    Partition(PartitionAttr<IntVal>, DomainPtr),
    Permutation(PermutationAttr<IntVal>, DomainPtr),
    /// A reference to a domain letting
    #[polyquine_skip]
    Reference(Reference),
}

impl UnresolvedDomain {
    pub(super) fn from_ground(domain: &GroundDomain) -> Option<UnresolvedDomain> {
        let unresolved = match domain {
            GroundDomain::Empty(_) | GroundDomain::Bool => return None,
            GroundDomain::Int(ranges, representation) => UnresolvedDomain::Int(
                ranges.iter().cloned().map(Into::into).collect(),
                representation.clone(),
            ),
            GroundDomain::Tuple(inners, representation) => UnresolvedDomain::Tuple(
                inners.iter().map(DomainPtr::from).collect(),
                representation.clone(),
            ),
            GroundDomain::Record(fields, representation) => UnresolvedDomain::Record(
                fields.iter().cloned().map(Into::into).collect(),
                representation.clone(),
            ),
            GroundDomain::Variant(fields, representation) => UnresolvedDomain::Variant(
                fields.iter().cloned().map(Into::into).collect(),
                representation.clone(),
            ),
            GroundDomain::Matrix(inner, indices, representation) => UnresolvedDomain::Matrix(
                DomainPtr::from(inner),
                indices.iter().map(DomainPtr::from).collect(),
                representation.clone(),
            ),
            GroundDomain::Sequence(attributes, inner) => {
                UnresolvedDomain::Sequence(attributes.clone().into(), DomainPtr::from(inner))
            }
            GroundDomain::Set(attributes, inner) => {
                UnresolvedDomain::Set(attributes.clone().into(), DomainPtr::from(inner))
            }
            GroundDomain::MSet(attributes, inner) => {
                UnresolvedDomain::MSet(attributes.clone().into(), DomainPtr::from(inner))
            }
            GroundDomain::Function(attributes, domain, codomain) => UnresolvedDomain::Function(
                attributes.clone().into(),
                DomainPtr::from(domain),
                DomainPtr::from(codomain),
            ),
            GroundDomain::Relation(attributes, inners) => UnresolvedDomain::Relation(
                attributes.clone().into(),
                inners.iter().map(DomainPtr::from).collect(),
            ),
            GroundDomain::Partition(attributes, inner) => {
                UnresolvedDomain::Partition(attributes.clone().into(), DomainPtr::from(inner))
            }
            GroundDomain::Permutation(attributes, inner) => {
                UnresolvedDomain::Permutation(attributes.clone().into(), DomainPtr::from(inner))
            }
        };

        Some(unresolved)
    }

    /// Whether this domain takes its values from a collection expression anywhere inside it.
    ///
    /// Such a domain is expensive to resolve -- the collection is evaluated afresh each time -- so
    /// callers ground it once rather than leaving it to be re-resolved on every query.
    pub fn has_int_from_values(&self) -> bool {
        match self {
            UnresolvedDomain::IntFromValues(_) => true,
            UnresolvedDomain::Int(_, _) => false,
            UnresolvedDomain::Tuple(inners, _) | UnresolvedDomain::Relation(_, inners) => {
                inners.iter().any(domain_has_int_from_values)
            }
            UnresolvedDomain::Record(entries, _) | UnresolvedDomain::Variant(entries, _) => entries
                .iter()
                .any(|entry| domain_has_int_from_values(&entry.value)),
            UnresolvedDomain::Matrix(value, indices, _) => {
                domain_has_int_from_values(value) || indices.iter().any(domain_has_int_from_values)
            }
            UnresolvedDomain::Sequence(_, inner)
            | UnresolvedDomain::Set(_, inner)
            | UnresolvedDomain::MSet(_, inner)
            | UnresolvedDomain::Partition(_, inner)
            | UnresolvedDomain::Permutation(_, inner) => domain_has_int_from_values(inner),
            UnresolvedDomain::Function(_, from, to) => {
                domain_has_int_from_values(from) || domain_has_int_from_values(to)
            }
            UnresolvedDomain::Reference(_) => false,
        }
    }

    pub fn resolve(&self) -> Result<GroundDomain, DomainOpError> {
        match self {
            UnresolvedDomain::IntFromValues(expr) => {
                let values = crate::ast::eval::generator_values_from_expr(expr)
                    .ok_or(DomainOpError::NotGround)?;
                let mut ranges = Vec::with_capacity(values.len());
                for value in values {
                    let crate::ast::Literal::Int(value) = value else {
                        return Err(DomainOpError::WrongType);
                    };
                    ranges.push(Range::Single(value));
                }
                Ok(GroundDomain::Int(Range::squeeze(&ranges), None))
            }
            UnresolvedDomain::Int(rngs, representation) => rngs
                .iter()
                .map(Range::<IntVal>::resolve)
                .collect::<Result<Vec<_>, _>>()
                .map(|ranges| {
                    let ranges = ranges
                        .into_iter()
                        .filter(
                            |range| !matches!(range, Range::Bounded(lower, upper) if lower > upper),
                        )
                        .collect::<Vec<_>>();
                    GroundDomain::Int(Range::squeeze(&ranges), representation.clone())
                }),
            UnresolvedDomain::Tuple(inners, representation) => inners
                .iter()
                .map(DomainPtr::resolve)
                .collect::<Result<_, _>>()
                .map(|inners| GroundDomain::Tuple(inners, representation.clone())),
            UnresolvedDomain::Record(entries, representation) => entries
                .iter()
                .map(|f| {
                    f.value.resolve().map(|gd| FieldGround {
                        name: f.name.clone(),
                        value: gd,
                    })
                })
                .collect::<Result<_, _>>()
                .map(|entries| GroundDomain::Record(entries, representation.clone())),
            UnresolvedDomain::Variant(entries, representation) => entries
                .iter()
                .map(|f| {
                    f.value.resolve().map(|gd| FieldGround {
                        name: f.name.clone(),
                        value: gd,
                    })
                })
                .collect::<Result<_, _>>()
                .map(|entries| GroundDomain::Variant(entries, representation.clone())),
            UnresolvedDomain::Matrix(inner, idx_doms, representation) => {
                let inner_gd = inner.resolve()?;
                idx_doms
                    .iter()
                    .map(DomainPtr::resolve)
                    .collect::<Result<_, _>>()
                    .map(|idx| GroundDomain::Matrix(inner_gd, idx, representation.clone()))
            }
            UnresolvedDomain::Sequence(attr, inner) => {
                Ok(GroundDomain::Sequence(attr.resolve()?, inner.resolve()?))
            }
            UnresolvedDomain::Set(attr, inner) => {
                Ok(GroundDomain::Set(attr.resolve()?, inner.resolve()?))
            }
            UnresolvedDomain::MSet(attr, inner) => {
                Ok(GroundDomain::MSet(attr.resolve()?, inner.resolve()?))
            }
            UnresolvedDomain::Function(attr, dom, cdom) => Ok(GroundDomain::Function(
                attr.resolve()?,
                dom.resolve()?,
                cdom.resolve()?,
            )),
            UnresolvedDomain::Relation(attr, inners) => {
                let resolved_attr = attr.resolve()?;
                inners
                    .iter()
                    .map(DomainPtr::resolve)
                    .collect::<Result<_, _>>()
                    .map(|items| GroundDomain::Relation(resolved_attr, items))
            }
            UnresolvedDomain::Partition(attr, inner) => {
                Ok(GroundDomain::Partition(attr.resolve()?, inner.resolve()?))
            }
            UnresolvedDomain::Permutation(attr, inner) => {
                Ok(GroundDomain::Permutation(attr.resolve()?, inner.resolve()?))
            }
            UnresolvedDomain::Reference(re) => re
                .ptr
                .as_domain_letting()
                .unwrap_or_else(|| {
                    bug!("Reference domain should point to domain letting, but got {re}")
                })
                .resolve()
                .map(Moo::unwrap_or_clone),
        }
    }

    pub(super) fn union_unresolved(
        &self,
        other: &UnresolvedDomain,
    ) -> Result<UnresolvedDomain, DomainOpError> {
        // Keep implemented variants before unsupported variants so mixed-domain unions report the
        // established error. Each group uses declaration order.
        match (self, other) {
            (UnresolvedDomain::Int(lhs, _), UnresolvedDomain::Int(rhs, _)) => {
                let merged = lhs.iter().chain(rhs.iter()).cloned().collect_vec();
                Ok(UnresolvedDomain::Int(merged, None))
            }
            (UnresolvedDomain::IntFromValues(_), _) | (_, UnresolvedDomain::IntFromValues(_)) => {
                Err(DomainOpError::NotGround)
            }
            (UnresolvedDomain::Int(_, _), _) | (_, UnresolvedDomain::Int(_, _)) => {
                Err(DomainOpError::WrongType)
            }
            (UnresolvedDomain::Tuple(lhs, _), UnresolvedDomain::Tuple(rhs, _))
                if lhs.len() == rhs.len() =>
            {
                let mut merged = Vec::new();
                for (l, r) in zip(lhs, rhs) {
                    merged.push(l.union(r)?)
                }
                Ok(UnresolvedDomain::Tuple(merged, None))
            }
            (UnresolvedDomain::Tuple(_, _), _) | (_, UnresolvedDomain::Tuple(_, _)) => {
                Err(DomainOpError::WrongType)
            }
            (UnresolvedDomain::Matrix(in1, idx1, _), UnresolvedDomain::Matrix(in2, idx2, _))
                if idx1 == idx2 =>
            {
                Ok(UnresolvedDomain::Matrix(
                    in1.union(in2)?,
                    idx1.clone(),
                    None,
                ))
            }
            (UnresolvedDomain::Matrix(_, _, _), _) | (_, UnresolvedDomain::Matrix(_, _, _)) => {
                Err(DomainOpError::WrongType)
            }
            (UnresolvedDomain::Set(_, in1), UnresolvedDomain::Set(_, in2)) => {
                Ok(UnresolvedDomain::Set(SetAttr::default(), in1.union(in2)?))
            }
            (UnresolvedDomain::Set(_, _), _) | (_, UnresolvedDomain::Set(_, _)) => {
                Err(DomainOpError::WrongType)
            }
            (UnresolvedDomain::MSet(_, in1), UnresolvedDomain::MSet(_, in2)) => {
                Ok(UnresolvedDomain::MSet(MSetAttr::default(), in1.union(in2)?))
            }
            (UnresolvedDomain::MSet(_, _), _) | (_, UnresolvedDomain::MSet(_, _)) => {
                Err(DomainOpError::WrongType)
            }
            (UnresolvedDomain::Relation(_, in1s), UnresolvedDomain::Relation(_, in2s)) => {
                let mut inners = Vec::new();
                for (in1, in2) in in1s.iter().zip(in2s.iter()) {
                    inners.push(in1.union(in2)?)
                }
                Ok(UnresolvedDomain::Relation(RelAttr::default(), inners))
            }
            (UnresolvedDomain::Relation(_, _), _) | (_, UnresolvedDomain::Relation(_, _)) => {
                Err(DomainOpError::WrongType)
            }
            // TODO: Could we define semantics for merging record domains?
            #[allow(unreachable_patterns)]
            (UnresolvedDomain::Record(_, _), _) | (_, UnresolvedDomain::Record(_, _)) => {
                Err(DomainOpError::WrongType)
            }
            #[allow(unreachable_patterns)]
            (UnresolvedDomain::Variant(_, _), _) | (_, UnresolvedDomain::Variant(_, _)) => {
                Err(DomainOpError::WrongType)
            }
            #[allow(unreachable_patterns)]
            (UnresolvedDomain::Sequence(_, _), _) | (_, UnresolvedDomain::Sequence(_, _)) => {
                Err(DomainOpError::WrongType)
            }
            #[allow(unreachable_patterns)]
            (UnresolvedDomain::Function(_, _, _), _) | (_, UnresolvedDomain::Function(_, _, _)) => {
                Err(DomainOpError::WrongType)
            }
            #[allow(unreachable_patterns)]
            (UnresolvedDomain::Partition(_, _), _) | (_, UnresolvedDomain::Partition(_, _)) => {
                Err(DomainOpError::WrongType)
            }
            #[allow(unreachable_patterns)]
            (UnresolvedDomain::Permutation(_, _), _) | (_, UnresolvedDomain::Permutation(_, _)) => {
                Err(DomainOpError::WrongType)
            }
            // TODO: Could we support unions of reference domains symbolically?
            #[allow(unreachable_patterns)]
            (UnresolvedDomain::Reference(_), _) | (_, UnresolvedDomain::Reference(_)) => {
                Err(DomainOpError::NotGround)
            }
        }
    }

    pub fn element_domain(&self) -> Option<DomainPtr> {
        match self {
            UnresolvedDomain::Matrix(inner, _, _) => Some(inner.clone()),
            // A sequence is a function from int(1..|s|), and iterating a function yields its
            // pairs, so iterating a sequence yields (position, value). Mirrors
            // `GroundDomain::element_domain`.
            UnresolvedDomain::Sequence(attr, inner_dom) => {
                let max = match &attr.size {
                    Range::Single(max) | Range::UnboundedL(max) | Range::Bounded(_, max) => {
                        max.clone()
                    }
                    Range::UnboundedR(_) | Range::Unbounded => return None,
                };
                let positions = Moo::new(crate::ast::Domain::Unresolved(Moo::new(
                    UnresolvedDomain::Int(vec![Range::Bounded(IntVal::new_const(1), max)], None),
                )));
                Some(Moo::new(crate::ast::Domain::Unresolved(Moo::new(
                    UnresolvedDomain::Tuple(vec![positions, inner_dom.clone()], None),
                ))))
            }
            UnresolvedDomain::Set(_, inner_dom) => Some(inner_dom.clone()),
            _ => None,
        }
    }

    /// True if any domain in this tree has a representation preference.
    pub fn has_representation_preference(&self) -> bool {
        match self {
            UnresolvedDomain::Int(_, representation) => representation.is_some(),
            UnresolvedDomain::IntFromValues(_) => false,
            UnresolvedDomain::Tuple(inners, representation) => {
                representation.is_some() || inners.iter().any(|d| d.has_representation_preference())
            }
            UnresolvedDomain::Record(entries, representation) => {
                representation.is_some()
                    || entries
                        .iter()
                        .any(|f| f.value.has_representation_preference())
            }
            UnresolvedDomain::Variant(entries, representation) => {
                representation.is_some()
                    || entries
                        .iter()
                        .any(|f| f.value.has_representation_preference())
            }
            UnresolvedDomain::Matrix(inner, idxs, representation) => {
                representation.is_some()
                    || inner.has_representation_preference()
                    || idxs.iter().any(|d| d.has_representation_preference())
            }
            UnresolvedDomain::Sequence(attr, inner) => {
                attr.representation.is_some() || inner.has_representation_preference()
            }
            UnresolvedDomain::Set(attr, inner) => {
                attr.representation.is_some() || inner.has_representation_preference()
            }
            UnresolvedDomain::MSet(attr, inner) => {
                attr.representation.is_some() || inner.has_representation_preference()
            }
            UnresolvedDomain::Function(attr, dom, cdom) => {
                attr.representation.is_some()
                    || dom.has_representation_preference()
                    || cdom.has_representation_preference()
            }
            UnresolvedDomain::Relation(attr, inners) => {
                attr.representation.is_some()
                    || inners.iter().any(|d| d.has_representation_preference())
            }
            UnresolvedDomain::Partition(attr, inner) => {
                attr.representation.is_some() || inner.has_representation_preference()
            }
            UnresolvedDomain::Permutation(attr, inner) => {
                attr.representation.is_some() || inner.has_representation_preference()
            }
            UnresolvedDomain::Reference(re) => re
                .domain()
                .is_some_and(|d| d.has_representation_preference()),
        }
    }

    /// Format this domain in Essence type style, omitting size attributes and integer ranges.
    pub fn as_type_string(&self) -> String {
        match self {
            UnresolvedDomain::Int(_, representation) => int_type_string(representation.as_deref()),
            UnresolvedDomain::IntFromValues(_) => "int".to_string(),
            UnresolvedDomain::Tuple(inners, representation) => {
                let args = representation
                    .iter()
                    .map(|r| format!("representation {r}"))
                    .chain(inners.iter().map(|d| d.as_type_string()))
                    .join(", ");
                format!("tuple ({args})")
            }
            UnresolvedDomain::Record(entries, representation) => {
                let inners = entries
                    .iter()
                    .map(|f| format!("{}: {}", f.name, f.value.as_type_string()))
                    .join(", ");
                format!(
                    "record{} {{{inners}}}",
                    representation_attribute(representation)
                )
            }
            UnresolvedDomain::Variant(entries, representation) => {
                let inners = entries
                    .iter()
                    .map(|f| format!("{}: {}", f.name, f.value.as_type_string()))
                    .join(", ");
                format!(
                    "variant{} {{{inners}}}",
                    representation_attribute(representation)
                )
            }
            UnresolvedDomain::Matrix(inner, idxs, representation) => {
                let idxs = idxs.iter().map(|d| d.as_type_string()).join(", ");
                format!(
                    "matrix{} indexed by [{idxs}] of {}",
                    representation_attribute(representation),
                    inner.as_type_string()
                )
            }
            UnresolvedDomain::Sequence(attrs, inner) => format!(
                "sequence{} of {}",
                representation_attribute(&attrs.representation),
                inner.as_type_string()
            ),
            UnresolvedDomain::Set(attrs, inner) => {
                let mut out = String::from("set");
                if let Some(repr) = &attrs.representation {
                    out.push_str(" (representation ");
                    out.push_str(repr);
                    out.push(')');
                }
                out.push_str(" of ");
                out.push_str(&inner.as_type_string());
                out
            }
            UnresolvedDomain::MSet(attrs, inner) => {
                let mut out = String::from("mset");
                if let Some(repr) = &attrs.representation {
                    out.push_str(" (representation ");
                    out.push_str(repr);
                    out.push(')');
                }
                out.push_str(" of ");
                out.push_str(&inner.as_type_string());
                out
            }
            UnresolvedDomain::Function(attr, dom, cdom) => {
                format!(
                    "function{} {} --> {}",
                    representation_attribute(&attr.representation),
                    dom.as_type_string(),
                    cdom.as_type_string()
                )
            }
            UnresolvedDomain::Relation(attr, inners) => {
                format!(
                    "relation{} of ({})",
                    representation_attribute(&attr.representation),
                    inners.iter().map(|d| d.as_type_string()).join(" * ")
                )
            }
            UnresolvedDomain::Partition(attr, inner) => {
                format!(
                    "partition{} from {}",
                    representation_attribute(&attr.representation),
                    inner.as_type_string()
                )
            }
            UnresolvedDomain::Permutation(attr, inner) => {
                format!(
                    "permutation{} of {}",
                    representation_attribute(&attr.representation),
                    inner.as_type_string()
                )
            }
            UnresolvedDomain::Reference(re) => re.to_string(),
        }
    }
}

impl Typeable for UnresolvedDomain {
    fn return_type(&self) -> ReturnType {
        match self {
            UnresolvedDomain::Int(_, _) | UnresolvedDomain::IntFromValues(_) => ReturnType::Int,
            UnresolvedDomain::Tuple(inners, _) => {
                let mut inner_types = Vec::new();
                for inner in inners {
                    inner_types.push(inner.return_type());
                }
                ReturnType::Tuple(inner_types)
            }
            UnresolvedDomain::Record(entries, _) => {
                let mut entry_types = Vec::new();
                for entry in entries {
                    entry_types.push(entry.clone().func_map(|x| x.return_type()));
                }
                entry_types.sort();
                ReturnType::Record(entry_types)
            }
            UnresolvedDomain::Variant(entries, _) => {
                let mut entry_types = Vec::new();
                for entry in entries {
                    entry_types.push(entry.clone().func_map(|x| x.return_type()));
                }
                ReturnType::Variant(entry_types)
            }
            UnresolvedDomain::Matrix(inner, _idx, _) => {
                ReturnType::Matrix(Box::new(inner.return_type()))
            }
            UnresolvedDomain::Sequence(_attr, inner) => {
                ReturnType::Sequence(Box::new(inner.return_type()))
            }
            UnresolvedDomain::Set(_attr, inner) => ReturnType::Set(Box::new(inner.return_type())),
            UnresolvedDomain::MSet(_attr, inner) => ReturnType::MSet(Box::new(inner.return_type())),
            UnresolvedDomain::Function(_, dom, cdom) => {
                ReturnType::Function(Box::new(dom.return_type()), Box::new(cdom.return_type()))
            }
            UnresolvedDomain::Relation(_, inners) => {
                let mut inner_types = Vec::new();
                for inner in inners {
                    inner_types.push(inner.return_type());
                }
                ReturnType::Relation(inner_types)
            }
            UnresolvedDomain::Partition(_, inner) => {
                ReturnType::Partition(Box::new(inner.return_type()))
            }
            UnresolvedDomain::Permutation(_, inner) => {
                ReturnType::Permutation(Box::new(inner.return_type()))
            }
            UnresolvedDomain::Reference(re) => re.return_type(),
        }
    }
}

impl Display for FieldUnresolved {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.name, self.value)
    }
}

impl Display for UnresolvedDomain {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            UnresolvedDomain::Int(ranges, representation) => {
                write_int_domain(f, ranges, representation.as_deref())
            }
            UnresolvedDomain::IntFromValues(expr) => write!(f, "int({expr})"),
            UnresolvedDomain::Tuple(domains, representation) => {
                // Members have always been printed without a space after the comma here.
                let members = domains.iter().join(",");
                match representation {
                    Some(representation) if members.is_empty() => {
                        write!(f, "tuple (representation {representation})")
                    }
                    Some(representation) => {
                        write!(f, "tuple (representation {representation}, {members})")
                    }
                    None => write!(f, "tuple ({members})"),
                }
            }
            UnresolvedDomain::Record(entries, representation) => {
                let inners = entries.iter().map(|t| format!("{}", t)).join(", ");
                let attrs = representation_attribute(representation);
                write!(f, "record{attrs} {{{inners}}}",)
            }
            UnresolvedDomain::Variant(entries, representation) => {
                let inners = entries.iter().map(|t| format!("{}", t)).join(", ");
                let attrs = representation_attribute(representation);
                write!(f, "variant{attrs} {{{inners}}}",)
            }
            UnresolvedDomain::Matrix(value_domain, index_domains, representation) => {
                write!(
                    f,
                    "matrix{} indexed by {} of {value_domain}",
                    representation_attribute(representation),
                    pretty_vec(&index_domains.iter().collect_vec())
                )
            }
            UnresolvedDomain::Sequence(attrs, inner_dom) => {
                write!(f, "sequence {attrs} of {inner_dom}")
            }
            UnresolvedDomain::Set(attrs, inner_dom) => {
                write!(f, "set")?;
                let attrs = attrs.to_string();
                if attrs.is_empty() {
                    write!(f, " of {inner_dom}")
                } else {
                    write!(f, " {attrs} of {inner_dom}")
                }
            }
            UnresolvedDomain::MSet(attrs, inner_dom) => {
                write!(f, "mset")?;
                let attrs = attrs.to_string();
                if attrs.is_empty() {
                    write!(f, " of {inner_dom}")
                } else {
                    write!(f, " {attrs} of {inner_dom}")
                }
            }
            UnresolvedDomain::Function(attribute, domain, codomain) => {
                write!(f, "function {} {} --> {} ", attribute, domain, codomain)
            }
            UnresolvedDomain::Relation(attrs, domains) => {
                write!(f, "relation {} of ({})", attrs, domains.iter().join(" * "))
            }
            UnresolvedDomain::Partition(attrs, inner_dom) => {
                write!(f, "partition {attrs} from {inner_dom}")
            }
            UnresolvedDomain::Permutation(attrs, inner_dom) => {
                write!(f, "permutation {attrs} of {inner_dom}")
            }
            UnresolvedDomain::Reference(re) => write!(f, "{re}"),
        }
    }
}

/// Whether `domain` takes its values from a collection expression anywhere inside it.
pub fn domain_has_int_from_values(domain: &DomainPtr) -> bool {
    match &**domain {
        crate::ast::Domain::Ground(_) => false,
        crate::ast::Domain::Unresolved(unresolved) => unresolved.has_int_from_values(),
    }
}
