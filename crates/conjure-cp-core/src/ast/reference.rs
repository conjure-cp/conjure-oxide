use super::{
    Atom, DeclarationKind, DeclarationPtr, DomainPtr, Expression, GroundDomain, Literal, Metadata,
    Moo, Name,
    categories::{Category, CategoryOf},
    domains::HasDomain,
    serde::{AsId, HasId},
};
use crate::bug;
use crate::representation::types::ReprGetOrInitResult;
use crate::representation::{
    ReferenceReprError, ReprError, ReprRule, ReprRulePtr, ReprSelectError, ReprStateStored,
};
use derivative::Derivative;
use parking_lot::MappedRwLockReadGuard;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::fmt::{Display, Formatter};
use uniplate::Uniplate;

/// A reference to a declaration (variable, parameter, etc.)
///
/// This is a thin wrapper around [`DeclarationPtr`] with two main purposes:
/// 1. Encapsulate the serde pragmas (e.g., serializing as IDs rather than full objects)
/// 2. Enable type-directed traversals of references via uniplate
#[serde_as]
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Uniplate, Derivative,
)]
#[derivative(Hash)]
#[uniplate()]
#[biplate(to=DeclarationPtr)]
#[biplate(to=Name)]
pub struct Reference {
    #[serde_as(as = "AsId")]
    pub ptr: DeclarationPtr,
    pub repr: Option<ReprRulePtr>,
}

impl Reference {
    pub fn new(ptr: DeclarationPtr) -> Self {
        Reference { ptr, repr: None }
    }

    pub fn ptr(&self) -> &DeclarationPtr {
        &self.ptr
    }

    pub fn into_ptr(self) -> DeclarationPtr {
        self.ptr
    }

    pub fn name(&self) -> MappedRwLockReadGuard<'_, Name> {
        self.ptr.name()
    }

    pub fn id(&self) -> crate::ast::serde::ObjId {
        self.ptr.id()
    }

    pub fn domain(&self) -> Option<DomainPtr> {
        self.ptr.domain()
    }

    pub fn resolve_domain(&mut self) -> Option<Moo<GroundDomain>> {
        self.ptr.resolve_domain()
    }

    /// Select the given representation for this reference, if it is currently unrepresented
    /// and the representation exists for the underlying variable.
    ///
    /// # Errors
    /// - [ReprSelectError::AlreadySelected] if a different representation is already selected for this reference
    /// - [ReprSelectError::DoesNotExist] if the representation does not exist for this variable
    ///
    /// # Returns
    /// State of the initialised representation
    pub fn select_repr<R: ReprRule + ?Sized>(
        &mut self,
    ) -> Result<MappedRwLockReadGuard<'_, R::DeclLevel>, ReprSelectError> {
        let _ = self.select_repr_via(R::STORED);
        Ok(self.repr_state_as_unchecked::<R>())
    }

    /// Same as [Reference::select_repr], but type-erased
    pub fn select_repr_via(
        &mut self,
        rule: ReprRulePtr,
    ) -> Result<MappedRwLockReadGuard<'_, dyn ReprStateStored>, ReprSelectError> {
        if let Some(repr) = self.repr
            && repr != rule
        {
            return Err(ReprSelectError::AlreadySelected(repr));
        }
        if !self.ptr.reprs().has_repr(rule) {
            return Err(ReprSelectError::DoesNotExist(self.ptr.clone(), rule.name()));
        }
        self.repr = Some(rule);
        Ok(self.repr_state_unchecked())
    }

    /// Same as [Reference::select_or_init_repr], but type-erased
    pub fn select_or_init_repr_via(
        &mut self,
        rule: ReprRulePtr,
    ) -> ReprGetOrInitResult<'_, dyn ReprStateStored, ReferenceReprError> {
        if let Some(repr) = self.repr
            && repr != rule
        {
            return Err(ReprSelectError::AlreadySelected(repr).into());
        }
        self.update_or_init_repr_via(rule).map_err(Into::into)
    }

    /// Select the given representation for this reference, initialising it if necessary.
    /// Will fail if a different representation is already selected.
    ///
    /// # Errors
    /// - [ReprSelectError] if a different representation is already selected for this reference
    /// - [ReprInitError] | [ReprInstantiateError] if the representation could not be initialised
    ///
    /// # Returns
    ///
    /// `(state, symbols, constraints)`
    /// where:
    /// - `state` is an instance of the given representation
    /// - `symbols` are new variables created by the representation
    /// - `constraints` are new top-level constraints created by the representation
    pub fn select_or_init_repr<R: ReprRule + ?Sized>(
        &mut self,
    ) -> ReprGetOrInitResult<'_, R::DeclLevel, ReferenceReprError> {
        let (_, symbols, constraints) = self.select_or_init_repr_via(R::STORED)?;
        let state = self.repr_state_as_unchecked::<R>();
        Ok((state, symbols, constraints))
    }

    /// Select the given representation for this reference, initialising it if necessary.
    /// Will overwrite the existing selection.
    ///
    /// # Errors
    /// - [ReprInitError] | [ReprInstantiateError] if the representation could not be initialised
    ///
    /// # Returns
    ///
    /// `(state, symbols, constraints)`
    /// where:
    /// - `state` is an instance of the given representation
    /// - `symbols` are new variables created by the representation
    /// - `constraints` are new top-level constraints created by the representation
    pub fn update_or_init_repr<R: ReprRule + ?Sized>(
        &mut self,
    ) -> ReprGetOrInitResult<'_, R::DeclLevel, ReprError> {
        let (_, symbols, constraints) = self.update_or_init_repr_via(R::STORED)?;
        let state = self.repr_state_as_unchecked::<R>();
        Ok((state, symbols, constraints))
    }

    /// Same as [Reference::update_or_init_repr], but type-erased
    pub fn update_or_init_repr_via(
        &mut self,
        rule: ReprRulePtr,
    ) -> ReprGetOrInitResult<'_, dyn ReprStateStored, ReprError> {
        let (symbols, constraints) = rule.init_for_if_not_exists(&mut self.ptr)?;
        self.repr = Some(rule);
        let state = self.repr_state_unchecked();
        Ok((state, symbols, constraints))
    }

    /// If this reference has a representation selected, return `(rule, state)`
    /// where
    /// - `rule` is a pointer to the representation rule
    /// - `state` is an instance of that representation
    pub fn get_repr(
        &self,
    ) -> Option<(ReprRulePtr, MappedRwLockReadGuard<'_, dyn ReprStateStored>)> {
        let rule = self.repr?;
        Some((rule, self.repr_state_unchecked()))
    }

    /// If this reference has a representation selected, return its state, otherwise crash
    fn repr_state_unchecked(&self) -> MappedRwLockReadGuard<'_, dyn ReprStateStored> {
        let rule = self
            .repr
            .unwrap_or_else(|| bug!("`{}` had no representation", self.name()));
        self.ptr
            .maybe_map(|d| d.representations().get_by_rule(rule))
            .unwrap_or_else(|| {
                bug!(
                    "Representation '{}' was selected for '{}' but its state was not stored!",
                    rule.name(),
                    self.name()
                )
            })
    }

    /// If this reference has this specific representation selected, return its state
    /// as a concrete type, otherwise crash
    fn repr_state_as_unchecked<R: ReprRule + ?Sized>(
        &self,
    ) -> MappedRwLockReadGuard<'_, R::DeclLevel> {
        R::get_for(&self.ptr).unwrap_or_else(|| {
            bug!(
                "`{}` did not have the expected representation `{}`",
                self.name(),
                R::NAME
            )
        })
    }

    /// Returns the expression behind a value-letting reference, if this is one.
    pub fn resolve_expression(&self) -> Option<Expression> {
        if let Some(expr) = self.ptr().as_value_letting() {
            return Some(expr.clone());
        }

        let generator = {
            let kind = self.ptr.kind();
            if let DeclarationKind::Quantified(inner) = &*kind {
                inner.generator().cloned()
            } else {
                None
            }
        };

        if let Some(generator) = generator
            && let Some(expr) = generator.as_value_letting()
        {
            return Some(expr.clone());
        }

        None
    }

    /// Evaluates this reference to a literal if it resolves to a constant.
    pub fn resolve_constant(&self) -> Option<Literal> {
        self.resolve_expression()
            .and_then(|expr| super::eval::eval_constant(&expr))
    }

    /// Resolves this reference to an atomic expression, if possible.
    pub fn resolve_atomic(&self) -> Option<Atom> {
        self.resolve_expression().and_then(|expr| match expr {
            Expression::Atomic(_, atom) => Some(atom),
            _ => None,
        })
    }
}

impl From<Reference> for Expression {
    fn from(value: Reference) -> Self {
        Expression::Atomic(Metadata::new(), value.into())
    }
}

impl From<Reference> for Moo<Expression> {
    fn from(value: Reference) -> Self {
        Moo::new(value.into())
    }
}

impl From<DeclarationPtr> for Reference {
    fn from(ptr: DeclarationPtr) -> Self {
        Reference::new(ptr)
    }
}

impl CategoryOf for Reference {
    fn category_of(&self) -> Category {
        self.ptr.category_of()
    }
}

impl HasDomain for Reference {
    fn domain_of(&self) -> DomainPtr {
        self.ptr.domain().unwrap_or_else(|| {
            bug!(
                "reference ({name}) should have a domain",
                name = self.ptr.name()
            )
        })
    }
}

impl Display for Reference {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.ptr.name().fmt(f)
    }
}
