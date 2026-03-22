use std::fmt::Display;

use super::categories::{Category, CategoryOf};
use conjure_cp_core::ast::DomainPtr;
use conjure_cp_core::ast::domains::HasDomain;
use derivative::Derivative;
use serde::{Deserialize, Serialize};
use uniplate::Uniplate;

/// Represents a decision variable within a computational model.
///
/// A `DecisionVariable` has a domain that defines the set of values it can take. The domain could be:
/// - A boolean domain, meaning the variable can only be `true` or `false`.
/// - An integer domain, meaning the variable can only take specific integer values or a range of integers.
///
/// # Fields
/// - `domain`:
///   - Type: `Domain`
///   - Represents the set of possible values that this decision variable can assume. The domain can be a range of integers
///     (IntDomain) or a boolean domain (BoolDomain).
///
/// # Example
///
/// use crate::ast::domains::{DecisionVariable, Domain, Range};
///
/// let bool_var = DecisionVariable::new(Domain::BoolDomain);
/// let int_var = DecisionVariable::new(Domain::IntDomain(vec![Range::Bounded(1, 10)]));
///
/// println!("Boolean Variable: {}", bool_var);
/// println!("Integer Variable: {}", int_var);

#[derive(Clone, Debug, Serialize, Deserialize, Derivative, Uniplate)]
#[derivative(Hash, PartialEq, Eq)]
#[biplate(to=DomainPtr)]
pub struct DecisionVariable {
    pub domain: DomainPtr,
}

impl DecisionVariable {
    pub fn new(domain: DomainPtr) -> DecisionVariable {
        DecisionVariable { domain }
    }
}

impl CategoryOf for DecisionVariable {
    fn category_of(&self) -> Category {
        Category::Decision
    }
}

impl HasDomain for DecisionVariable {
    fn domain_of(&self) -> DomainPtr {
        self.domain.clone()
    }
}

impl Display for DecisionVariable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.domain.fmt(f)
    }
}
