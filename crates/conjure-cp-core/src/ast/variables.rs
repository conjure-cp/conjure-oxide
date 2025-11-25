use std::fmt::Display;

use derivative::Derivative;
use serde::{Deserialize, Serialize};

use crate::{ast::domains::Domain, representation::Representation};

use super::{
    ReturnType,
    categories::{Category, CategoryOf},
    types::Typeable,
};

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

#[derive(Clone, Debug, Serialize, Deserialize, Derivative)]
#[derivative(Hash, PartialEq, Eq)]
pub struct DecisionVariable {
    pub domain: Domain,

    // use this through [`Declaration`] - in the future, this probably will be stored in
    // declaration / domain, not here.
    #[serde(skip)]
    #[derivative(Hash = "ignore", PartialEq = "ignore")]
    pub(super) representations: Vec<Vec<Box<dyn Representation>>>,

    /// Category - should be quantified or decision variable
    pub(super) category: Category,
}

impl DecisionVariable {
    pub fn new(domain: Domain, category: Category) -> DecisionVariable {
        assert!(
            category >= Category::Quantified,
            "category of a DecisionVariable should be quantified or decision"
        );
        DecisionVariable {
            domain,
            representations: vec![],
            category,
        }
    }
}

impl CategoryOf for DecisionVariable {
    fn category_of(&self) -> Category {
        self.category
    }
}

impl Typeable for DecisionVariable {
    fn return_type(&self) -> Option<ReturnType> {
        todo!()
    }
}

impl Display for DecisionVariable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.domain.fmt(f)
    }
}
