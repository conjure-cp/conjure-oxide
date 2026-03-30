use crate::bottom_up_adaptor::as_bottom_up;
use crate::guard;
use crate::representation::MatrixToAtom;
use crate::utils::to_aux_var;
use conjure_cp::ast::matrix::unflatten_matrix_expr;
use conjure_cp::ast::{
    Atom, DeclarationKind, Expression, GroundDomain, Metadata, Moo, Range, Reference, SymbolTable,
    eval_constant,
};
use conjure_cp::into_matrix_expr;
use conjure_cp::representation::ReprRule;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
    register_rule_set,
};
use conjure_cp::settings::SolverFamily;
use conjure_cp::solver::adaptors::smt::{MatrixTheory, TheoryConfig};
use conjure_cp::{domain_int, essence_expr};
use std::collections::VecDeque;
use uniplate::{Biplate, Uniplate};

register_rule_set!("ReprMatrixToAtom", ("Base"), |f: &SolverFamily| {
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

/// Special-case repr selection for matrices as their only representation is MatrixToAtom
#[register_rule(("ReprMatrixToAtom", 8001))]
fn select_repr_mta(expr: &Expression, symtab: &SymbolTable) -> ApplicationResult {
    let Expression::Root(..) = expr else {
        return Err(RuleNotApplicable);
    };

    // Initialise MatrixToAtom for every matrix var in the symbol table
    let mut new_symtab = symtab.clone();
    let mut new_constraints = Vec::new();
    for (_, decl) in symtab.iter_local() {
        guard!(
            // this is a variable or constant
            matches!(&decl.kind() as &DeclarationKind, DeclarationKind::Find(..) | DeclarationKind::ValueLetting(..)) &&
            // ...which hasn't been represented yet
            decl.reprs().is_empty() &&
            // ...and its domain resolves to a matrix
            let mut new_decl = decl.clone() &&
            let Some(gd) = new_decl.resolve_domain() &&
            matches!(gd.as_ref(), GroundDomain::Matrix(..))
            else {
                continue;
            }
        );

        let (symbols, new_top) = MatrixToAtom::init_for(&mut new_decl).unwrap();
        new_symtab.update_insert(new_decl);
        new_symtab.extend(symbols);
        new_constraints.extend(new_top);
    }

    // Select MatrixToAtom for every matrix variable in the model
    let new_expr = expr.transform_bi(&|mut re: Reference| {
        let _ = re.select_repr_via(&MatrixToAtom);
        re
    });

    // Avoid infinite loop
    let unchanged = new_expr.eq(expr) && new_symtab.eq(symtab);
    if unchanged {
        Err(RuleNotApplicable)
    } else {
        Ok(Reduction::new(new_expr, new_constraints, new_symtab))
    }
}

/// Using the `matrix_to_atom`  representation rule, rewrite matrix indexing.
/// ```plain
/// find m: matrix indexed by [int(1..2), int(1..3), int(1..4)] of bool
/// find x: int(1..3)
/// such that
///
/// m[1, x, 2] = true
/// ~~>
/// [m_1_1_2, m_1_2_2, m_1_3_2][x] = true
/// ```
#[register_rule(("ReprMatrixToAtom", 5000))]
fn index_matrix_to_atom(expr: &Expression, symbols: &SymbolTable) -> ApplicationResult {
    // If we apply this rule top-down, nested indices (e.g m[m[i]]) become a pathological case:
    // The outer one is unwrapped first, creating a massive expression with lots of copies of m[i],
    // and so on (getting exponentially worse with each dimension).
    // Instead, we want to convert the inner `m[i]`, *then* the outer `m[m[i]]`.
    as_bottom_up(index_matrix_to_atom_impl)(expr, symbols)
}

fn index_matrix_to_atom_impl(expr: &Expression, symbols: &SymbolTable) -> ApplicationResult {
    guard!(
        // this is a safe indexing expression
        let Expression::SafeIndex(_, subject, indices) = expr &&
        let Expression::Atomic(_, Atom::Reference(re)) = &**subject &&
        // ...into a variable represented by MatrixToAtom
        let Some(mta) = re.ptr().get_repr::<MatrixToAtom>() &&
        // ...which has a matrix domain
        let dom = re.domain().ok_or(RuleNotApplicable)? &&
        let Some((_, idx_doms)) = dom.as_matrix()
        else {
            return Err(RuleNotApplicable);
        }
    );

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

    let view = mta.slice_lit(&slices);

    // Flat slice of remaining elements to index
    let mut lhs_elems: Vec<Expression> = mta
        .view_cloned(&view)
        .into_iter()
        .map(|decl| Reference::new(decl).into())
        .collect();

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
        let idx_dom_gd = idx_dom.as_ground().expect("idx doms must be ground");

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
                let mapped_idx =
                    Reference::new(idx_auxvars.gensym(&domain_int!(0..(dim_sz as i32 - 1))));
                let mut eq_cases = Vec::new();
                for idx_val in 0..dim_sz {
                    let orig_idx_val = mta.index_flat_to_lit(di, idx_val);
                    eq_cases.push(essence_expr!(
                        r"(&idx_expr = &orig_idx_val) /\ (&mapped_idx = &idx_val)"
                    ));
                }
                // to avoid over-constraining the original `idx_expr`, add a case for when it falls
                // out of matrix bounds; bubbling rules should have dealt with this previously anyway
                let default_case =
                    Expression::InDomain(Metadata::new(), Moo::new(idx_expr), idx_dom.clone());
                eq_cases.push(essence_expr!(!&default_case));
                let eq_cases_disj =
                    Expression::Or(Metadata::new(), Moo::new(into_matrix_expr!(eq_cases)));
                idx_auxvar_constraints.push(eq_cases_disj);

                idx_expr = essence_expr!(&off * &mapped_idx);
            }
        }

        new_rhs_exprs.push_front(idx_expr);
        off *= dim_sz;
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

#[register_rule(("ReprMatrixToAtom", 5000))]
fn slice_matrix_to_atom(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    guard!(
        // this is a safe slicing expression
        let Expression::SafeSlice(_, subject, dim_slices) = expr &&
        let Expression::Atomic(_, Atom::Reference(re)) = &**subject &&
        // ...into a variable represented by MatrixToAtom
        let Some(mta) = re.ptr().get_repr::<MatrixToAtom>() &&
        // ...which has a matrix domain
        let dom = re.domain().ok_or(RuleNotApplicable)? &&
        let Some((_, idx_doms)) = dom.as_matrix()
        else {
            return Err(RuleNotApplicable);
        }
    );

    // All indices that evaluate to a literal are resolved immediately;
    // The rest of the matrix is put into a flat slice which we index by the remaining indices
    let mut slices = Vec::new();
    let mut new_index_domains = Vec::new();
    let mut new_indices = Vec::new();
    for (i, dim_slice) in dim_slices.iter().enumerate() {
        if let Some(idx_expr) = dim_slice
            && let Some(idx_lit) = eval_constant(idx_expr)
        {
            slices.push(Range::Single(idx_lit));
        } else {
            slices.push(Range::Unbounded);
            new_indices.push(dim_slice.clone());
            new_index_domains.push(idx_doms[i].clone());
        }
        // TODO: The above handles indices or `..` slices but not `a..b`
        //       Add handling of `a..b` when range expressions are supported by AST / parser
    }

    let view = mta.slice_lit(&slices);

    // Flat slice of remaining elements to index
    let mut lhs_elems: Vec<Expression> = mta
        .view_cloned(&view)
        .into_iter()
        .map(|decl| Reference::new(decl).into())
        .collect();

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

    // Some indices were not resolved so output a slice into a matrix literal
    let new_lhs = unflatten_matrix_expr(&lhs_elems, &new_index_domains, &view.strides);
    let new_expr = Expression::SafeSlice(Metadata::new(), Moo::new(new_lhs), new_indices);
    Ok(Reduction::pure(new_expr))
}

/// Flatten a represented matrix
/// ```plain
/// flatten(x)
/// ~>
/// [x_MatrixToAtom_1, ..., x_MatrixToAtom_N]
/// ```
#[register_rule(("ReprMatrixToAtom", 5000))]
fn matrix_flatten_to_atom(expr: &Expression, _symbols: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expression::Flatten(_, dims, subj) = expr            &&
        let Expression::Atomic(_, Atom::Reference(re)) = &**subj &&
        let Some(repr) = re.get_repr_as::<MatrixToAtom>()
        else {
            return Err(RuleNotApplicable);
        }
    );

    if dims.is_some() {
        todo!("Handle dimension option in matrix flattening");
    }

    let flat_elems: Vec<Expression> = repr.flat_elem_refs().map(Expression::from).collect();
    Ok(Reduction::pure(into_matrix_expr!(flat_elems)))
}

/// Converts a reference to a 1d-matrix not contained within an indexing or slicing expression to its atoms.
#[register_rule(("ReprMatrixToAtom", 2000))]
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
                && let Some(mta) = re.ptr().get_repr::<MatrixToAtom>()
            {
                changed = true;
                let elem_refs: Vec<Expression> =
                    mta.flat_elem_refs().map(Expression::from).collect();
                into_matrix_expr!(elem_refs)
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
