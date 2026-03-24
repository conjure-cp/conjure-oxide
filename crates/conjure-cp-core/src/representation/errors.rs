use super::stored::ReprRuleStored;
#[allow(unused_imports)]
use super::types::{ReprDeclLevel, ReprDomainLevel};
use crate::ast::{DeclarationPtr, DomainPtr, Literal};
use std::fmt::Debug;
use thiserror::Error;

/// Errors that can be thrown by [ReprDomainLevel::init]
#[derive(Debug, Error)]
pub enum ReprInitError {
    /// The given domain isn't supported by this representation
    #[error("domain `{}` is not supported by representation `{}`{}", .0, .1, if .2.is_empty() { String::from("") } else { format!(": {}", .2) })]
    UnsupportedDomain(DomainPtr, &'static str, String),
    /// Can't initialise representation for a different reason
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Errors that can be thrown by [ReprDomainLevel::instantiate]
#[derive(Debug, Error)]
pub enum ReprInstantiateError {
    /// An instance of this representation already exists for this variable
    #[error("representation `{}` is already initialised for `{}`", .1, .0.name())]
    AlreadyExists(DeclarationPtr, &'static str),
    /// The given variable had no domain
    #[error("declaration `{}` (`{}`) had no domain", .0.name(), .0)]
    NoDomain(DeclarationPtr),
    /// The given variable was of an unsupported kind
    #[error("declaration `{}` (`{}`) had an unsupported kind{}", .0.name(), .0, if .1.is_empty() { String::from("") } else { format!(": {}", .1) })]
    BadKind(DeclarationPtr, String),
    /// This variable's domain is different from the one this domain-level representation is for
    #[error("got `{}: {}`, but this representation is for domain `{}`", .0.name(), .0.domain().map(|d| d.to_string()).unwrap_or(String::from("None")), .1)]
    BadDomain(DeclarationPtr, DomainPtr),
    /// Can't instantiate representation for a different reason
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Either [ReprInitError] or [ReprInstantiateError]
#[derive(Debug, Error)]
pub enum ReprError {
    #[error(transparent)]
    Init(#[from] ReprInitError),
    #[error(transparent)]
    Instantiate(#[from] ReprInstantiateError),
}

/// Either [ReprInitError] | [ReprInstantiateError] | [ReprSelectError]
#[derive(Debug, Error)]
pub enum ReferenceReprError {
    #[error(transparent)]
    Init(#[from] ReprInitError),
    #[error(transparent)]
    Instantiate(#[from] ReprInstantiateError),
    #[error(transparent)]
    Select(#[from] ReprSelectError),
}

impl From<ReprError> for ReferenceReprError {
    fn from(e: ReprError) -> Self {
        match e {
            ReprError::Init(e) => Self::Init(e),
            ReprError::Instantiate(e) => Self::Instantiate(e),
        }
    }
}

#[derive(Debug, Error)]
pub enum ReprDownError {
    /// The given literal cannot be represented by this representation
    #[error("unexpected value `{}`{}", .0, if .1.is_empty() { String::from("") } else { format!(": {}", .1) })]
    BadValue(Literal, String),
    /// Can't go down for a different reason
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, Error)]
pub enum ReprUpError {
    /// Looking up a representation variable failed
    #[error("no value found for `{}`", .0.name())]
    NotFound(DeclarationPtr),
    /// Lookup succeeded but the value is not what we expected
    #[error("value `{} = {}` is not allowed by domain {}", .0.name(), .1, .2)]
    BadDomain(DeclarationPtr, Literal, DomainPtr),
    /// Can't go up for a different reason
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, Error)]
pub enum ReprSelectError {
    #[error("representation `{}` does not exist for variable `{}`", .1, .0.name())]
    DoesNotExist(DeclarationPtr, &'static str),
    #[error("this reference already has representation `{}`", .0.name())]
    AlreadySelected(&'static dyn ReprRuleStored),
}
