//! The Model Syntax tree for the Minion bindings.

use std::collections::HashMap;

pub type VarName = String;
pub type Tuple = (Constant, Constant);
pub type TwoVars = (Var, Var);

pub struct Model {
    /// A lookup table of all named variables.
    pub named_variables: SymbolTable,
    pub constraints: Vec<Constraint>,
}

impl Model {
    pub fn new() -> Model {
        Model {
            named_variables: SymbolTable::new(),
            constraints: Vec::new(),
        }
    }
}

impl Default for Model {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Constraint {
    Difference(TwoVars, Var),
    Div(TwoVars, Var),
    DivUndefZero(TwoVars, Var),
    Modulo(TwoVars, Var),
    ModuloUndefZero(TwoVars, Var),
    Pow(TwoVars, Var),
    Product(TwoVars, Var),
    WeightedSumGeq(Vec<Constant>, Vec<Var>, Var),
    WeightedSumLeq(Vec<Constant>, Vec<Var>, Var),
    CheckAssign(Box<Constraint>),
    CheckGsa(Box<Constraint>),
    ForwardChecking(Box<Constraint>),
    Reify(Box<Constraint>, Var),
    ReifyImply(Box<Constraint>, Var),
    ReifyImplyQuick(Box<Constraint>, Var),
    WatchedAnd(Vec<Constraint>),
    WatchedOr(Vec<Constraint>),
    GacAllDiff(Vec<Var>),
    AllDiff(Vec<Var>),
    AllDiffMatrix(Vec<Var>, Constant),
    WatchSumGeq(Vec<Var>, Constant),
    WatchSumLeq(Vec<Var>, Constant),
    OccurrenceGeq(Vec<Var>, Constant, Constant),
    OccurrenceLeq(Vec<Var>, Constant, Constant),
    Occurrence(Vec<Var>, Constant, Var),
    LitSumGeq(Vec<Var>, Vec<Constant>, Constant),
    Gcc(Vec<Var>, Vec<Constant>, Vec<Var>),
    GccWeak(Vec<Var>, Vec<Constant>, Vec<Var>),
    LexLeqRv(Vec<Var>, Vec<Var>),
    LexLeq(Vec<Var>, Vec<Var>),
    LexLess(Vec<Var>, Vec<Var>),
    LexLeqQuick(Vec<Var>, Vec<Var>),
    LexLessQuick(Vec<Var>, Vec<Var>),
    WatchVecNeq(Vec<Var>, Vec<Var>),
    WatchVecExistsLess(Vec<Var>, Vec<Var>),
    Hamming(Vec<Var>, Vec<Var>, Constant),
    NotHamming(Vec<Var>, Vec<Var>, Constant),
    FrameUpdate(Vec<Var>, Vec<Var>, Vec<Var>, Vec<Var>, Constant),
    //HaggisGac(Vec<Var>,Vec<
    //HaggisGacStable
    //ShortStr2
    //ShortcTupleStr2
    NegativeTable(Vec<Var>, Vec<Tuple>),
    Table(Vec<Var>, Vec<Tuple>),
    GacSchema(Vec<Var>, Vec<Tuple>),
    LightTable(Vec<Var>, Vec<Tuple>),
    Mddc(Vec<Var>, Vec<Tuple>),
    NegativeMddc(Vec<Var>, Vec<Tuple>),
    Str2Plus(Vec<Var>, Var),
    Max(Vec<Var>, Var),
    Min(Vec<Var>, Var),
    NvalueGeq(Vec<Var>, Var),
    NvalueLeq(Vec<Var>, Var),
    SumLeq(Vec<Var>, Var),
    SumGeq(Vec<Var>, Var),
    Element(Vec<Var>, Var, Var),
    ElementOne(Vec<Var>, Var, Var),
    ElementUndefZero(Vec<Var>, Var, Var),
    WatchElement(Vec<Var>, Var, Var),
    WatchElementOne(Vec<Var>, Var, Var),
    WatchElementOneUndefZero(Vec<Var>, Var, Var),
    WatchElementUndefZero(Vec<Var>, Var, Var),
    WLiteral(Var, Constant),
    WNotLiteral(Var, Constant),
    WInIntervalSet(Var, Vec<Constant>),
    WInRange(Var, Vec<Constant>),
    WInset(Var, Vec<Constant>),
    WNotInRange(Var, Vec<Constant>),
    WNotInset(Var, Vec<Constant>),
    Abs(Var, Var),
    DisEq(Var, Var),
    Eq(Var, Var),
    MinusEq(Var, Var),
    GacEq(Var, Var),
    WatchLess(Var, Var),
    WatchNeq(Var, Var),
    Ineq(Var, Var, Constant),
}

/// A variable can either be a named variable, or an anomynous "constant as a variable".
///
/// The latter is not stored in the symbol table, or counted in Minions internal list of all
/// variables, but is used to allow the use of a constant in the place of a variable in a
/// constraint.
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Var {
    NameRef(VarName),
    ConstantAsVar(i32),
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum Constant {
    Bool(bool),
    Integer(i32),
}

#[derive(Debug, Copy, Clone)]
pub enum VarDomain {
    Bound(i32, i32),
    Discrete(i32, i32),
    SparseBound(i32, i32),
    Bool,
}

pub struct SymbolTable {
    table: HashMap<VarName, VarDomain>,

    // for now doubles both as Minion's SearchOrder and print order
    var_order: Vec<VarName>,
}

impl SymbolTable {
    fn new() -> SymbolTable {
        SymbolTable {
            table: HashMap::new(),
            var_order: Vec::new(),
        }
    }

    /// Creates a new variable and adds it to the symbol table.
    /// If a variable already exists with the given name, an error is thrown.
    pub fn add_var(&mut self, name: VarName, vartype: VarDomain) -> Option<()> {
        if self.table.contains_key(&name) {
            return None;
        }

        self.table.insert(name.clone(), vartype);
        self.var_order.push(name);

        Some(())
    }

    pub fn get_vartype(&self, name: VarName) -> Option<VarDomain> {
        self.table.get(&name).cloned()
    }

    pub fn get_variable_order(&self) -> Vec<VarName> {
        self.var_order.clone()
    }

    pub fn contains(&self, name: VarName) -> bool {
        self.table.contains_key(&name)
    }
}
