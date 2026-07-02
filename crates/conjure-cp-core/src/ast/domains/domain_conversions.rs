use super::{Domain, DomainOpError, DomainPtr, GroundDomain, UnresolvedDomain};
use crate::ast::Moo;

impl From<GroundDomain> for Domain {
    fn from(gd: GroundDomain) -> Self {
        Domain::Ground(Moo::new(gd))
    }
}

impl From<UnresolvedDomain> for Domain {
    fn from(ud: UnresolvedDomain) -> Self {
        Domain::Unresolved(Moo::new(ud))
    }
}

impl TryFrom<Domain> for Moo<GroundDomain> {
    type Error = DomainOpError;

    fn try_from(value: Domain) -> Result<Self, Self::Error> {
        match value {
            Domain::Ground(gd) => Ok(gd),
            Domain::Unresolved(_) => Err(DomainOpError::NotGround),
        }
    }
}

impl TryFrom<Domain> for GroundDomain {
    type Error = DomainOpError;

    fn try_from(value: Domain) -> Result<Self, Self::Error> {
        Ok(Moo::unwrap_or_clone(value.try_into()?))
    }
}

impl From<Moo<GroundDomain>> for DomainPtr {
    fn from(value: Moo<GroundDomain>) -> Self {
        Moo::new(Domain::Ground(value))
    }
}

impl From<&Moo<GroundDomain>> for DomainPtr {
    fn from(value: &Moo<GroundDomain>) -> Self {
        Moo::new(Domain::Ground(value.clone()))
    }
}

impl TryFrom<DomainPtr> for Moo<GroundDomain> {
    type Error = DomainOpError;

    fn try_from(value: DomainPtr) -> Result<Self, Self::Error> {
        match value.as_ref() {
            Domain::Ground(gd) => Ok(gd.clone()),
            Domain::Unresolved(_) => Err(DomainOpError::NotGround),
        }
    }
}

impl TryFrom<DomainPtr> for GroundDomain {
    type Error = DomainOpError;

    fn try_from(value: DomainPtr) -> Result<Self, Self::Error> {
        Ok(Moo::unwrap_or_clone(value.try_into()?))
    }
}

impl From<Moo<UnresolvedDomain>> for DomainPtr {
    fn from(value: Moo<UnresolvedDomain>) -> Self {
        Moo::new(Domain::Unresolved(value))
    }
}

impl From<&Moo<UnresolvedDomain>> for DomainPtr {
    fn from(value: &Moo<UnresolvedDomain>) -> Self {
        Moo::new(Domain::Unresolved(value.clone()))
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
