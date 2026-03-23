use super::stored::ReprRuleStored;
use crate::ast::{
    DeclarationPtr, DomainPtr, Expression, Literal, Metadata, Moo, Name, Reference, SymbolTable,
};
use parking_lot::MappedRwLockReadGuard;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;

pub type ReprError = String;

pub trait ReprDomainLevel {
    type Assignment: ReprAssignment;
    type DeclLevel: ReprDeclLevel<DomainLevel = Self, Assignment = Self::Assignment>;
    const RULE: &'static dyn ReprRuleStored;

    /// Initialise this representation at the domain level.
    /// Returns `Err` if it is not applicable to the given domain.
    fn init(dom: DomainPtr) -> Result<Self, ReprError>
    where
        Self: Sized;

    /// Construct a concrete instance of this representation
    /// Returns:
    /// - The declaration-level representation
    /// - Representation variables to add to the symbol table
    /// - List of structural constraints
    fn instantiate(self, decl: DeclarationPtr) -> (Self::DeclLevel, SymbolTable, Vec<Expression>);

    /// Given an instance of this representation for some domain, and a value in that domain,
    /// construct the corresponding assignment of representation variables.
    fn down(&self, value: Literal) -> Result<Self::Assignment, ReprError>;
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

    /// Given an instance of this representation for some variable, and a value of the variable,
    /// construct the corresponding assignment of representation variables.
    fn down(&self, value: Literal) -> Result<Self::Assignment, ReprError> {
        self.clone().to_domain_level().down(value)
    }

    /// Look up the values of representation variables
    /// (TODO: This method impl should be auto-generated!)
    fn lookup_via(&self, lu: &LookupFn<'_>) -> Result<Self::Assignment, ReprError>;

    fn lookup(
        &self,
        raw_assignment: &HashMap<Name, Literal>,
    ) -> Result<Self::Assignment, ReprError> {
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

pub type ReprInitResult = Result<(SymbolTable, Vec<Expression>), ReprError>;

pub trait ReprRule: Send + Sync {
    const NAME: &'static str;
    const STORED: &'static dyn ReprRuleStored;
    type Assignment: ReprAssignment;
    type DeclLevel: ReprDeclLevel<Assignment = Self::Assignment>;
    type DomainLevel: ReprDomainLevel<DeclLevel = Self::DeclLevel>;

    fn get_for(decl: &DeclarationPtr) -> Option<MappedRwLockReadGuard<'_, Self::DeclLevel>> {
        decl.get_repr::<Self>()
    }

    fn init_for(decl: &mut DeclarationPtr) -> ReprInitResult {
        let decl2 = decl.clone();

        if decl.reprs().get::<Self>().is_some() {
            return Err(format!(
                "This representation already exists for {}",
                decl.name()
            ));
        }
        let dom = decl
            .domain()
            .ok_or(format!("Variable {} must have a domain", decl.name()))?;

        let dom_level = Self::DomainLevel::init(dom)?;
        let (state, symbols, mut constraints) = dom_level.instantiate(decl2.clone());

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
}
