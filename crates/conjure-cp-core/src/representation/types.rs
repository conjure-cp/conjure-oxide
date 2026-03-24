use super::errors::{ReprDownError, ReprError, ReprInitError, ReprInstantiateError, ReprUpError};
use super::stored::ReprRuleStored;
use crate::ast::{
    DeclarationPtr, DomainPtr, Expression, Literal, Metadata, Moo, Name, Reference, SymbolTable,
};
use crate::bug;
use parking_lot::MappedRwLockReadGuard;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fmt::Debug;

pub type ReprInstantiateResult<D> = Result<(D, SymbolTable, Vec<Expression>), ReprInstantiateError>;

pub trait ReprDomainLevel {
    type Assignment: ReprAssignment;
    type DeclLevel: ReprDeclLevel<DomainLevel = Self, Assignment = Self::Assignment>;
    const RULE: &'static dyn ReprRuleStored;

    /// Initialise this representation at the domain level.
    /// Returns `Err` if it is not applicable to the given domain.
    fn init(dom: DomainPtr) -> Result<Self, ReprInitError>
    where
        Self: Sized;

    /// Construct a concrete instance of this representation
    /// Returns:
    /// - The declaration-level representation
    /// - Representation variables to add to the symbol table
    /// - List of structural constraints
    fn instantiate(self, decl: DeclarationPtr) -> ReprInstantiateResult<Self::DeclLevel>;

    /// Given an instance of this representation for some domain, and a value in that domain,
    /// construct the corresponding assignment of representation variables.
    fn down(&self, value: Literal) -> Result<Self::Assignment, ReprDownError>;
}

pub type LookupFn<'a> = Box<dyn Fn(&DeclarationPtr) -> Option<Literal> + 'a>;

pub trait ReprDeclLevel:
    Sized + Clone + Send + Sync + Debug + Serialize + for<'de> Deserialize<'de> + 'static
{
    type Assignment: ReprAssignment;
    type DomainLevel: ReprDomainLevel<DeclLevel = Self, Assignment = Self::Assignment>;
    const RULE: &'static dyn ReprRuleStored;

    /// Convert an instance of this representation back to domain level
    fn to_domain_level(self) -> Self::DomainLevel;

    /// Look up the values of representation variables
    fn lookup_via(&self, lu: &LookupFn<'_>) -> Result<Self::Assignment, ReprUpError>;

    /// Get the list of representation variables, in an arbitrary order
    fn repr_vars(&self) -> VecDeque<DeclarationPtr>;

    /// Given an instance of this representation for some variable, and a value of the variable,
    /// construct the corresponding assignment of representation variables.
    fn down(&self, value: Literal) -> Result<Self::Assignment, ReprDownError> {
        self.clone().to_domain_level().down(value)
    }

    fn lookup(
        &self,
        raw_assignment: &HashMap<Name, Literal>,
    ) -> Result<Self::Assignment, ReprUpError> {
        let lu: LookupFn<'_> =
            Box::new(|decl: &DeclarationPtr| raw_assignment.get(&decl.name()).cloned());
        self.lookup_via(&lu)
    }
}

pub trait ReprAssignment {
    /// Given an assignment of representation variables, construct the corresponding
    /// value of the represented variable
    fn up(self) -> Literal;
}

pub type ReprResult = Result<(SymbolTable, Vec<Expression>), ReprError>;
pub type ReprGetOrInitResult<'a, D, E> =
    Result<(MappedRwLockReadGuard<'a, D>, SymbolTable, Vec<Expression>), E>;

pub trait ReprRule: Send + Sync {
    const NAME: &'static str;
    const STORED: &'static dyn ReprRuleStored;
    type Assignment: ReprAssignment;
    type DeclLevel: ReprDeclLevel<Assignment = Self::Assignment>;
    type DomainLevel: ReprDomainLevel<DeclLevel = Self::DeclLevel>;

    fn get_or_init_for(
        decl: &'_ mut DeclarationPtr,
    ) -> ReprGetOrInitResult<'_, Self::DeclLevel, ReprError> {
        let (symbols, constraints) = Self::init_for_if_not_exists(decl)?;
        let state = decl.get_repr::<Self>().unwrap_or_else(|| {
            bug!(
                "just initialised representation `{}` for `{}`, but it was not stored",
                Self::NAME,
                decl
            )
        });
        Ok((state, symbols, constraints))
    }

    fn get_for(decl: &DeclarationPtr) -> Option<MappedRwLockReadGuard<'_, Self::DeclLevel>> {
        decl.get_repr::<Self>()
    }

    fn init_for(decl: &mut DeclarationPtr) -> ReprResult {
        let dom = decl
            .domain()
            .ok_or(ReprInstantiateError::NoDomain(decl.clone()))?;

        let dom_level = Self::DomainLevel::init(dom)?;
        let (state, symbols, mut constraints) = dom_level.instantiate(decl.clone())?;

        // save a copy `decl` so we can acquire a lock on the original
        let decl2 = decl.clone();
        for (_, decl) in decl.reprs().iter() {
            let us = Reference {
                ptr: decl2.clone(),
                repr: Some(Self::STORED),
            };
            let them = Reference {
                ptr: decl2.clone(),
                repr: Some(decl.rule()),
            };
            let eq = Expression::Eq(Metadata::new(), Moo::new(us.into()), Moo::new(them.into()));
            constraints.push(eq);
        }

        // we acquire a write lock here so nothing else beyond this point should touch `decl`
        decl.reprs_mut().put::<Self>(state);
        Ok((symbols, constraints))
    }

    fn init_for_if_not_exists(decl: &mut DeclarationPtr) -> ReprResult {
        if decl.reprs().has::<Self>() {
            return Ok((SymbolTable::default(), Vec::new()));
        }
        Self::init_for(decl)
    }
}
