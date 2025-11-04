use std::collections::HashMap;

use z3::{Solvable, SortKind, Symbol, ast::*};

use crate::ast::{Literal, Name};
use crate::solver::SolverError;

/// Initially, this maps CO variable names to Z3 symbols.
/// When reading a solution from a solver-returned model, we create a new store
/// which instead maps to their values.
#[derive(Clone)]
pub struct Store {
    map: HashMap<Name, Dynamic>,
}

impl Store {
    pub fn new() -> Self {
        Store {
            map: HashMap::new(),
        }
    }

    /// Return this store as a mapping of CO names to literals
    pub fn literals_map(&self) -> Result<HashMap<Name, Literal>, SolverError> {
        let mut literals = HashMap::new();
        for (name, ast) in self.map.iter() {
            let lit = dynamic_to_literal(ast.clone())?;
            literals.insert(name.clone(), lit);
        }
        Ok(literals)
    }

    pub fn insert(&mut self, name: Name, ast: Dynamic) -> Option<Dynamic> {
        self.map.insert(name, ast)
    }

    pub fn get(&self, name: &Name) -> Option<&Dynamic> {
        self.map.get(name)
    }
}

impl Solvable for Store {
    type ModelInstance = Self;

    fn read_from_model(
        &self,
        model: &z3::Model,
        model_completion: bool,
    ) -> Option<Self::ModelInstance> {
        let mut new_store = Store::new();
        for (name, ast) in self.map.iter() {
            // Get the interpretation of each constant
            let val = model.eval(ast, model_completion).unwrap();
            new_store.map.insert(name.clone(), val);
        }
        Some(new_store)
    }

    fn generate_constraint(&self, model: &Self::ModelInstance) -> Bool {
        let bools: Vec<_> = self
            .map
            .iter()
            .map(|(name, ast)| {
                let other = model.map.get(name).unwrap();
                ast.ne(other)
            })
            .collect();
        Bool::or(bools.as_slice())
    }
}

/// Takes a constant Z3 value and returns the CO literal it maps to.
fn dynamic_to_literal(ast: Dynamic) -> Result<Literal, SolverError> {
    match &ast.sort_kind() {
        SortKind::Bool => Ok(Literal::Bool(ast.as_bool().unwrap().as_bool().ok_or(
            SolverError::Runtime("Bool AST is not a literal value".into()),
        )?)),
        SortKind::Int => Ok(Literal::Int(
            ast.as_int()
                .unwrap()
                .as_i64()
                .ok_or(SolverError::Runtime(format!(
                    "could not cast to i64: {ast}"
                )))?
                .try_into()
                .map_err(|err| SolverError::Runtime(format!("value {ast} out of range: {err}")))?,
        )),
        SortKind::BV => {
            /// BVs do not sign-extend when returning u64s (if they are < 64 bits)
            /// To correctly retrieve negative numbers, we cast to a u32 and then bit-wise interpret
            /// it as an i32, rather than casting.
            /// See https://github.com/prove-rs/z3.rs/issues/458
            let unsigned: u32 = ast
                .as_bv()
                .unwrap()
                .as_u64()
                .ok_or(SolverError::Runtime(format!(
                    "could not retrieve u64: {ast}"
                )))?
                .try_into()
                .map_err(|err| SolverError::Runtime(format!("value {ast} out of range: {err}")))?;
            Ok(Literal::Int(i32::from_ne_bytes(unsigned.to_ne_bytes())))
        }
        _ => Err(SolverError::RuntimeNotImplemented(format!(
            "conversion from AST to literal not implemented: {ast}"
        ))),
    }
}
