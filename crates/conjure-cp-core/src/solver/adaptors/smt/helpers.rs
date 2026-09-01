use crate::ast::{AbstractLiteral, Atom, DeclarationKind, Domain, Literal, Moo, Name, Range};
use crate::bug;
use crate::solver::{SolverError, SolverResult};
use conjure_cp_core::ast::GroundDomain;
use std::ops::Deref;
use z3::{Sort, Symbol, ast::*};

use super::IntTheory;
use super::store::SymbolStore;

/// Use 32-bit 2's complement signed bit-vectors
pub const BV_SIZE: u32 = 32;

/// A function which encodes a restriction for a specific variable. Given an AST of the correct
/// sort, constructs a boolean assertion which will ensure the variable has the correct domain.
type RestrictFn = Box<dyn Fn(&Dynamic) -> Bool>;

/// Returns the Oxide domain as a Z3 sort, along with a function to restrict a variable of that sort
/// to the original domain's restrictions.
pub fn domain_to_sort(domain: &GroundDomain, ints: IntTheory) -> SolverResult<(Sort, RestrictFn)> {
    use IntTheory::{Bv, Lia};

    match (ints, domain) {
        // Booleans of course have the same domain in SMT, so no restriction required
        (_, GroundDomain::Bool) => Ok((Sort::bool(), Box::new(|_| Bool::from_bool(true)))),

        // Return a disjunction of the restrictions each range of the domain enforces
        // I.e. `x: int(1, 3..5)` -> `or([x = 1, x >= 3 /\ x <= 5])`
        (Lia, GroundDomain::Int(ranges)) => {
            let ranges = ranges.clone();
            let restrict_fn = move |ast: &Dynamic| {
                let int = ast.as_int().unwrap();
                let restrictions: Vec<_> = ranges
                    .iter()
                    .map(|range| int_range_to_int_restriction(&int, range))
                    .collect();
                Bool::or(restrictions.as_slice())
            };
            Ok((Sort::int(), Box::new(restrict_fn)))
        }
        (Bv, GroundDomain::Int(ranges)) => {
            let ranges = ranges.clone();
            let restrict_fn = move |ast: &Dynamic| {
                let bv = ast.as_bv().unwrap();
                let restrictions: Vec<_> = ranges
                    .iter()
                    .map(|range| int_range_to_bv_restriction(&bv, range))
                    .collect();
                Bool::or(restrictions.as_slice())
            };
            Ok((Sort::bitvector(BV_SIZE), Box::new(restrict_fn)))
        }

        (_, GroundDomain::Matrix(val_domain, idx_domains)) => {
            // We constrain the inner values of the domain recursively
            // I.e. every way to index the array must give a value in the correct domain

            let (range_sort, restrict_val) = match idx_domains.as_slice() {
                [_] => domain_to_sort(val_domain, ints),
                [_, tail @ ..] => {
                    // Treat as a matrix containing (n-1)-dimensional matrices
                    let inner_domain = GroundDomain::Matrix(val_domain.clone(), tail.to_vec());
                    domain_to_sort(&inner_domain, ints)
                }
                [] => Err(SolverError::ModelInvalid(
                    "empty matrix index domain".into(),
                )),
            }?;

            // No need to constrain the indices themselves, that's done through SafeIndex/InDomain
            //
            // An index is a position in the matrix, not a value the model ever reasons about, so it
            // always uses the integer theory whatever the elements are. Otherwise indexing a
            // bit-vector array would need every index expression converted to a machine word first.
            let idx_domain = &idx_domains[0];
            let (domain_sort, _) = domain_to_sort(idx_domain.as_ref(), IntTheory::Lia)?;

            // Use the lower dimension's restricting fn to restrict all indexes in this dimension
            let idx_asts = domain_to_ast_vec(IntTheory::Lia, idx_domain.as_ref())?;
            let restrict_fn = move |ast: &Dynamic| {
                let arr = ast.as_array().unwrap();
                let restrictions: Vec<_> = idx_asts
                    .iter()
                    .map(|idx_ast| (restrict_val)(&arr.select(idx_ast)))
                    .collect();
                Bool::and(restrictions.as_slice())
            };
            Ok((
                Sort::array(&domain_sort, &range_sort),
                Box::new(restrict_fn),
            ))
        }

        (_, GroundDomain::Set(attr, elem_domain)) => {
            let (val_sort, _) = domain_to_sort(elem_domain, ints)?;

            // Restrict the size of the set
            let member_asts = domain_to_ast_vec(ints, elem_domain)?;
            let attr_size = attr.size.clone();
            let restrict_fn = move |ast: &Dynamic| {
                let set = ast.as_set().unwrap();
                let is_member: Vec<_> = member_asts
                    .iter()
                    .map(|val| set.member(val).ite(&Int::from(1), &Int::from(0)))
                    .collect();
                let size = Int::add(&is_member);
                match attr_size {
                    Range::Single(n) => size.eq(Int::from(n)),
                    Range::UnboundedL(r) => size.le(Int::from(r)),
                    Range::UnboundedR(l) => size.ge(Int::from(l)),
                    Range::Bounded(l, r) => {
                        Bool::and(&[size.ge(Int::from(l)), size.le(Int::from(r))])
                    }
                    Range::Unbounded => Bool::from_bool(true),
                }
            };

            Ok((Sort::set(&val_sort), Box::new(restrict_fn)))
        }

        _ => Err(SolverError::ModelFeatureNotImplemented(format!(
            "sort for '{domain}' not implemented"
        ))),
    }
}

/// Returns a domain as a vector of Z3 AST literals.
pub fn domain_to_ast_vec(ints: IntTheory, domain: &GroundDomain) -> SolverResult<Vec<Dynamic>> {
    let lits = domain
        .values()
        .map_err(|err| SolverError::Runtime(err.to_string()))?;
    lits.map(|lit| literal_to_ast(ints, &lit)).collect()
}

/// Returns a boolean expression restricting the given integer variable to the given range.
pub fn int_range_to_int_restriction(var: &Int, range: &Range<i32>) -> Bool {
    match range {
        Range::Single(n) => var.eq(Int::from(*n)),
        Range::UnboundedL(r) => var.le(Int::from(*r)),
        Range::UnboundedR(l) => var.ge(Int::from(*l)),
        Range::Bounded(l, r) => Bool::and(&[var.ge(Int::from(*l)), var.le(Int::from(*r))]),
        _ => bug!("int ranges should not be unbounded"),
    }
}

/// Returns a boolean expression restricting the given bitvector variable to the given integer range.
pub fn int_range_to_bv_restriction(var: &BV, range: &Range<i32>) -> Bool {
    match range {
        Range::Single(n) => var.eq(BV::from_i64(*n as i64, BV_SIZE)),
        Range::UnboundedL(r) => var.bvsle(BV::from_i64(*r as i64, BV_SIZE)),
        Range::UnboundedR(l) => var.bvsge(BV::from_i64(*l as i64, BV_SIZE)),
        Range::Bounded(l, r) => Bool::and(&[
            var.bvsge(BV::from_i64(*l as i64, BV_SIZE)),
            var.bvsle(BV::from_i64(*r as i64, BV_SIZE)),
        ]),
        _ => bug!("int ranges should not be unbounded"),
    }
}

pub fn name_to_symbol(name: &Name) -> SolverResult<Symbol> {
    match name {
        Name::User(ustr) => Ok(Symbol::String((*ustr).into())),
        Name::Machine(num) => Ok(Symbol::Int(*num as u32)),
        Name::Represented(parts) => {
            let (name, rule_str, suffix) = parts.as_ref();
            Ok(Symbol::String(format!("{name}#{rule_str}_{suffix}")))
        }
        _ => Err(SolverError::ModelFeatureNotImplemented(format!(
            "variable '{name}' name is unsupported"
        ))),
    }
}

/// Converts an atom (literal or reference) into an AST node.
pub fn atom_to_ast(store: &SymbolStore, atom: &Atom) -> SolverResult<Dynamic> {
    match atom {
        Atom::Reference(reference) => {
            if let Some((_, ast, _)) = store.get(&reference.name()) {
                return Ok(ast.clone());
            }

            if let Some(lit) = reference.resolve_constant() {
                return literal_to_ast(IntTheory::default(), &lit);
            }

            if let Some(inner_atom) = reference.resolve_atomic() {
                return atom_to_ast(store, &inner_atom);
            }

            let decl_kind = reference.ptr().kind();
            match decl_kind.deref() {
                DeclarationKind::ValueLetting(expr, _) => {
                    Err(SolverError::ModelFeatureNotImplemented(format!(
                        "value letting '{}' did not resolve to an atomic expression: {expr}",
                        reference.name()
                    )))
                }
                _ => Err(SolverError::ModelInvalid(format!(
                    "variable '{}' does not exist",
                    reference.name()
                ))),
            }
        }
        Atom::Literal(lit) => literal_to_ast(IntTheory::default(), lit),
        _ => Err(SolverError::ModelFeatureNotImplemented(format!(
            "atom sort not implemented: {atom}"
        ))),
    }
}

/// Converts a CO literal (expression containing no variables) into an AST node.
///
/// A constant has no representation of its own to read a theory off, so it is built in whichever
/// theory the caller is working in; operations mixing theories coerce their operands afterwards.
pub fn literal_to_ast(ints: IntTheory, lit: &Literal) -> SolverResult<Dynamic> {
    match lit {
        Literal::Bool(b) => Ok(Bool::from_bool(*b).into()),
        Literal::Int(n) => Ok(match ints {
            IntTheory::Lia => Int::from(*n).into(),
            IntTheory::Bv => BV::from_i64(*n as i64, BV_SIZE).into(),
        }),
        // A constant matrix becomes a Z3 array, so that indexing one by a variable works. The
        // packed matrix layout decodes through exactly this: a table of the element domain's
        // values, indexed by a digit computed from the packed integer.
        Literal::AbstractLiteral(AbstractLiteral::Matrix(elements, index_domain)) => {
            let values: Vec<Dynamic> = elements
                .iter()
                .map(|element| literal_to_ast(ints, element))
                .collect::<SolverResult<_>>()?;
            let Some(first) = values.first() else {
                return Err(SolverError::ModelFeatureNotImplemented(
                    "cannot build an array from an empty matrix literal".to_owned(),
                ));
            };

            // A plain list is written with an implied, unbounded index domain that cannot be
            // enumerated; its positions are simply one per element.
            let indices = domain_to_ast_vec(IntTheory::Lia, index_domain)
                .ok()
                .filter(|indices| indices.len() == values.len())
                .unwrap_or_else(|| {
                    (1..=values.len())
                        .map(|position| Int::from(position as i64).into())
                        .collect()
                });
            let index_sort = indices
                .first()
                .map(|index| index.get_sort())
                .unwrap_or_else(Sort::int);

            let mut array = Array::const_array(&index_sort, first);
            for (index, value) in indices.iter().zip(values.iter()) {
                array = array.store(index, value);
            }
            Ok(array.into())
        }
        _ => Err(SolverError::ModelFeatureNotImplemented(format!(
            "literal type not implemented: {lit}"
        ))),
    }
}
