//! The Model Syntax tree for the Minion bindings.

use std::{collections::HashMap};

pub type VarName = String;

pub struct Model {
    /// A lookup table of all named variables.
    pub named_variables : SymbolTable,
    pub constraints: Vec<Constraint>
}


impl Model {
    pub fn new() -> Model{
        Model {
            named_variables: SymbolTable::new(),
            constraints: Vec::new()
        }
    }
}

pub enum Constraint {
    SumLeq(Vec<Var>,Var),
    SumGeq(Vec<Var>,Var),
    Ineq(Var,Var,Constant)
}


/// A variable can either be a named variable, or an anomynous "constant as a variable".
///
/// The latter is not stored in the symbol table, or counted in Minions internal list of all
/// variables, but is used to allow the use of a constant in the place of a variable in a
/// constraint.
pub enum Var{
    NameRef(VarName),
    ConstantAsVar(i32)
}

pub enum Constant {
    Bool(bool),
    Discrete(i32)
}

#[derive(Copy,Clone)]
pub enum VarType {
    Bounded(i32,i32),
    Bool(bool)
}

pub struct SymbolTable {
    table: HashMap<VarName,VarType>,

    // for now doubles both as Minion's SearchOrder and print order
    var_order: Vec<VarName>
}

impl SymbolTable {
    fn new() -> SymbolTable {
        SymbolTable {
            table: HashMap::new(),
            var_order: Vec::new()
        }
    }

    /// Creates a new variable and adds it to the symbol table.
    /// If a variable already exists with the given name, an error is thrown.
    pub fn add_var(&mut self,name: VarName, vartype: VarType) -> Option<()> {
        if self.table.contains_key(&name){
            return None;
        }

        self.table.insert(name.clone(),vartype);
        self.var_order.push(name);

        return Some(());
    }

    pub fn get_vartype(&self,name: VarName) -> Option<VarType> {
        match self.table.get(&name) {
            Some(m) => Some(*m),
            None => None
        }
    }

    pub fn get_variable_order(&self) -> Vec<VarName> {
        self.var_order.clone()
    }

    pub fn contains(&self, name: VarName) -> bool {
        self.table.contains_key(&name)
    }


}
