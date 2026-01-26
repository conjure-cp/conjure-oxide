use crate::ast::categories::CategoryOf;
use crate::ast::types::Typeable;
use crate::ast::{DeclarationPtr, Literal};
use crate::ast::{DomainPtr, Name, SymbolTable};
use conjure_cp_core::ast::categories::Category;
use itertools::Itertools;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AssignmentError {
    #[error("Variable {0} does not exist in this symbol table")]
    UnknownVariable(Name),
    #[error("{0} is not a decision variable")]
    NotVariable(DeclarationPtr),
    #[error("Cannot assign value {value}:{} to variable {}:{}", .value.return_type(), .variable.name(), .variable.return_type())]
    BadType {
        variable: DeclarationPtr,
        value: Literal,
    },
    #[error("Value {value} is not allowed by the domain of variable {}:{}", .variable.name(), .variable.domain().map(|d| d.to_string()).unwrap_or(String::from("no domain")))]
    NotInDomain {
        variable: DeclarationPtr,
        value: Literal,
    },
    #[error("Variables {} are not assigned", .0.iter().map(|d| d.name().to_string()).join(", "))]
    IncompleteAssignment(Vec<DeclarationPtr>),
}

pub struct AssignmentBuilder {
    symbol_table: Rc<RefCell<SymbolTable>>,
    data: BTreeMap<DeclarationPtr, Literal>,
}

pub struct Assignment {
    pub data: BTreeMap<DeclarationPtr, Literal>,
}

impl AssignmentBuilder {
    pub(super) fn new(symbol_table: Rc<RefCell<SymbolTable>>) -> Self {
        Self {
            symbol_table,
            data: BTreeMap::new(),
        }
    }

    fn get_unassigned(&self) -> BTreeSet<DeclarationPtr> {
        self.symbol_table
            .borrow()
            .clone()
            .into_iter()
            .filter_map(|(_, v)| match v.category_of() {
                Category::Decision => Some(v),
                _ => None,
            })
            .collect()
    }

    pub fn insert_mut(&mut self, name: Name, value: Literal) -> Result<(), AssignmentError> {
        let Some(var) = self.symbol_table.borrow().lookup(&name) else {
            return Err(AssignmentError::UnknownVariable(name));
        };

        let Some(dv) = var.as_var() else {
            return Err(AssignmentError::NotVariable(var));
        };
        if dv.return_type() != value.return_type() {
            return Err(AssignmentError::BadType {
                variable: var.clone(),
                value,
            });
        }
        if let Some(gd) = dv.domain.as_ground()
            && !gd.contains(&value)
        {
            return Err(AssignmentError::NotInDomain {
                variable: var.clone(),
                value,
            });
        }

        // TODO (repr): Propagate assignments of representation variables
        self.data.insert(var.clone(), value);

        Ok(())
    }

    pub fn insert(self, name: Name, value: Literal) -> Result<AssignmentBuilder, AssignmentError> {
        let mut ans = self;
        ans.insert_mut(name, value)?;
        Ok(ans)
    }

    pub fn build(self) -> Result<Assignment, AssignmentError> {
        let ua = self.get_unassigned().into_iter().collect_vec();
        if ua.is_empty() {
            return Ok(Assignment { data: self.data });
        }
        Err(AssignmentError::IncompleteAssignment(ua))
    }
}
