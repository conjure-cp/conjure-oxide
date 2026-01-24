use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use crate::ast::{DomainPtr, SymbolTable};
use crate::ast::types::Typeable;
use thiserror::Error;
use crate::ast::{DeclarationPtr, Literal};

#[derive(Debug, Error)]
pub enum AssignmentError {
    #[error("Variable {0} does not exist in this symbol table")]
    UnknownVariable(DeclarationPtr),
    #[error("{0} is not a decision variable")]
    NotVariable(DeclarationPtr),
    #[error("Cannot assign value {value}:{} to variable {}:{}", .value.return_type(), .variable.name(), .variable.return_type())]
    BadType { variable: DeclarationPtr, value: Literal },
    #[error("Value {value} is not allowed by the domain of variable {}:{}", .variable.name(), .variable.domain().map(|d| d.to_string()).unwrap_or(String::from("no domain")))]
    NotInDomain { variable: DeclarationPtr, value: Literal }
}

pub(super) struct AssignmentBuilder {
    symbol_table: Rc<RefCell<SymbolTable>>,
    data: BTreeMap<DeclarationPtr, Literal>
}

pub struct Assignment {
    pub symbol_table: Rc<RefCell<SymbolTable>>,
    pub data: BTreeMap<DeclarationPtr, Literal>
}

impl AssignmentBuilder {
    pub fn insert(&mut self, variable: DeclarationPtr, value: Literal) -> Result<(), AssignmentError> {
        let Some(var) = variable.as_var() else {
            return Err(AssignmentError::NotVariable(variable));
        };

        if self.symbol_table.borrow().lookup(&variable.name()).is_none() {
            return Err(AssignmentError::UnknownVariable(variable));
        }
        if variable.return_type() != value.return_type() {
            return  Err(AssignmentError::BadType { variable, value });
        }
        if variable.domain().is_some_and()
    }
}