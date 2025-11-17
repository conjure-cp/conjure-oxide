use std::collections::HashMap;

use itertools::Itertools;
use z3::{Model, Solvable, SortKind, Symbol, ast::*};

use crate::ast::{AbstractLiteral, Domain, Literal, Name, Range};
use crate::solver::{SolverError, SolverResult};

use super::helpers::*;
use super::{IntTheory, TheoryConfig};

/// Maps CO variable names to their CO domains, Z3 symbolic constants, and Z3 symbols.
#[derive(Clone, Debug)]
pub struct SymbolStore {
    map: HashMap<Name, (Domain, Dynamic, Symbol)>,
    theories: TheoryConfig,
}

impl SymbolStore {
    pub fn new(theories: TheoryConfig) -> Self {
        SymbolStore {
            map: HashMap::new(),
            theories,
        }
    }

    pub fn insert(
        &mut self,
        name: Name,
        val: (Domain, Dynamic, Symbol),
    ) -> Option<(Domain, Dynamic, Symbol)> {
        self.map.insert(name, val)
    }

    pub fn get(&self, name: &Name) -> Option<&(Domain, Dynamic, Symbol)> {
        self.map.get(name)
    }
}

impl Solvable for SymbolStore {
    type ModelInstance = LiteralStore;

    fn read_from_model(
        &self,
        model: &Model,
        model_completion: bool,
    ) -> Option<Self::ModelInstance> {
        let mut new_store = LiteralStore::new();
        for (name, (domain, ast, sym)) in self.map.iter() {
            // Get the interpretation of each constant
            let (ast, lit) =
                interpret(model, (domain, ast), model_completion, &self.theories).unwrap();
            new_store.map.insert(name.clone(), (ast, lit));
        }
        Some(new_store)
    }

    fn generate_constraint(&self, model: &Self::ModelInstance) -> Bool {
        let bools: Vec<_> = self
            .map
            .iter()
            .map(|(name, (domain, ast, _))| {
                let (other, _) = model.map.get(name).unwrap();
                match domain {
                    Domain::Matrix(_, idx_domains) => {
                        // Rather than just setting `array != other_array`, we need to do it for every element
                        // Otherwise, Z3 can generate new arrays which are equal over the domain
                        // but different otherwise (and this loops infinitely)

                        let neqs: Vec<_> = idx_domains
                            .iter()
                            .map(|domain| domain_to_ast_vec(&self.theories, domain).unwrap())
                            .multi_cartesian_product()
                            .map(|idxs| {
                                idxs.iter()
                                    .fold((ast.clone(), other.clone()), |(a, b), idx| {
                                        (
                                            a.as_array().unwrap().select(idx),
                                            b.as_array().unwrap().select(idx),
                                        )
                                    })
                            })
                            .map(|(a, b)| a.ne(b))
                            .collect();
                        Bool::or(&neqs)
                    }

                    // Any other variables are just directly compared
                    _ => ast.ne(other),
                }
            })
            .collect();
        Bool::or(bools.as_slice())
    }
}

/// Maps CO variable names to their literal values in both the Z3 model and Essence.
#[derive(Clone, Debug)]
pub struct LiteralStore {
    map: HashMap<Name, (Dynamic, Literal)>,
}

impl LiteralStore {
    fn new() -> Self {
        LiteralStore {
            map: HashMap::new(),
        }
    }

    /// Return this store as a mapping of CO names to literals
    pub fn as_literals_map(&self) -> SolverResult<HashMap<Name, Literal>> {
        let iter = self
            .map
            .iter()
            .map(|(name, (_, lit))| (name.clone(), lit.clone()));
        Ok(HashMap::from_iter(iter))
    }
}

impl Solvable for LiteralStore {
    // We never actually use this type as a Solvable, but the trait requires ModelInstance
    // to be a Solvable as well.

    type ModelInstance = Self;

    fn read_from_model(&self, _: &Model, _: bool) -> Option<Self::ModelInstance> {
        unimplemented!()
    }

    fn generate_constraint(&self, _: &Self::ModelInstance) -> Bool {
        unimplemented!()
    }
}

/// Interprets the given value within the given model and returns a CO literal.
/// The value can be any AST including a symbolic constant, as long as it is defined in the model.
///
/// This method makes any interpretations necessary to fully evaluate the literal. E.g. for arrays
/// it must enumerate over all elements in the index domain and evaluate the elements as literals.
fn interpret(
    model: &Model,
    value: (&Domain, &Dynamic),
    model_completion: bool,
    theories: &TheoryConfig,
) -> SolverResult<(Dynamic, Literal)> {
    use IntTheory::{Bv, Lia};

    let (domain, var_ast) = value;
    let lit_ast = model
        .eval(var_ast, model_completion)
        .ok_or(SolverError::Runtime(format!(
            "could not interpret variable: {var_ast}"
        )))?;

    let literal = match (theories.ints, lit_ast.sort_kind()) {
        (_, SortKind::Bool) => {
            let bool_ast = lit_ast.as_bool().unwrap();
            let bool = bool_ast.as_bool().unwrap();
            Ok(Literal::Bool(bool))
        }
        (Lia, SortKind::Int) => {
            let int_ast = lit_ast.as_int().unwrap();
            let int = int_ast
                .as_i64()
                .ok_or(SolverError::Runtime(format!(
                    "could not cast to i64: {lit_ast}"
                )))?
                .try_into()
                .map_err(|err| {
                    SolverError::Runtime(format!("value {lit_ast} out of range: {err}"))
                })?;
            Ok(Literal::Int(int))
        }
        (Bv, SortKind::BV) => {
            // BVs do not sign-extend when returning u64s (if they are < 64 bits)
            // To correctly retrieve negative numbers, we downsize to a u32 and then bit-wise
            // interpret it as an i32, rather than casting.
            // See https://github.com/prove-rs/z3.rs/issues/458
            let bv_ast = lit_ast.as_bv().unwrap();
            let unsigned_64: u64 = bv_ast.as_u64().ok_or(SolverError::Runtime(format!(
                "could not retrieve u64: {lit_ast}"
            )))?;
            let unsigned_32: u32 = unsigned_64.try_into().map_err(|err| {
                SolverError::Runtime(format!("value {lit_ast} out of range: {err}"))
            })?;
            let signed = i32::from_ne_bytes(unsigned_32.to_ne_bytes());
            Ok(Literal::Int(signed))
        }
        (_, SortKind::Array) => {
            let arr_ast = lit_ast.as_array().unwrap();
            let Domain::Matrix(val_domain, idx_domains) = domain else {
                return Err(SolverError::Runtime(format!(
                    "non-matrix variable interpreted as array: {domain}"
                )));
            };

            let inner_domain = match idx_domains.as_slice() {
                [idx_domain] => *val_domain.clone(),
                [idx_domain, tail @ ..] => Domain::Matrix(val_domain.clone(), tail.to_vec()),
                [] => return Err(SolverError::Runtime("empty matrix index domain".into())),
            };

            let indices = domain_to_ast_vec(theories, &idx_domains[0])?;
            let elements_res: Result<Vec<_>, _> = indices
                .iter()
                .map(|idx| model.eval(&arr_ast.select(idx), model_completion).unwrap())
                .map(|ast| interpret(model, (&inner_domain, &ast), model_completion, theories))
                .map(|res| res.map(|(_, lit)| lit))
                .collect();
            let elements = elements_res?;

            Ok(Literal::AbstractLiteral(AbstractLiteral::Matrix(
                elements,
                Box::new(domain.clone()),
            )))
        }
        _ => Err(SolverError::RuntimeNotImplemented(format!(
            "conversion from AST to literal not implemented: {lit_ast}"
        ))),
    }?;
    Ok((lit_ast, literal))
}
