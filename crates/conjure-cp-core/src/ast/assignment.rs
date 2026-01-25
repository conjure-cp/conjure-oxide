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
    UnknownVariable(DeclarationPtr),
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
    unassigned: BTreeSet<DeclarationPtr>,
}

pub struct Assignment {
    pub symbol_table: Rc<RefCell<SymbolTable>>,
    pub data: BTreeMap<DeclarationPtr, Literal>,
}

impl AssignmentBuilder {
    pub(super) fn new(symbol_table: Rc<RefCell<SymbolTable>>) -> Self {
        let st = symbol_table.borrow().clone();
        let unassigned: BTreeSet<DeclarationPtr> = st
            .into_iter()
            .filter_map(|(_, v)| match v.category_of() {
                Category::Decision => Some(v),
                _ => None,
            })
            .collect();
        Self {
            symbol_table,
            unassigned,
            data: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, var: DeclarationPtr, value: Literal) -> Result<(), AssignmentError> {
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
        if self.symbol_table.borrow().lookup(&var.name()).is_none() {
            return Err(AssignmentError::UnknownVariable(var.clone()));
        }

        self.data.insert(var.clone(), value.clone());
        // TODO (repr): Propagate assignments of representation variables

        self.unassigned.remove(&var);
        Ok(())
    }

    pub fn build(self) -> Result<Assignment, AssignmentError> {
        if self.unassigned.is_empty() {
            return Ok(Assignment {
                symbol_table: self.symbol_table,
                data: self.data,
            });
        }
        let ua = self.unassigned.into_iter().collect_vec();
        Err(AssignmentError::IncompleteAssignment(ua))
    }
}
