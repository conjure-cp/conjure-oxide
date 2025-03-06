use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::ast::domains::{Domain, Range};

use super::{types::Typeable, ReturnType};

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

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct DecisionVariable {
    pub domain: Domain,
}

impl DecisionVariable {
    pub fn new(domain: Domain) -> DecisionVariable {
        DecisionVariable { domain }
    }
}

impl Typeable for DecisionVariable {
    fn return_type(&self) -> Option<ReturnType> {
        todo!()
    }
}

impl Display for DecisionVariable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.domain {
            Domain::BoolDomain => write!(f, "bool"),
            Domain::IntDomain(ranges) => {
                let mut first = true;
                for r in ranges {
                    if first {
                        first = false;
                    } else {
                        write!(f, " or ")?;
                    }
                    match r {
                        Range::Single(i) => write!(f, "{}", i)?,
                        Range::Bounded(i, j) => write!(f, "{}..{}", i, j)?,
                    }
                }
                Ok(())
            }
            Domain::DomainReference(name) => write!(f, "{}", name),
            Domain::DomainSet(attr, domain) => {
                write!(f, "{}", domain)?;
                Ok(())
            }
        }
    }
}
