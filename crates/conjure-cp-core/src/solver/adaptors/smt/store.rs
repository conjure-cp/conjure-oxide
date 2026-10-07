use std::collections::{BTreeMap, HashMap};

use itertools::Itertools;
use z3::{Context, Model, Solvable, SortKind, Symbol, Translate, ast::*};

use crate::ast::{
    AbstractLiteral, Domain, GroundDomain, Literal, Moo, Name, Range, ReturnType, Typeable,
};
use crate::solver::{SolverError, SolverResult};

use super::IntTheory;
use super::helpers::*;

/// Maps CO variable names to their CO domains, Z3 symbolic constants, and Z3 symbols.
#[derive(Clone, Debug)]
pub struct SymbolStore {
    map: BTreeMap<Name, (Moo<GroundDomain>, Dynamic, Symbol)>,
}

impl SymbolStore {
    pub fn new() -> Self {
        SymbolStore {
            map: BTreeMap::new(),
        }
    }

    pub fn insert(
        &mut self,
        name: Name,
        val: (Moo<GroundDomain>, Dynamic, Symbol),
    ) -> Option<(Moo<GroundDomain>, Dynamic, Symbol)> {
        self.map.insert(name, val)
    }

    pub fn get(&self, name: &Name) -> Option<&(Moo<GroundDomain>, Dynamic, Symbol)> {
        self.map.get(name)
    }
}

/// The integer theory an already-created variable is in, read off its Z3 sort.
fn int_theory_of_ast(ast: &Dynamic) -> IntTheory {
    match ast.get_sort().array_domain().map(|sort| sort.kind()) {
        Some(SortKind::Bv) => IntTheory::Bv,
        Some(_) => IntTheory::Lia,
        None => match ast.sort_kind() {
            SortKind::Bv => IntTheory::Bv,
            _ => IntTheory::Lia,
        },
    }
}

/// Builds a disequality over exactly the values admitted by an Essence domain.
///
/// SMT arrays and sets are total over their SMT sorts, so comparing either one directly also
/// observes values outside the finite Essence index/element domains. Z3 can otherwise keep
/// producing models that differ only there, even though they reconstruct to the same Essence
/// solution. Recurse through compound domains so those invisible differences are ignored at every
/// nesting level.
fn domain_disequality(domain: &GroundDomain, ast: &Dynamic, other: &Dynamic) -> Bool {
    match domain {
        GroundDomain::Matrix(value_domain, index_domains) => {
            let neqs: Vec<_> = index_domains
                .iter()
                .map(|domain| domain_to_ast_vec(IntTheory::Lia, domain).unwrap())
                .multi_cartesian_product()
                .map(|indices| {
                    indices
                        .iter()
                        .fold((ast.clone(), other.clone()), |(left, right), index| {
                            (
                                left.as_array().unwrap().select(index),
                                right.as_array().unwrap().select(index),
                            )
                        })
                })
                .map(|(left, right)| domain_disequality(value_domain.as_ref(), &left, &right))
                .collect();
            Bool::or(&neqs)
        }
        GroundDomain::Set(_, element_domain) => {
            let set = ast.as_set().unwrap();
            let other_set = other.as_set().unwrap();
            let neqs: Vec<_> = domain_to_ast_vec(int_theory_of_ast(ast), element_domain)
                .unwrap()
                .iter()
                .map(|element| set.member(element).ne(other_set.member(element)))
                .collect();
            Bool::or(&neqs)
        }
        _ => ast.ne(other),
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
            let (ast, lit) = interpret(model, (domain, ast), model_completion).unwrap();
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
                domain_disequality(domain, ast, other)
            })
            .collect();
        Bool::or(bools.as_slice())
    }
}

unsafe impl Translate for SymbolStore {
    fn translate(&self, ctx: &Context) -> Self {
        let mut new_map = BTreeMap::new();
        for (name, (domain, ast, sym)) in self.map.iter() {
            let new_ast = ast.translate(ctx);
            let new_sym = sym.clone(); // Symbols are not translated
            new_map.insert(name.clone(), (domain.clone(), new_ast, new_sym));
        }
        SymbolStore { map: new_map }
    }
}

/// Maps CO variable names to their literal values in both the Z3 model and Essence.
#[derive(Clone, Debug)]
pub struct LiteralStore {
    map: BTreeMap<Name, (Dynamic, Literal)>,
}

impl LiteralStore {
    fn new() -> Self {
        LiteralStore {
            map: BTreeMap::new(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::SetAttr;
    use crate::{domain_int_ground, range};
    use z3::{SatResult, Solver};

    #[test]
    fn matrix_of_sets_blocking_ignores_members_outside_the_element_domain() {
        let element_domain = domain_int_ground!(1..3);
        let set_domain = Moo::new(GroundDomain::Set(SetAttr::new_size(1), element_domain));
        let domain = GroundDomain::Matrix(set_domain, vec![domain_int_ground!(1..2)]);
        let (sort, _) = domain_to_sort(&domain, IntTheory::Lia).unwrap();
        let left = Dynamic::new_const("left", &sort);
        let right = Dynamic::new_const("right", &sort);
        let solver = Solver::new();

        // The two matrices agree on every observable set member.
        for index in 1..=2 {
            let index = Int::from(index);
            let left_set = left.as_array().unwrap().select(&index).as_set().unwrap();
            let right_set = right.as_array().unwrap().select(&index).as_set().unwrap();
            for element in 1..=3 {
                let element = Int::from(element);
                solver.assert(left_set.member(&element).eq(right_set.member(&element)));
            }
        }

        // They differ only at an integer outside the Essence element domain.
        let index = Int::from(1);
        let outside = Int::from(99);
        let left_set = left.as_array().unwrap().select(&index).as_set().unwrap();
        let right_set = right.as_array().unwrap().select(&index).as_set().unwrap();
        solver.assert(left_set.member(&outside).ne(right_set.member(&outside)));
        solver.assert(domain_disequality(&domain, &left, &right));

        assert_eq!(solver.check(), SatResult::Unsat);
    }
}

/// Interprets the given value within the given model and returns a CO literal.
/// The value can be any AST including a symbolic constant, as long as it is defined in the model.
///
/// This method makes any interpretations necessary to fully evaluate the literal. E.g. for arrays
/// it must enumerate over all elements in the index domain and evaluate the elements as literals.
fn interpret(
    model: &Model,
    value: (&Moo<GroundDomain>, &Dynamic),
    model_completion: bool,
) -> SolverResult<(Dynamic, Literal)> {
    use IntTheory::{Bv, Lia};

    let (domain, var_ast) = value;
    // Which theory this variable ended up in is a property of the variable itself, not of the
    // model, so read it back off the AST's sort rather than off a solver-wide setting.
    let theory = match var_ast.sort_kind() {
        SortKind::Bv => Bv,
        _ => Lia,
    };
    let lit_ast = model
        .eval(var_ast, model_completion)
        .ok_or(SolverError::Runtime(format!(
            "could not interpret variable: {var_ast}"
        )))?;

    let literal = match (theory, domain.as_ref()) {
        (_, GroundDomain::Bool) => {
            let bool_ast = lit_ast.as_bool().unwrap();
            let bool = bool_ast.as_bool().unwrap();
            Ok(Literal::Bool(bool))
        }
        (Lia, GroundDomain::Int(_)) => {
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
        (Bv, GroundDomain::Int(_)) => {
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
        (_, GroundDomain::Matrix(val_domain, idx_domains)) => {
            let arr_ast = lit_ast.as_array().unwrap();

            let inner_domain = match idx_domains.as_slice() {
                [idx_domain] => val_domain.clone(),
                [idx_domain, tail @ ..] => {
                    Moo::new(GroundDomain::Matrix(val_domain.clone(), tail.to_vec()))
                }
                [] => return Err(SolverError::Runtime("empty matrix index domain".into())),
            };

            // Indices are positions, always in the integer theory; see `domain_to_sort`.
            let indices = domain_to_ast_vec(IntTheory::Lia, &idx_domains[0])?;
            let elements_res: Result<Vec<_>, _> = indices
                .iter()
                .map(|idx| model.eval(&arr_ast.select(idx), model_completion).unwrap())
                .map(|ast| interpret(model, (&inner_domain, &ast), model_completion))
                .map(|res| res.map(|(_, lit)| lit))
                .collect();
            let elements = elements_res?;

            Ok(Literal::AbstractLiteral(AbstractLiteral::Matrix(
                elements,
                domain.clone(),
            )))
        }
        (_, GroundDomain::Set(_, val_domain)) => {
            let set_ast = lit_ast.as_set().unwrap();

            // Collect every member of the domain that is in the set
            let dom_iter = val_domain
                .values()
                .map_err(|_| SolverError::Runtime("could not construct domain iterator".into()))?;
            let members: Result<Vec<Literal>, SolverError> = dom_iter
                .map(|lit| {
                    // We want to bubble errors up but also only collect set members
                    let lit_ast = literal_to_ast(theory, &lit)?;
                    let is_member = model
                        .eval(&set_ast.member(&lit_ast), model_completion)
                        .unwrap()
                        .as_bool()
                        .unwrap();
                    Ok(is_member.then_some(lit))
                })
                .filter_map(|res: Result<Option<Literal>, SolverError>| res.ok().flatten().map(Ok))
                .collect();
            let members = members?;

            Ok(Literal::AbstractLiteral(AbstractLiteral::Set(members)))
        }
        _ => Err(SolverError::RuntimeNotImplemented(format!(
            "conversion from AST to literal of type '{domain}' not implemented: {lit_ast}"
        ))),
    }?;
    Ok((lit_ast, literal))
}
