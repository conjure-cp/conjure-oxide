use super::attrs::SetAttr;
use super::ground::GroundDomain;
use super::range::Range;
use super::unresolved::{IntVal, UnresolvedDomain};
use crate::ast::{
    DeclarationPtr, DomainOpError, Expression, FuncAttr, Literal, Moo, RecordEntry,
    RecordEntryGround, Reference, ReturnType, Typeable,
};
use itertools::Itertools;
use polyquine::Quine;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::thread_local;
use uniplate::Uniplate;

/// The integer type used in all domain code (int ranges, set sizes, etc)
pub type Int = i32;

pub type DomainPtr = Moo<Domain>;

impl DomainPtr {
    pub fn resolve(&self) -> Option<Moo<GroundDomain>> {
        self.as_ref().resolve()
    }

    /// Convenience method to take [Domain::union] of the [Domain]s behind two [DomainPtr]s
    /// and wrap the result in a new [DomainPtr].
    pub fn union(&self, other: &DomainPtr) -> Result<DomainPtr, DomainOpError> {
        self.as_ref().union(other.as_ref()).map(DomainPtr::new)
    }

    /// Convenience method to take [Domain::intersect] of the [Domain]s behind two [DomainPtr]s
    /// and wrap the result in a new [DomainPtr].
    pub fn intersect(&self, other: &DomainPtr) -> Result<DomainPtr, DomainOpError> {
        self.as_ref().intersect(other.as_ref()).map(DomainPtr::new)
    }
}

impl From<Moo<GroundDomain>> for DomainPtr {
    fn from(value: Moo<GroundDomain>) -> Self {
        Moo::new(Domain::Ground(value))
    }
}

impl From<Moo<UnresolvedDomain>> for DomainPtr {
    fn from(value: Moo<UnresolvedDomain>) -> Self {
        Moo::new(Domain::Unresolved(value))
    }
}

impl From<GroundDomain> for DomainPtr {
    fn from(value: GroundDomain) -> Self {
        Moo::new(Domain::Ground(Moo::new(value)))
    }
}

impl From<UnresolvedDomain> for DomainPtr {
    fn from(value: UnresolvedDomain) -> Self {
        Moo::new(Domain::Unresolved(Moo::new(value)))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine, Uniplate)]
#[biplate(to=DomainPtr)]
#[biplate(to=GroundDomain)]
#[biplate(to=UnresolvedDomain)]
#[biplate(to=Expression)]
#[biplate(to=Reference)]
#[biplate(to=RecordEntry)]
#[biplate(to=IntVal)]
#[path_prefix(conjure_cp::ast)]
pub enum Domain {
    /// A fully resolved domain
    Ground(Moo<GroundDomain>),
    /// A domain which may contain references
    Unresolved(Moo<UnresolvedDomain>),
}

/// Types that have a [`Domain`].
pub trait HasDomain {
    /// Gets the [`Domain`] of `self`.
    fn domain_of(&self) -> DomainPtr;
}

impl<T: HasDomain> Typeable for T {
    fn return_type(&self) -> ReturnType {
        self.domain_of().return_type()
    }
}

// Domain::Bool is completely static, so reuse the same chunk of memory
// for all bool domains to avoid many small memory allocations
thread_local! {
    static BOOL_DOMAIN: DomainPtr =
        Moo::new(Domain::Ground(Moo::new(GroundDomain::Bool)));
}

impl Domain {
    /// Create a new boolean domain and return a pointer to it.
    /// Boolean domains are always ground (see [GroundDomain::Bool]).
    pub fn bool() -> DomainPtr {
        BOOL_DOMAIN.with(Clone::clone)
    }

    /// Create a new empty domain of the given type and return a pointer to it.
    /// Empty domains are always ground (see [GroundDomain::Empty]).
    pub fn empty(ty: ReturnType) -> DomainPtr {
        Moo::new(Domain::Ground(Moo::new(GroundDomain::Empty(ty))))
    }

    /// Create a new int domain with the given ranges.
    /// If the ranges are all ground, the variant will be [GroundDomain::Int].
    /// Otherwise, it will be [UnresolvedDomain::Int].
    pub fn int<T>(ranges: Vec<T>) -> DomainPtr
    where
        T: Into<Range<IntVal>> + TryInto<Range<Int>> + Clone,
    {
        if let Ok(int_rngs) = ranges
            .iter()
            .cloned()
            .map(TryInto::try_into)
            .collect::<Result<Vec<_>, _>>()
        {
            return Domain::int_ground(int_rngs);
        }
        let unresolved_rngs: Vec<Range<IntVal>> = ranges.into_iter().map(Into::into).collect();
        Moo::new(Domain::Unresolved(Moo::new(UnresolvedDomain::Int(
            unresolved_rngs,
        ))))
    }

    /// Create a new ground integer domain with the given ranges
    pub fn int_ground(ranges: Vec<Range<Int>>) -> DomainPtr {
        let rngs = Range::squeeze(&ranges);
        Moo::new(Domain::Ground(Moo::new(GroundDomain::Int(rngs))))
    }

    /// Create a new set domain with the given element domain and attributes.
    /// If the element domain and the attributes are ground, the variant
    /// will be [GroundDomain::Set]. Otherwise, it will be [UnresolvedDomain::Set].
    pub fn set<T>(attr: T, inner_dom: DomainPtr) -> DomainPtr
    where
        T: Into<SetAttr<IntVal>> + TryInto<SetAttr<Int>> + Clone,
    {
        if let Domain::Ground(gd) = inner_dom.as_ref()
            && let Ok(int_attr) = attr.clone().try_into()
        {
            return Moo::new(Domain::Ground(Moo::new(GroundDomain::Set(
                int_attr,
                gd.clone(),
            ))));
        }
        Moo::new(Domain::Unresolved(Moo::new(UnresolvedDomain::Set(
            attr.into(),
            inner_dom,
        ))))
    }

    /// Create a new matrix domain with the given element domain and index domains.
    /// If the given domains are all ground, the variant will be [GroundDomain::Matrix].
    /// Otherwise, it will be [UnresolvedDomain::Matrix].
    pub fn matrix(inner_dom: DomainPtr, idx_doms: Vec<DomainPtr>) -> DomainPtr {
        if let Domain::Ground(gd) = inner_dom.as_ref()
            && let Some(idx_gds) = as_grounds(&idx_doms)
        {
            return Moo::new(Domain::Ground(Moo::new(GroundDomain::Matrix(
                gd.clone(),
                idx_gds,
            ))));
        }
        Moo::new(Domain::Unresolved(Moo::new(UnresolvedDomain::Matrix(
            inner_dom, idx_doms,
        ))))
    }

    /// Create a new tuple domain with the given element domains.
    /// If the given domains are all ground, the variant will be [GroundDomain::Tuple].
    /// Otherwise, it will be [UnresolvedDomain::Tuple].
    pub fn tuple(inner_doms: Vec<DomainPtr>) -> DomainPtr {
        if let Some(inner_gds) = as_grounds(&inner_doms) {
            return Moo::new(Domain::Ground(Moo::new(GroundDomain::Tuple(inner_gds))));
        }
        Moo::new(Domain::Unresolved(Moo::new(UnresolvedDomain::Tuple(
            inner_doms,
        ))))
    }

    /// Create a new tuple domain with the given entries.
    /// If the entries are all ground, the variant will be [GroundDomain::Record].
    /// Otherwise, it will be [UnresolvedDomain::Record].
    pub fn record(entries: Vec<RecordEntry>) -> DomainPtr {
        if let Ok(entries_gds) = entries.iter().cloned().map(TryInto::try_into).try_collect() {
            return Moo::new(Domain::Ground(Moo::new(GroundDomain::Record(entries_gds))));
        }
        Moo::new(Domain::Unresolved(Moo::new(UnresolvedDomain::Record(
            entries,
        ))))
    }

    /// Create a new [UnresolvedDomain::Reference] domain from a domain letting
    pub fn reference(ptr: DeclarationPtr) -> Option<DomainPtr> {
        ptr.as_domain_letting()?;
        Some(Moo::new(Domain::Unresolved(Moo::new(
            UnresolvedDomain::Reference(Reference::new(ptr)),
        ))))
    }

    /// Create a new function domain
    pub fn function<T>(attrs: T, dom: DomainPtr, cdom: DomainPtr) -> DomainPtr
    where
        T: Into<FuncAttr<IntVal>> + TryInto<FuncAttr<Int>> + Clone,
    {
        if let Ok(attrs_gd) = attrs.clone().try_into()
            && let Some(dom_gd) = dom.as_ground()
            && let Some(cdom_gd) = cdom.as_ground()
        {
            return Moo::new(Domain::Ground(Moo::new(GroundDomain::Function(
                attrs_gd,
                Moo::new(dom_gd.clone()),
                Moo::new(cdom_gd.clone()),
            ))));
        }

        Moo::new(Domain::Unresolved(Moo::new(UnresolvedDomain::Function(
            attrs.into(),
            dom,
            cdom,
        ))))
    }

    /// If this domain is ground, return a [Moo] to the underlying [GroundDomain].
    /// Otherwise, try to resolve it; Return None if this is not yet possible.
    /// Domains which contain references to givens cannot be resolved until these
    /// givens are substituted for their concrete values.
    pub fn resolve(&self) -> Option<Moo<GroundDomain>> {
        match self {
            Domain::Ground(gd) => Some(gd.clone()),
            Domain::Unresolved(ud) => ud.resolve().map(Moo::new),
        }
    }

    /// If this domain is already ground, return a reference to the underlying [GroundDomain].
    /// Otherwise, return None. This method does NOT perform any resolution.
    /// See also: [Domain::resolve].
    pub fn as_ground(&self) -> Option<&GroundDomain> {
        match self {
            Domain::Ground(gd) => Some(gd.as_ref()),
            _ => None,
        }
    }

    /// If this domain is already ground, return a mutable reference to the underlying [GroundDomain].
    /// Otherwise, return None. This method does NOT perform any resolution.
    pub fn as_ground_mut(&mut self) -> Option<&mut GroundDomain> {
        match self {
            Domain::Ground(gd) => Some(Moo::<GroundDomain>::make_mut(gd)),
            _ => None,
        }
    }

    /// If this domain is unresolved, return a reference to the underlying [UnresolvedDomain].
    pub fn as_unresolved(&self) -> Option<&UnresolvedDomain> {
        match self {
            Domain::Unresolved(ud) => Some(ud.as_ref()),
            _ => None,
        }
    }

    /// If this domain is unresolved, return a mutable reference to the underlying [UnresolvedDomain].
    pub fn as_unresolved_mut(&mut self) -> Option<&mut UnresolvedDomain> {
        match self {
            Domain::Unresolved(ud) => Some(Moo::<UnresolvedDomain>::make_mut(ud)),
            _ => None,
        }
    }

    /// If this is [GroundDomain::Empty(ty)], get a reference to the return type `ty`
    pub fn as_dom_empty(&self) -> Option<&ReturnType> {
        if let Some(GroundDomain::Empty(ty)) = self.as_ground() {
            return Some(ty);
        }
        None
    }

    /// If this is [GroundDomain::Empty(ty)], get a mutable reference to the return type `ty`
    pub fn as_dom_empty_mut(&mut self) -> Option<&mut ReturnType> {
        if let Some(GroundDomain::Empty(ty)) = self.as_ground_mut() {
            return Some(ty);
        }
        None
    }

    /// True if this is [GroundDomain::Bool]
    pub fn is_bool(&self) -> bool {
        self.return_type() == ReturnType::Bool
    }

    /// True if this is a [GroundDomain::Int] or an [UnresolvedDomain::Int]
    pub fn is_int(&self) -> bool {
        self.return_type() == ReturnType::Int
    }

    /// If this domain is [GroundDomain::Int] or [UnresolveDomain::Int], get
    /// its ranges. The ranges are cloned and upcast to Range<IntVal> if necessary.
    pub fn as_int(&self) -> Option<Vec<Range<IntVal>>> {
        if let Some(GroundDomain::Int(rngs)) = self.as_ground() {
            return Some(rngs.iter().cloned().map(|r| r.into()).collect());
        }
        if let Some(UnresolvedDomain::Int(rngs)) = self.as_unresolved() {
            return Some(rngs.clone());
        }
        None
    }

    /// If this is an int domain, get a mutable reference to its ranges.
    /// The domain always becomes [UnresolvedDomain::Int] after this operation.
    pub fn as_int_mut(&mut self) -> Option<&mut Vec<Range<IntVal>>> {
        // We're "upcasting" ground ranges (Range<Int>) to the more general
        // Range<IntVal>, which may contain references or expressions.
        // We know that for now they are still ground, but we're giving the user a mutable
        // reference, so they can overwrite the ranges with values that aren't ground.
        // So, the entire domain has to become non-ground as well.
        if let Some(GroundDomain::Int(rngs_gds)) = self.as_ground() {
            let rngs: Vec<Range<IntVal>> = rngs_gds.iter().cloned().map(|r| r.into()).collect();
            *self = Domain::Unresolved(Moo::new(UnresolvedDomain::Int(rngs)))
        }

        if let Some(UnresolvedDomain::Int(rngs)) = self.as_unresolved_mut() {
            return Some(rngs);
        }
        None
    }

    /// If this is a [GroundDomain::Int(rngs)], get an immutable reference to rngs.
    pub fn as_int_ground(&self) -> Option<&Vec<Range<Int>>> {
        if let Some(GroundDomain::Int(rngs)) = self.as_ground() {
            return Some(rngs);
        }
        None
    }

    /// If this is a [GroundDomain::Int(rngs)], get an immutable reference to rngs.
    pub fn as_int_ground_mut(&mut self) -> Option<&mut Vec<Range<Int>>> {
        if let Some(GroundDomain::Int(rngs)) = self.as_ground_mut() {
            return Some(rngs);
        }
        None
    }

    /// If this is a matrix domain, get pointers to its element domain
    /// and index domains.
    pub fn as_matrix(&self) -> Option<(DomainPtr, Vec<DomainPtr>)> {
        if let Some(GroundDomain::Matrix(inner_dom_gd, idx_doms_gds)) = self.as_ground() {
            let idx_doms: Vec<DomainPtr> = idx_doms_gds.iter().cloned().map(|d| d.into()).collect();
            let inner_dom: DomainPtr = inner_dom_gd.clone().into();
            return Some((inner_dom, idx_doms));
        }
        if let Some(UnresolvedDomain::Matrix(inner_dom, idx_doms)) = self.as_unresolved() {
            return Some((inner_dom.clone(), idx_doms.clone()));
        }
        None
    }

    /// If this is a matrix domain, get mutable references to its element
    /// domain and its vector of index domains.
    /// The domain always becomes [UnresolvedDomain::Matrix] after this operation.
    pub fn as_matrix_mut(&mut self) -> Option<(&mut DomainPtr, &mut Vec<DomainPtr>)> {
        // "upcast" the entire domain to UnresolvedDomain
        // See [Domain::as_dom_int_mut] for an explanation of why this is necessary
        if let Some(GroundDomain::Matrix(inner_dom_gd, idx_doms_gds)) = self.as_ground() {
            let inner_dom: DomainPtr = inner_dom_gd.clone().into();
            let idx_doms: Vec<DomainPtr> = idx_doms_gds.iter().cloned().map(|d| d.into()).collect();
            *self = Domain::Unresolved(Moo::new(UnresolvedDomain::Matrix(inner_dom, idx_doms)));
        }

        if let Some(UnresolvedDomain::Matrix(inner_dom, idx_doms)) = self.as_unresolved_mut() {
            return Some((inner_dom, idx_doms));
        }
        None
    }

    /// If this is a [GroundDomain::Matrix], get immutable references to its element and index domains
    pub fn as_matrix_ground(&self) -> Option<(&Moo<GroundDomain>, &Vec<Moo<GroundDomain>>)> {
        if let Some(GroundDomain::Matrix(inner_dom, idx_doms)) = self.as_ground() {
            return Some((inner_dom, idx_doms));
        }
        None
    }

    /// If this is a [GroundDomain::Matrix], get mutable references to its element and index domains
    pub fn as_matrix_ground_mut(
        &mut self,
    ) -> Option<(&mut Moo<GroundDomain>, &mut Vec<Moo<GroundDomain>>)> {
        if let Some(GroundDomain::Matrix(inner_dom, idx_doms)) = self.as_ground_mut() {
            return Some((inner_dom, idx_doms));
        }
        None
    }

    /// If this is a set domain, get its attributes and a pointer to its element domain.
    pub fn as_set(&self) -> Option<(SetAttr<IntVal>, DomainPtr)> {
        if let Some(GroundDomain::Set(attr, inner_dom)) = self.as_ground() {
            return Some((attr.clone().into(), inner_dom.clone().into()));
        }
        if let Some(UnresolvedDomain::Set(attr, inner_dom)) = self.as_unresolved() {
            return Some((attr.clone(), inner_dom.clone()));
        }
        None
    }

    /// If this is a set domain, get mutable reference to its attributes and element domain.
    /// The domain always becomes [UnresolvedDomain::Set] after this operation.
    pub fn as_set_mut(&mut self) -> Option<(&mut SetAttr<IntVal>, &mut DomainPtr)> {
        if let Some(GroundDomain::Set(attr_gd, inner_dom_gd)) = self.as_ground() {
            let attr: SetAttr<IntVal> = attr_gd.clone().into();
            let inner_dom = inner_dom_gd.clone().into();
            *self = Domain::Unresolved(Moo::new(UnresolvedDomain::Set(attr, inner_dom)));
        }

        if let Some(UnresolvedDomain::Set(attr, inner_dom)) = self.as_unresolved_mut() {
            return Some((attr, inner_dom));
        }
        None
    }

    /// If this is a [GroundDomain::Set], get immutable references to its attributes and inner domain
    pub fn as_set_ground(&self) -> Option<(&SetAttr<Int>, &Moo<GroundDomain>)> {
        if let Some(GroundDomain::Set(attr, inner_dom)) = self.as_ground() {
            return Some((attr, inner_dom));
        }
        None
    }

    /// If this is a [GroundDomain::Set], get mutable references to its attributes and inner domain
    pub fn as_set_ground_mut(&mut self) -> Option<(&mut SetAttr<Int>, &mut Moo<GroundDomain>)> {
        if let Some(GroundDomain::Set(attr, inner_dom)) = self.as_ground_mut() {
            return Some((attr, inner_dom));
        }
        None
    }

    /// If this is a tuple domain, get pointers to its element domains.
    pub fn as_tuple(&self) -> Option<Vec<DomainPtr>> {
        if let Some(GroundDomain::Tuple(inner_doms)) = self.as_ground() {
            return Some(inner_doms.iter().cloned().map(|d| d.into()).collect());
        }
        if let Some(UnresolvedDomain::Tuple(inner_doms)) = self.as_unresolved() {
            return Some(inner_doms.clone());
        }
        None
    }

    /// If this is a tuple domain, get a mutable reference to its vector of element domains.
    /// The domain always becomes [UnresolvedDomain::Tuple] after this operation.
    pub fn as_tuple_mut(&mut self) -> Option<&mut Vec<DomainPtr>> {
        if let Some(GroundDomain::Tuple(inner_doms_gds)) = self.as_ground() {
            let inner_doms: Vec<DomainPtr> =
                inner_doms_gds.iter().cloned().map(|d| d.into()).collect();
            *self = Domain::Unresolved(Moo::new(UnresolvedDomain::Tuple(inner_doms)));
        }

        if let Some(UnresolvedDomain::Tuple(inner_doms)) = self.as_unresolved_mut() {
            return Some(inner_doms);
        }
        None
    }

    /// If this is a [GroundDomain::Tuple], get immutable references to its element domains
    pub fn as_tuple_ground(&self) -> Option<&Vec<Moo<GroundDomain>>> {
        if let Some(GroundDomain::Tuple(inner_doms)) = self.as_ground() {
            return Some(inner_doms);
        }
        None
    }

    /// If this is a [GroundDomain::Tuple], get mutable reference to its element domains
    pub fn as_tuple_ground_mut(&mut self) -> Option<&mut Vec<Moo<GroundDomain>>> {
        if let Some(GroundDomain::Tuple(inner_doms)) = self.as_ground_mut() {
            return Some(inner_doms);
        }
        None
    }

    /// If this is a record domain, clone and return its entries.
    pub fn as_record(&self) -> Option<Vec<RecordEntry>> {
        if let Some(GroundDomain::Record(record_entries)) = self.as_ground() {
            return Some(record_entries.iter().cloned().map(|r| r.into()).collect());
        }
        if let Some(UnresolvedDomain::Record(record_entries)) = self.as_unresolved() {
            return Some(record_entries.clone());
        }
        None
    }

    /// If this is a [GroundDomain::Record], get a mutable reference to its entries
    pub fn as_record_ground(&self) -> Option<&Vec<RecordEntryGround>> {
        if let Some(GroundDomain::Record(entries)) = self.as_ground() {
            return Some(entries);
        }
        None
    }

    /// If this is a record domain, get a mutable reference to its list of entries.
    /// The domain always becomes [UnresolvedDomain::Record] after this operation.
    pub fn as_record_mut(&mut self) -> Option<&mut Vec<RecordEntry>> {
        if let Some(GroundDomain::Record(entries_gds)) = self.as_ground() {
            let entries: Vec<RecordEntry> = entries_gds.iter().cloned().map(|r| r.into()).collect();
            *self = Domain::Unresolved(Moo::new(UnresolvedDomain::Record(entries)));
        }

        if let Some(UnresolvedDomain::Record(entries_gds)) = self.as_unresolved_mut() {
            return Some(entries_gds);
        }
        None
    }

    /// If this is a [GroundDomain::Record], get a mutable reference to its entries
    pub fn as_record_ground_mut(&mut self) -> Option<&mut Vec<RecordEntryGround>> {
        if let Some(GroundDomain::Record(entries)) = self.as_ground_mut() {
            return Some(entries);
        }
        None
    }

    /// Compute the intersection of two domains
    pub fn union(&self, other: &Domain) -> Result<Domain, DomainOpError> {
        match (self, other) {
            (Domain::Ground(a), Domain::Ground(b)) => Ok(Domain::Ground(Moo::new(a.union(b)?))),
            (Domain::Unresolved(a), Domain::Unresolved(b)) => {
                Ok(Domain::Unresolved(Moo::new(a.union_unresolved(b)?)))
            }
            (Domain::Unresolved(u), Domain::Ground(g))
            | (Domain::Ground(g), Domain::Unresolved(u)) => {
                todo!("Union of unresolved domain {u} and ground domain {g} is not yet implemented")
            }
        }
    }

    /// Compute the intersection of two ground domains
    pub fn intersect(&self, other: &Domain) -> Result<Domain, DomainOpError> {
        match (self, other) {
            (Domain::Ground(a), Domain::Ground(b)) => {
                a.intersect(b).map(|res| Domain::Ground(Moo::new(res)))
            }
            _ => Err(DomainOpError::NotGround),
        }
    }

    /// If the domain is ground, return an iterator over its values
    pub fn values(&self) -> Result<impl Iterator<Item = Literal>, DomainOpError> {
        if let Some(gd) = self.as_ground() {
            return gd.values();
        }
        Err(DomainOpError::NotGround)
    }

    /// If the domain is ground, return its size bound
    pub fn length(&self) -> Result<u64, DomainOpError> {
        if let Some(gd) = self.as_ground() {
            return gd.length();
        }
        Err(DomainOpError::NotGround)
    }

    /// Construct a ground domain from a slice of values
    pub fn from_literal_vec(vals: &[Literal]) -> Result<DomainPtr, DomainOpError> {
        GroundDomain::from_literal_vec(vals).map(DomainPtr::from)
    }

    /// Returns true if `lit` is a valid value of this domain
    pub fn contains(&self, lit: &Literal) -> Result<bool, DomainOpError> {
        if let Some(gd) = self.as_ground() {
            return gd.contains(lit);
        }
        Err(DomainOpError::NotGround)
    }

    pub fn element_domain(&self) -> Option<DomainPtr> {
        match self {
            Domain::Ground(gd) => gd.element_domain().map(DomainPtr::from),
            Domain::Unresolved(ud) => ud.element_domain(),
        }
    }
}

impl Typeable for Domain {
    fn return_type(&self) -> ReturnType {
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

fn as_grounds(doms: &[DomainPtr]) -> Option<Vec<Moo<GroundDomain>>> {
    doms.iter()
        .map(|idx| match idx.as_ref() {
            Domain::Ground(idx_gd) => Some(idx_gd.clone()),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Name;
    use crate::{domain_int, range};

    #[test]
    fn test_negative_product() {
        let d1 = Domain::int(vec![Range::Bounded(-2, 1)]);
        let d2 = Domain::int(vec![Range::Bounded(-2, 1)]);
        let res = d1
            .as_ground()
            .unwrap()
            .apply_i32(|a, b| Some(a * b), d2.as_ground().unwrap())
            .unwrap();

        assert!(matches!(res, GroundDomain::Int(_)));
        if let GroundDomain::Int(ranges) = res {
            assert!(!ranges.contains(&Range::Bounded(-4, 4)));
        }
    }

    #[test]
    fn test_negative_div() {
        let d1 = GroundDomain::Int(vec![Range::Bounded(-2, 1)]);
        let d2 = GroundDomain::Int(vec![Range::Bounded(-2, 1)]);
        let res = d1
            .apply_i32(|a, b| if b != 0 { Some(a / b) } else { None }, &d2)
            .unwrap();

        assert!(matches!(res, GroundDomain::Int(_)));
        if let GroundDomain::Int(ranges) = res {
            assert!(!ranges.contains(&Range::Bounded(-4, 4)));
        }
    }

    #[test]
    fn test_length_basic() {
        assert_eq!(Domain::empty(ReturnType::Int).length(), Ok(0));
        assert_eq!(Domain::bool().length(), Ok(2));
        assert_eq!(domain_int!(1..3, 5, 7..9).length(), Ok(7));
        assert_eq!(
            domain_int!(1..2, 5..).length(),
            Err(DomainOpError::Unbounded)
        );
    }
    #[test]
    fn test_length_set_basic() {
        // {∅, {1}, {2}, {3}, {1,2}, {1,3}, {2,3}, {1,2,3}}
        let s = Domain::set(SetAttr::<IntVal>::default(), domain_int!(1..3));
        assert_eq!(s.length(), Ok(8));

        // {{1,2}, {1,3}, {2,3}}
        let s = Domain::set(SetAttr::new_size(2), domain_int!(1..3));
        assert_eq!(s.length(), Ok(3));

        // {{1}, {2}, {3}, {1,2}, {1,3}, {2,3}}
        let s = Domain::set(SetAttr::new_min_max_size(1, 2), domain_int!(1..3));
        assert_eq!(s.length(), Ok(6));

        // {{1}, {2}, {3}, {1,2}, {1,3}, {2,3}, {1,2,3}}
        let s = Domain::set(SetAttr::new_min_size(1), domain_int!(1..3));
        assert_eq!(s.length(), Ok(7));

        // {∅, {1}, {2}, {3}, {1,2}, {1,3}, {2,3}}
        let s = Domain::set(SetAttr::new_max_size(2), domain_int!(1..3));
        assert_eq!(s.length(), Ok(7));
    }

    #[test]
    fn test_length_set_nested() {
        // {
        // ∅,                                          -- all size 0
        // {∅}, {{1}}, {{2}}, {{1, 2}},                -- all size 1
        // {∅, {1}}, {∅, {2}}, {∅, {1, 2}},            -- all size 2
        // {{1}, {2}}, {{1}, {1, 2}}, {{2}, {1, 2}}
        // }
        let s2 = Domain::set(
            SetAttr::new_max_size(2),
            // {∅, {1}, {2}, {1,2}}
            Domain::set(SetAttr::<IntVal>::default(), domain_int!(1..2)),
        );
        assert_eq!(s2.length(), Ok(11));
    }

    #[test]
    fn test_length_set_unbounded_inner() {
        // leaf domain is unbounded
        let s2_bad = Domain::set(
            SetAttr::new_max_size(2),
            Domain::set(SetAttr::<IntVal>::default(), domain_int!(1..)),
        );
        assert_eq!(s2_bad.length(), Err(DomainOpError::Unbounded));
    }

    #[test]
    fn test_length_set_overflow() {
        let s = Domain::set(SetAttr::<IntVal>::default(), domain_int!(1..20));
        assert!(s.length().is_ok());

        // current way of calculating the formula overflows for anything larger than this
        let s = Domain::set(SetAttr::<IntVal>::default(), domain_int!(1..63));
        assert_eq!(s.length(), Err(DomainOpError::TooLarge));
    }

    #[test]
    fn test_length_tuple() {
        // 3 ways to pick first element, 2 ways to pick second element
        let t = Domain::tuple(vec![domain_int!(1..3), Domain::bool()]);
        assert_eq!(t.length(), Ok(6));
    }

    #[test]
    fn test_length_record() {
        // 3 ways to pick rec.a, 2 ways to pick rec.b
        let t = Domain::record(vec![
            RecordEntry {
                name: Name::user("a"),
                domain: domain_int!(1..3),
            },
            RecordEntry {
                name: Name::user("b"),
                domain: Domain::bool(),
            },
        ]);
        assert_eq!(t.length(), Ok(6));
    }

    #[test]
    fn test_length_matrix_basic() {
        // 3 booleans -> [T, T, T], [T, T, F], ..., [F, F, F]
        let m = Domain::matrix(Domain::bool(), vec![domain_int!(1..3)]);
        assert_eq!(m.length(), Ok(8));

        // 2 numbers, each 1..3 -> 3*3 options
        let m = Domain::matrix(domain_int!(1..3), vec![domain_int!(1..2)]);
        assert_eq!(m.length(), Ok(9));
    }

    #[test]
    fn test_length_matrix_2d() {
        // 2x3 matrix of booleans -> (2**2)**3 = 64 options
        let m = Domain::matrix(Domain::bool(), vec![domain_int!(1..2), domain_int!(1..3)]);
        assert_eq!(m.length(), Ok(64));
    }

    #[test]
    fn test_length_matrix_of_sets() {
        // 3 sets drawn from 1..2; 4**3 = 64 total options
        let m = Domain::matrix(
            Domain::set(SetAttr::<IntVal>::default(), domain_int!(1..2)),
            vec![domain_int!(1..3)],
        );
        assert_eq!(m.length(), Ok(64));
    }
}
