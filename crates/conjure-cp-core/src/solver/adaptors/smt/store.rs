use std::collections::HashMap;

use z3::{Solvable, SortKind, Symbol, ast::*};

use crate::ast::{Literal, Name};

#[derive(Clone)]
pub struct Store {
    /// Initially, this maps CO variable names to Z3 symbols.
    /// When reading a solution from a solver-returned model, we create a new store
    /// which instead maps to their values.
    map: HashMap<Name, Dynamic>,
}

impl Store {
    pub fn new() -> Self {
        Store {
            map: HashMap::new(),
        }
    }

    /// Return this store as a mapping of CO names to literals
    pub fn literals_map(&self) -> HashMap<Name, Literal> {
        let mut literals = HashMap::new();
        for (name, ast) in self.map.iter() {
            let lit = dynamic_to_literal(ast.clone());
            literals.insert(name.clone(), lit);
        }
        literals
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
            let val = model
                .eval(ast, model_completion)
                .expect("constant could not be evaluated");
            new_store.map.insert(name.clone(), val);
        }
        Some(new_store)
    }

    fn generate_constraint(&self, model: &Self::ModelInstance) -> Bool {
        let bools: Vec<_> = self
            .map
            .iter()
            .map(|(name, ast)| {
                let other = model
                    .map
                    .get(name)
                    .expect("value stores must have equal key sets");
                ast.ne(other)
            })
            .collect();
        Bool::or(bools.as_slice())
    }
}

// TODO: return Result<Literal, SolverError>
fn dynamic_to_literal(ast: Dynamic) -> Literal {
    match &ast.sort_kind() {
        SortKind::Bool => Literal::Bool(
            ast.as_bool()
                .unwrap()
                .as_bool()
                .expect("Bool AST is not a literal value"),
        ),
        _ => unimplemented!(),
    }
}
