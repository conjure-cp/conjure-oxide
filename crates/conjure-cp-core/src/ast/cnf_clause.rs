use crate::ast::Expression;
use polyquine::Quine;
use serde::Deserialize;
use serde::Serialize;
use std::fmt;
use uniplate::Uniplate;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Uniplate, Quine)]
pub struct CnfClause {
    // This represents a cnf clause in its simplest form, it should only contain literals
    literals: Vec<Expression>,
}

impl CnfClause {
    pub fn new(literals: Vec<Expression>) -> Self {
        CnfClause { literals }
    }

    // Expose an iterator for the vector
    pub fn iter(&self) -> impl Iterator<Item = &Expression> {
        self.literals.iter()
    }

    pub fn literals(&self) -> &Vec<Expression> {
        &self.literals
    }
}

impl fmt::Display for CnfClause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Print the contents of the valid_variants vector
        write!(f, "(")?;
        for (i, lit) in self.literals.iter().enumerate() {
            if i > 0 {
                write!(f, " \\/ ")?; // Add a comma between elements
            }
            match lit {
                Expression::Not(_, var) => write!(f, "¬{}", var.as_ref())?,
                Expression::Atomic(_, _) => write!(f, "{lit}")?,
                _ => panic!("This expression type should not appear in a CnfClause"),
            }
        }
        write!(f, ")") // Close the vector representation
    }
}
