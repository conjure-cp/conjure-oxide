use crate::ast::Expression;
use crate::ast::domains::ground::GroundDomain;
use crate::ast::domains::range::Range;
use crate::ast::domains::set_attr::SetAttr;
use crate::ast::domains::unresolved::{IntVal, UnresolvedDomain};
use crate::ast::{
    DeclarationPtr, DomainOpError, Literal, MaybeTypeable, Moo, RecordEntry, RecordEntryGround,
    Reference, ReturnType, Typeable,
};
use itertools::Itertools;
use polyquine::Quine;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
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

impl<T: HasDomain> MaybeTypeable for T {
    fn maybe_return_type(&self) -> Option<ReturnType> {
        self.domain_of().maybe_return_type()
    }
}

impl Domain {
    /// Create a new boolean domain and return a pointer to it.
    /// Boolean domains are always ground (see [GroundDomain::Bool]).
    pub fn new_bool() -> DomainPtr {
        // TODO(perf): Since this is completely static, and we're using references, we may save
        // some minor memory allocations by initialising one static Moo::(...Bool)
        // and passing that around instead of creating new ones every time
        Moo::new(Domain::Ground(Moo::new(GroundDomain::Bool)))
    }

    /// Create a new empty domain of the given type and return a pointer to it.
    /// Empty domains are always ground (see [GroundDomain::Empty]).
    pub fn new_empty(ty: ReturnType) -> DomainPtr {
        Moo::new(Domain::Ground(Moo::new(GroundDomain::Empty(ty))))
    }

    /// Create a new int domain with the given ranges.
    /// If the ranges are all ground, the variant will be [GroundDomain::Int].
    /// Otherwise, it will be [UnresolvedDomain::Int].
    pub fn new_int<T>(ranges: Vec<T>) -> DomainPtr
    where
        T: Into<Range<IntVal>> + TryInto<Range<Int>> + Clone,
    {
        if let Ok(int_rngs) = ranges.iter().cloned().map(TryInto::try_into).try_collect() {
            return Moo::new(Domain::Ground(Moo::new(GroundDomain::Int(int_rngs))));
        }
        let unresolved_rngs: Vec<Range<IntVal>> = ranges.into_iter().map(Into::into).collect();
        Moo::new(Domain::Unresolved(Moo::new(UnresolvedDomain::Int(
            unresolved_rngs,
        ))))
    }

    /// Create a new ground integer domain with the given ranges
    pub fn new_int_ground(ranges: Vec<Range<Int>>) -> DomainPtr {
        Moo::new(Domain::Ground(Moo::new(GroundDomain::Int(ranges))))
    }

    /// Create a new set domain with the given element domain and attributes.
    /// If the element domain and the attributes are ground, the variant
    /// will be [GroundDomain::Set]. Otherwise, it will be [UnresolvedDomain::Set].
    pub fn new_set<T>(attr: T, inner_dom: DomainPtr) -> DomainPtr
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
    pub fn new_matrix(inner_dom: DomainPtr, idx_doms: Vec<DomainPtr>) -> DomainPtr {
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
    pub fn new_tuple(inner_doms: Vec<DomainPtr>) -> DomainPtr {
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
    pub fn new_record(entries: Vec<RecordEntry>) -> DomainPtr {
        if let Ok(entries_gds) = entries.iter().cloned().map(TryInto::try_into).try_collect() {
            return Moo::new(Domain::Ground(Moo::new(GroundDomain::Record(entries_gds))));
        }
        Moo::new(Domain::Unresolved(Moo::new(UnresolvedDomain::Record(
            entries,
        ))))
    }

    /// Create a new [UnresolvedDomain::Reference] domain from a domain letting
    pub fn new_ref(ptr: DeclarationPtr) -> Option<DomainPtr> {
        ptr.as_domain_letting()?;
        Some(Moo::new(Domain::Unresolved(Moo::new(
            UnresolvedDomain::Reference(Reference::new(ptr)),
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
        self.maybe_return_type() == Some(ReturnType::Bool)
    }

    /// True if this is a [GroundDomain::Int] or an [UnresolvedDomain::Int]
    pub fn is_int(&self) -> bool {
        self.maybe_return_type() == Some(ReturnType::Int)
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

    pub fn intersect(&self, other: &Domain) -> Result<Domain, DomainOpError> {
        match (self, other) {
            (Domain::Ground(a), Domain::Ground(b)) => {
                a.intersect(b).map(|res| Domain::Ground(Moo::new(res)))
            }
            _ => Err(DomainOpError::NotGround),
        }
    }

    pub fn values(&self) -> Result<impl Iterator<Item = Literal>, DomainOpError> {
        if let Some(gd) = self.as_ground() {
            return gd.values();
        }
        Err(DomainOpError::NotGround)
    }

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
}

impl MaybeTypeable for Domain {
    fn maybe_return_type(&self) -> Option<ReturnType> {
        match self {
            Domain::Ground(dom) => Some(dom.return_type()),
            Domain::Unresolved(dom) => dom.maybe_return_type(),
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
