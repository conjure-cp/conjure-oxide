use crate::guard;
use crate::representation::MatrixComponents;
use crate::utils::{eval_to_usize, to_aux_var};
use conjure_cp::ast::matrix::unflatten_matrix;
use conjure_cp::ast::{
    Atom, DeclarationKind, DomainPtr, Expression, GroundDomain, Metadata, Moo, Range, Reference,
    SymbolTable, eval_constant,
};
use conjure_cp::bug::UnwrapOrBug;
use conjure_cp::into_matrix_expr;
use conjure_cp::representation::ReprRule;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect as Reduction, register_rule,
    register_rule_set,
};
use conjure_cp::settings::SolverFamily;
use conjure_cp::solver::adaptors::smt::{MatrixTheory, TheoryConfig};
use conjure_cp::utils::View;
use conjure_cp::{domain_int, essence_expr, range};
use std::cell::Cell;
use std::collections::VecDeque;
use uniplate::{Biplate, Uniplate};

register_rule_set!("ReprMatrixComponents", ("Base"), |f: &SolverFamily| {
    if matches!(
        f,
        SolverFamily::Smt(TheoryConfig {
            matrices: MatrixTheory::Atomic,
            ..
        })
    ) {
        return true;
    }
    matches!(f, SolverFamily::Sat(_) | SolverFamily::Minion)
});

/// True when a local find/letting still needs `MatrixComponents` initialised.
fn decl_needs_matrix_components_init(decl: &conjure_cp::ast::DeclarationPtr) -> bool {
    matches!(
        &decl.kind() as &DeclarationKind,
        DeclarationKind::Find(..)
            | DeclarationKind::FindAuxiliary(..)
            | DeclarationKind::ValueLetting(..)
    ) && decl.reprs().is_empty()
        && decl
            .resolved_domain()
            .is_some_and(|gd| matches!(gd.as_ref(), GroundDomain::Matrix(..)))
}

/// True when a reference can select `MatrixComponents` but has not yet done so.
fn reference_needs_matrix_components_selection(re: &Reference) -> bool {
    re.repr.is_none() && re.ptr.reprs().has_repr(MatrixComponents::STORED)
}

/// True when a top-most `Reference` under the expression (via `Biplate`) still needs selection.
///
/// Prefer this over `universe_bi`: that expands each `Reference` into declaration domains and
/// dominated EFPA samples on the hot fail path. Top-most biplate children cover expression and
/// in-expression domain refs; nested declaration-domain refs are not required for MatrixComponents.
fn expression_needs_matrix_components_selection(expr: &Expression) -> bool {
    Biplate::<Reference>::children_bi(expr)
        .into_iter()
        .any(|re| reference_needs_matrix_components_selection(&re))
}

/// Select `MatrixComponents` on top-most biplate `Reference` children (shallow `descend_bi`).
fn select_matrix_components_on_biplate_refs(expr: &Expression, changed: &Cell<bool>) -> Expression {
    expr.descend_bi(&|mut re: Reference| {
        if reference_needs_matrix_components_selection(&re) {
            let _ = re.select_repr_via(&MatrixComponents);
            changed.set(true);
        }
        re
    })
}

/// Special-case repr selection for matrices as their only representation is MatrixComponents
#[register_rule("ReprMatrixComponents", 8500, [Root])]
fn select_repr_mta(expr: &Expression, symtab: &SymbolTable) -> ApplicationResult {
    let Expression::Root(..) = expr else {
        return Err(RuleNotApplicable);
    };

    // Hot fail path: Root is re-dirtied on almost every rewrite. Once every matrix declaration
    // already exposes MatrixComponents, do not walk Biplate::<Reference> over the whole model: that
    // scan is O(model size) and dominates large post-expansion trees (e.g. lee-distance).
    //
    // References that emerge later from opaque comprehension bodies are selected by
    // [`select_mta_after_comprehension_expansion`] on the materialised AC node instead.
    let needs_decl_init = symtab
        .iter_local()
        .any(|(_, decl)| decl_needs_matrix_components_init(decl));
    if !needs_decl_init {
        return Err(RuleNotApplicable);
    }

    // Initialise MatrixComponents for every matrix var in the symbol table
    let mut new_symtab = symtab.clone();
    let mut new_constraints = Vec::new();
    let changed = Cell::new(false);
    for (_, decl) in symtab.iter_local() {
        guard!(
            // this is a variable or constant
            matches!(&decl.kind() as &DeclarationKind, DeclarationKind::Find(..) | DeclarationKind::FindAuxiliary(..) | DeclarationKind::ValueLetting(..)) &&
            // ...which hasn't been represented yet
            decl.reprs().is_empty() &&
            // ...and its domain resolves to a matrix
            let mut new_decl = decl.clone() &&
            let Some(gd) = new_decl.resolved_domain() &&
            matches!(gd.as_ref(), GroundDomain::Matrix(..))
            else {
                continue;
            }
        );

        let Ok((symbols, new_top)) = MatrixComponents::init_for(&mut new_decl) else {
            continue;
        };
        new_symtab.update_insert(new_decl);
        new_symtab.extend(symbols);
        new_constraints.extend(new_top);
        changed.set(true);
    }

    // Select MatrixComponents on top-most biplate references already visible outside comprehensions.
    let new_expr = select_matrix_components_on_biplate_refs(expr, &changed);

    // Avoid infinite loop
    if !changed.get() {
        Err(RuleNotApplicable)
    } else {
        Ok(Reduction::new(new_expr, new_constraints, new_symtab))
    }
}

/// Select `MatrixComponents` on references that appear when comprehensions expand.
///
/// Comprehensions are Uniplate leaves, so [`select_repr_mta`] cannot see their bodies before
/// expansion. After a comprehension becomes an AC argument list, select once on that node so later
/// Root-level `select_repr_mta` attempts can stay O(declarations) rather than O(model).
#[register_rule("ReprMatrixComponents", 1990, [And, Or, Sum, Product])]
fn select_mta_after_comprehension_expansion(
    expr: &Expression,
    _: &SymbolTable,
) -> ApplicationResult {
    if !expression_needs_matrix_components_selection(expr) {
        return Err(RuleNotApplicable);
    }

    let changed = Cell::new(false);
    let new_expr = select_matrix_components_on_biplate_refs(expr, &changed);
    if !changed.get() {
        Err(RuleNotApplicable)
    } else {
        Ok(Reduction::pure(new_expr))
    }
}

/// Lowers a constant in-bounds [`Expression::UnsafeIndex`] of a `MatrixComponents` subject to the atom.
///
/// After comprehension expansion, indices are typically ground. The default pipeline is
/// `index_to_bubble` (`UnsafeIndex` → `SafeIndex`) then [`index_matrix_components`] (`SafeIndex` →
/// atom), which costs two worklist updates per indexing site. This rule is the fused fast path for
/// that case: same result as those two rules when every index is an in-bounds constant, so Bubble
/// would not wrap a condition. Out-of-bounds or non-constant indices stay with `index_to_bubble`.
///
/// The same lowering is also applied during comprehension expansion simplification so ground
/// indices never enter the rewriter worklist; this rule remains the oracle for any sites that
/// become constant only after expansion.
#[register_rule("ReprMatrixComponents", 6500, [UnsafeIndex])]
fn unsafe_const_index_matrix_components(
    expr: &Expression,
    _symbols: &SymbolTable,
) -> ApplicationResult {
    try_lower_const_unsafe_index_matrix_components(expr)
        .map(Reduction::pure)
        .ok_or(RuleNotApplicable)
}

/// Attempts the fused constant `UnsafeIndex` → `MatrixComponents` element lowering.
///
/// Returns [`Some`] only when every index is a constant inside its dimension domain and no nested
/// represented-matrix index remains below this node. Callers outside the rule engine (notably
/// comprehension expansion simplification) use this to avoid paying a worklist update per site.
pub(crate) fn try_lower_const_unsafe_index_matrix_components(
    expr: &Expression,
) -> Option<Expression> {
    let Expression::UnsafeIndex(_, subject, indices) = expr else {
        return None;
    };

    let Expression::Atomic(_, Atom::Reference(re)) = subject.as_ref() else {
        return None;
    };

    // Nested represented-matrix indices must be lowered first (same invariant as SafeIndex path).
    if expr
        .universe()
        .iter()
        .skip(1)
        .any(is_matrix_components_index)
    {
        return None;
    }

    let mta = re.ptr().get_repr::<MatrixComponents>()?;

    if indices.len() != mta.index_domains.len() {
        return None;
    }

    // Require every index to be a constant inside its dimension domain; otherwise Bubble owns it.
    let mut slices = Vec::with_capacity(indices.len());
    for (domain, index) in mta.index_domains.iter().zip(indices.iter()) {
        let lit = eval_constant(index)?;
        match domain.contains(&lit) {
            Ok(true) => slices.push(Range::Single(lit)),
            _ => return None,
        }
    }

    let view = mta.slice_lit(&slices).unwrap_or_bug();
    let mut elems = mta.view_as_exprs(&view);
    // All indices were concrete and in-bounds, so the view is a single scalar atom.
    assert_eq!(
        elems.len(),
        1,
        "constant in-bounds MatrixComponents index should yield one element"
    );
    Some(elems.swap_remove(0))
}

/// Using the matrix components representation rule, rewrite matrix indexing.
/// ```plain
/// find m: matrix indexed by [int(1..2), int(1..3), int(1..4)] of bool
/// find x: int(1..3)
/// such that
///
/// m[1, x, 2] = true
/// ~~>
/// [m_1_1_2, m_1_2_2, m_1_3_2][x] = true
/// ```
#[register_rule("ReprMatrixComponents", 5000, [SafeIndex])]
fn index_matrix_components(expr: &Expression, symbols: &SymbolTable) -> ApplicationResult {
    // Rewriting an outer index first can duplicate nested indices exponentially (e.g. m[m[i]]).
    // Defer it until every represented-matrix index below it has been rewritten.
    if expr
        .universe()
        .iter()
        .skip(1)
        .any(is_matrix_components_index)
    {
        return Err(RuleNotApplicable);
    }
    index_matrix_components_impl(expr, symbols)
}

/// True when `expr` is a (safe or unsafe) index into a declaration with `MatrixComponents` initialised.
fn is_matrix_components_index(expr: &Expression) -> bool {
    let subject = match expr {
        Expression::SafeIndex(_, subject, _) | Expression::UnsafeIndex(_, subject, _) => subject,
        _ => return false,
    };
    matches!(
        subject.as_ref(),
        Expression::Atomic(_, Atom::Reference(re)) if re.ptr().get_repr::<MatrixComponents>().is_some()
    )
}

pub(crate) fn try_index_matrix_components(
    expr: &Expression,
    symbols: &SymbolTable,
) -> ApplicationResult {
    index_matrix_components_impl(expr, symbols)
}

fn index_matrix_components_impl(expr: &Expression, symbols: &SymbolTable) -> ApplicationResult {
    guard!(
        // this is a safe indexing expression
        let Expression::SafeIndex(_, subject, indices) = expr &&
        let Expression::Atomic(_, Atom::Reference(re)) = &**subject &&
        // ...into a variable represented by MatrixComponents
        let Some(mta) = re.ptr().get_repr::<MatrixComponents>()
        else {
            return Err(RuleNotApplicable);
        }
    );
    let idx_doms = &mta.index_domains;

    // All indices that evaluate to a literal are resolved immediately;
    // The rest of the matrix is put into a flat slice which we index by the remaining indices
    let mut slices = Vec::new();
    let mut remaining_dims = Vec::new();
    for (i, idx_expr) in indices.iter().enumerate() {
        if let Some(idx_lit) = eval_constant(idx_expr) {
            slices.push(Range::Single(idx_lit));
        } else {
            slices.push(Range::Unbounded);
            remaining_dims.push(i);
        }
    }

    let view = mta.slice_lit(&slices).unwrap_or_bug();

    // Flat slice of remaining elements to index
    let mut lhs_elems: Vec<Expression> = mta.view_as_exprs(&view);

    // We've resolved all indices so the result is a scalar
    if remaining_dims.is_empty() {
        assert_eq!(lhs_elems.len(), 1);
        return Ok(Reduction::pure(lhs_elems.swap_remove(0)));
    }

    // Some indices were not resolved so output is an index into a list
    let new_lhs = into_matrix_expr!(lhs_elems);
    let mut new_rhs_exprs = VecDeque::new();
    let mut idx_auxvars = symbols.clone();
    let mut idx_auxvar_constraints = Vec::new();

    // Flatten the remaining indices;
    // iterate in reverse order and calculate offset as we go
    let mut off = 1;
    for i in (0..view.dims.len()).rev() {
        // which dimension this was in the original matrix
        let di = remaining_dims[i];
        // size of this dimension
        let dim_sz = view.dims[i];

        // indexing expression and domain for that dimension
        let mut idx_expr = indices[di].clone();
        let idx_dom = &idx_doms[di];
        let idx_dom_gd = idx_dom.as_ref();

        // if indexing expr is compound, extract it into an auxvar
        // for the stuff that comes below...
        if let Some(res) = to_aux_var(&idx_expr, &idx_auxvars) {
            idx_auxvar_constraints.push(res.top_level_expr());
            idx_auxvars = res.symbols();
            idx_expr = res.as_expr();
        }

        // remap "weird" indices to 1..dim_sz
        match idx_dom_gd {
            // for booleans and contiguous int domains, the mapping is simpler
            GroundDomain::Bool => {
                idx_expr = essence_expr!(&off * toInt(&idx_expr));
            }
            GroundDomain::Int(rngs) if Range::is_contiguous(rngs) => {
                let lo = Range::low_of(rngs).expect("unbounded index");
                idx_expr = essence_expr!(&off * (&idx_expr - &lo));
            }
            // for abstract domains, we'll have to build a big mapping table, which is expensive...
            _ => {
                // build a constraint mapping original indices integers
                let mapped_idx = Reference::new(
                    idx_auxvars.gen_find_auxiliary(&domain_int!(0..(dim_sz as i32 - 1))),
                );
                let mut eq_cases = Vec::new();
                for idx_val in 0..dim_sz {
                    let orig_idx_val = mta.index_flat_to_lit(di, idx_val).unwrap_or_bug();
                    let idx_val = idx_val as i32;
                    eq_cases.push(essence_expr!(
                        r"(&idx_expr = &orig_idx_val) /\ (&mapped_idx = &idx_val)"
                    ));
                }
                // to avoid over-constraining the original `idx_expr`, add a case for when it falls
                // out of matrix bounds; bubbling rules should have dealt with this previously anyway
                let default_case =
                    Expression::InDomain(Metadata::new(), Moo::new(idx_expr), idx_dom.into());
                eq_cases.push(essence_expr!(!&default_case));
                let eq_cases_disj =
                    Expression::Or(Metadata::new(), Moo::new(into_matrix_expr!(eq_cases)));
                idx_auxvar_constraints.push(eq_cases_disj);

                idx_expr = essence_expr!(&off * &mapped_idx);
            }
        }

        new_rhs_exprs.push_front(idx_expr);
        off *= dim_sz as i32;
    }

    // Index into flat matrix literal
    new_rhs_exprs.push_back(1.into()); // because indices start from 1
    let new_rhs = Expression::Sum(
        Metadata::new(),
        Moo::new(into_matrix_expr!(new_rhs_exprs.into())),
    );
    let new_expr = Expression::SafeIndex(Metadata::new(), Moo::new(new_lhs), vec![new_rhs]);

    Ok(Reduction::new(
        new_expr,
        idx_auxvar_constraints,
        idx_auxvars,
    ))
}

#[register_rule("ReprMatrixComponents", 5000, [SafeSlice])]
fn slice_matrix_components(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    guard!(
        // this is a safe slicing expression
        let Expression::SafeSlice(_, subject, dim_slices) = expr &&
        let Expression::Atomic(_, Atom::Reference(re)) = &**subject &&
        // ...into a variable represented by MatrixComponents
        let Some(mta) = re.ptr().get_repr::<MatrixComponents>()
        else {
            return Err(RuleNotApplicable);
        }
    );
    let idx_doms = &mta.index_domains;

    // All indices that evaluate to a literal are resolved immediately;
    // The rest of the matrix is put into a flat slice which we index by the remaining indices
    let mut slices = Vec::new();
    let mut new_index_domains: Vec<DomainPtr> = Vec::new();
    let mut new_indices = Vec::new();
    for (i, dim_slice) in dim_slices.iter().enumerate() {
        if let Some(idx_expr) = dim_slice
            && let Some(idx_lit) = eval_constant(idx_expr)
        {
            slices.push(Range::Single(idx_lit));
        } else {
            slices.push(Range::Unbounded);
            new_indices.push(dim_slice.clone());
            new_index_domains.push((&idx_doms[i]).into());
        }
        // TODO: The above handles indices or `..` slices but not `a..b`
        //       Add handling of `a..b` when range expressions are supported by AST / parser
    }

    let view = mta.slice_lit(&slices).unwrap_or_bug();

    // Flat slice of remaining elements to index
    let mut lhs_elems: Vec<Expression> = mta.view_as_exprs(&view);

    // We've resolved all indices so the result is a scalar
    if new_indices.is_empty() {
        assert_eq!(lhs_elems.len(), 1);
        return Ok(Reduction::pure(lhs_elems.swap_remove(0)));
    }

    // All remaining slices are `..`, so result is equivalent to just the LHS
    if new_indices.iter().all(|x| x.is_none()) {
        let new_lhs = into_matrix_expr!(lhs_elems);
        return Ok(Reduction::pure(new_lhs));
    }

    // Separate remaining dimensions into indexed (Some(expr)) and sliced (None / `..`).
    // Build a permutation that puts indexed dims first so they become the outer
    // dimensions of a matrix literal, addressable by SafeIndex.
    let (idx_positions, slice_positions): (Vec<_>, Vec<_>) = new_indices
        .iter()
        .enumerate()
        .partition::<Vec<_>, _>(|(_, idx)| idx.is_some());
    let perm: Vec<usize> = idx_positions
        .iter()
        .chain(slice_positions.iter())
        .map(|(i, _)| *i)
        .collect();

    // Permute the view so elements come out in the reorganised order,
    // then unflatten into a matrix literal with indexed dims as outer structure.
    let permuted_view = view.permute(&perm);
    let permuted_elems: Vec<Expression> = mta.view_as_exprs(&permuted_view);
    let permuted_index_domains: Vec<_> =
        perm.iter().map(|&i| new_index_domains[i].clone()).collect();
    let unflatten_strides = View::row_major_strides(&permuted_view.dims);
    let new_lhs = unflatten_matrix(&permuted_elems, &permuted_index_domains, &unflatten_strides);

    // Now index into it using the remaining expressions
    let index_exprs: Vec<Expression> = idx_positions
        .iter()
        .map(|(i, _)| new_indices[*i].clone().unwrap())
        .collect();

    let new_expr = Expression::SafeIndex(Metadata::new(), Moo::new(new_lhs), index_exprs);
    Ok(Reduction::pure(new_expr))
}

/// Flatten a represented matrix
/// ```plain
/// flatten(x)
/// ~>
/// [x_MatrixComponents_1, ..., x_MatrixComponents_N]
/// ```
#[register_rule("ReprMatrixComponents", 5000, [Flatten])]
fn matrix_flatten_to_atom(expr: &Expression, _symbols: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expression::Flatten(_, dims, subj) = expr            &&
        let Expression::Atomic(_, Atom::Reference(re)) = &**subj &&
        let Some(repr) = re.get_repr_as::<MatrixComponents>()
        else {
            return Err(RuleNotApplicable);
        }
    );

    let n = dims.as_ref().map(|x| eval_to_usize(x)).unwrap_or(0);

    let view = repr.flatten(n);
    let elems: Vec<Expression> = repr.view_as_exprs(&view);
    Ok(Reduction::pure(into_matrix_expr!(elems)))
}

/// Converts a reference to a 1d-matrix not contained within an indexing or slicing expression to its atoms.
///
/// Prefiltered to parents of atomic children: the rule only rewrites
/// `Atomic`/`Reference` children that already have a [`MatrixComponents`]
/// representation. Keeping it universal previously dominated failed attempts
/// on large post-expansion trees (solitaire_battleship).
#[register_rule("ReprMatrixComponents", 2000, [* / Atomic])]
fn matrix_ref_to_atom(expr: &Expression, _symbols: &SymbolTable) -> ApplicationResult {
    if let Expression::SafeSlice(..)
    | Expression::UnsafeSlice(..)
    | Expression::SafeIndex(..)
    | Expression::UnsafeIndex(..)
    | Expression::Flatten(..) = expr
    {
        return Err(RuleNotApplicable);
    };

    let mut changed = false;
    let flattened_children = expr
        .children()
        .into_iter()
        .map(|expr| {
            if let Expression::Atomic(_, Atom::Reference(re)) = &expr
                && let Some(mta) = re.ptr().get_repr::<MatrixComponents>()
            {
                changed = true;
                let elem_refs: Vec<Expression> =
                    mta.flat_elem_refs().map(Expression::from).collect();
                let index_domains: Vec<DomainPtr> =
                    mta.index_domains.iter().map(Into::into).collect();
                unflatten_matrix(&elem_refs, &index_domains, &mta.strides)
            } else {
                expr
            }
        })
        .collect();

    if !changed {
        return Err(RuleNotApplicable);
    }

    let new_expr = expr.with_children(flattened_children);
    Ok(Reduction::pure(new_expr))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{DeclarationPtr, Domain, Metadata, Name, Range, SymbolTable};
    use conjure_cp::into_matrix_expr;
    use conjure_cp::rule_engine::{ApplicationError, get_rule_by_name};

    /// Builds a represented matrix find and a large `and([...])` of unselected references to it.
    fn matrix_and_unselected_refs(n: usize) -> (SymbolTable, Expression, DeclarationPtr) {
        let mut symbols = SymbolTable::new();
        let domain = Domain::matrix(
            Domain::int(vec![Range::Bounded(1, 4)]),
            vec![Domain::int(vec![Range::Bounded(1, 3)])],
        );
        let decl = DeclarationPtr::new_find(Name::user("S"), domain);
        symbols
            .insert(decl.clone())
            .expect("matrix find should insert");

        let mut decl_for_init = decl.clone();
        let (extra, _tops) = MatrixComponents::init_for(&mut decl_for_init).unwrap();
        symbols.update_insert(decl_for_init.clone());
        symbols.extend(extra);

        let refs: Vec<Expression> = (0..n)
            .map(|_| {
                Expression::Atomic(
                    Metadata::new(),
                    Atom::Reference(Reference::new(decl.clone())),
                )
            })
            .collect();
        let and_expr = Expression::And(Metadata::new(), Moo::new(into_matrix_expr!(refs)));
        let root = Expression::Root(Metadata::new(), vec![and_expr]);
        (symbols, root, decl_for_init)
    }

    #[test]
    fn select_repr_mta_skips_expression_walk_when_decls_already_initialised() {
        let (symbols, root, decl) = matrix_and_unselected_refs(64);
        assert!(decl.reprs().has_repr(MatrixComponents::STORED));
        assert!(expression_needs_matrix_components_selection(&root));

        let rule = get_rule_by_name("select_repr_mta").expect("select_repr_mta registered");
        let err = rule.apply(&root, &symbols).unwrap_err();
        assert!(matches!(err, ApplicationError::RuleNotApplicable));
    }

    #[test]
    fn select_mta_after_comprehension_expansion_selects_emergent_refs() {
        let (symbols, root, _) = matrix_and_unselected_refs(8);
        let Expression::Root(_, children) = &root else {
            panic!("expected root");
        };
        let and_expr = &children[0];
        assert!(expression_needs_matrix_components_selection(and_expr));

        let rule = get_rule_by_name("select_mta_after_comprehension_expansion")
            .expect("select_mta_after_comprehension_expansion registered");
        let result = rule.apply(and_expr, &symbols).expect("selection applies");
        assert!(!expression_needs_matrix_components_selection(
            &result.new_expression
        ));
    }

    /// Builds an initialised 1d matrix find `m` indexed by `int(1..3)` of `int(1..4)`.
    fn matrix_find_1d() -> (SymbolTable, DeclarationPtr) {
        let mut symbols = SymbolTable::new();
        let domain = Domain::matrix(
            Domain::int(vec![Range::Bounded(1, 4)]),
            vec![Domain::int(vec![Range::Bounded(1, 3)])],
        );
        let decl = DeclarationPtr::new_find(Name::user("m"), domain);
        symbols
            .insert(decl.clone())
            .expect("matrix find should insert");
        let mut decl_for_init = decl.clone();
        let (extra, _tops) = MatrixComponents::init_for(&mut decl_for_init).unwrap();
        symbols.update_insert(decl_for_init.clone());
        symbols.extend(extra);
        (symbols, decl_for_init)
    }

    #[test]
    fn unsafe_const_index_matrix_components_lowers_in_bounds_constants() {
        let (symbols, decl) = matrix_find_1d();
        let subject = Expression::Atomic(Metadata::new(), Atom::Reference(Reference::new(decl)));
        let expr = Expression::UnsafeIndex(Metadata::new(), Moo::new(subject), vec![2.into()]);

        let rule = get_rule_by_name("unsafe_const_index_matrix_components")
            .expect("unsafe_const_index_matrix_components registered");
        let result = rule
            .apply(&expr, &symbols)
            .expect("fused const index applies");
        assert!(matches!(
            result.new_expression,
            Expression::Atomic(_, Atom::Reference(_))
        ));
    }

    #[test]
    fn try_lower_const_unsafe_index_matrix_components_matches_rule() {
        let (_symbols, decl) = matrix_find_1d();
        let subject = Expression::Atomic(Metadata::new(), Atom::Reference(Reference::new(decl)));
        let expr = Expression::UnsafeIndex(Metadata::new(), Moo::new(subject), vec![2.into()]);

        let lowered = try_lower_const_unsafe_index_matrix_components(&expr)
            .expect("helper should lower in-bounds constants");
        assert!(matches!(lowered, Expression::Atomic(_, Atom::Reference(_))));
        assert!(try_lower_const_unsafe_index_matrix_components(&Expression::from(true)).is_none());
    }

    #[test]
    fn unsafe_const_index_matrix_components_refuses_out_of_bounds() {
        let (symbols, decl) = matrix_find_1d();
        let subject = Expression::Atomic(Metadata::new(), Atom::Reference(Reference::new(decl)));
        let expr = Expression::UnsafeIndex(Metadata::new(), Moo::new(subject), vec![9.into()]);

        let rule = get_rule_by_name("unsafe_const_index_matrix_components")
            .expect("unsafe_const_index_matrix_components registered");
        let err = rule.apply(&expr, &symbols).unwrap_err();
        assert!(matches!(err, ApplicationError::RuleNotApplicable));
    }

    #[test]
    fn unsafe_const_index_matrix_components_refuses_non_constant_index() {
        let (symbols, decl) = matrix_find_1d();
        let idx_dom = Domain::int(vec![Range::Bounded(1, 3)]);
        let idx_decl = DeclarationPtr::new_find(Name::user("i"), idx_dom);
        let mut symbols = symbols;
        symbols
            .insert(idx_decl.clone())
            .expect("index find inserts");

        let subject = Expression::Atomic(Metadata::new(), Atom::Reference(Reference::new(decl)));
        let index = Expression::Atomic(Metadata::new(), Atom::Reference(Reference::new(idx_decl)));
        let expr = Expression::UnsafeIndex(Metadata::new(), Moo::new(subject), vec![index]);

        let rule = get_rule_by_name("unsafe_const_index_matrix_components")
            .expect("unsafe_const_index_matrix_components registered");
        let err = rule.apply(&expr, &symbols).unwrap_err();
        assert!(matches!(err, ApplicationError::RuleNotApplicable));
    }
}
