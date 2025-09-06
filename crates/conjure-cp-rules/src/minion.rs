/************************************************************************/
/*        Rules for translating to Minion-supported constraints         */
/************************************************************************/

use std::collections::VecDeque;
use std::{collections::HashMap, convert::TryInto};

use crate::{
    extra_check,
    utils::{is_flat, to_aux_var},
};
use conjure_cp::ast::Moo;
use conjure_cp::ast::categories::Category;
use conjure_cp::{
    ast::Metadata,
    ast::{
        Atom, Domain, Expression as Expr, Literal as Lit, Range, ReturnType, SymbolTable, Typeable,
    },
    into_matrix_expr, matrix_expr,
    rule_engine::{
        ApplicationError, ApplicationResult, Reduction, register_rule, register_rule_set,
    },
    solver::SolverFamily,
};

use itertools::Itertools;
use uniplate::Uniplate;

use ApplicationError::RuleNotApplicable;

register_rule_set!("Minion", ("Base"), (SolverFamily::Minion));

#[register_rule(("Minion", 4200))]
fn introduce_producteq(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    // product = val
    let val: Atom;
    let product: Moo<Expr>;

    match expr.clone() {
        Expr::Eq(_m, a, b) => {
            let a_atom: Option<&Atom> = (&a).try_into().ok();
            let b_atom: Option<&Atom> = (&b).try_into().ok();

            if let Some(f) = a_atom {
                // val = product
                val = f.clone();
                product = b;
            } else if let Some(f) = b_atom {
                // product = val
                val = f.clone();
                product = a;
            } else {
                return Err(RuleNotApplicable);
            }
        }
        Expr::AuxDeclaration(_m, decl, e) => {
            val = Atom::Reference(decl);
            product = e;
        }
        _ => {
            return Err(RuleNotApplicable);
        }
    }

    if !(matches!(&*product, Expr::Product(_, _,))) {
        return Err(RuleNotApplicable);
    }

    let Expr::Product(_, factors) = &*product else {
        return Err(RuleNotApplicable);
    };

    let mut factors_vec = (**factors).clone().unwrap_list().ok_or(RuleNotApplicable)?;
    if factors_vec.len() < 2 {
        return Err(RuleNotApplicable);
    }

    // Product is a vecop, but FlatProductEq a binop.
    // Introduce auxvars until it is a binop

    // the expression returned will be x*y=val.
    // if factors is > 2 arguments, y will be an auxiliary variable

    #[allow(clippy::unwrap_used)] // should never panic - length is checked above
    let x: Atom = factors_vec
        .pop()
        .unwrap()
        .try_into()
        .or(Err(RuleNotApplicable))?;

    #[allow(clippy::unwrap_used)] // should never panic - length is checked above
    let mut y: Atom = factors_vec
        .pop()
        .unwrap()
        .try_into()
        .or(Err(RuleNotApplicable))?;

    let mut symbols = symbols.clone();
    let mut new_tops: Vec<Expr> = vec![];

    // FIXME: add a test for this
    while let Some(next_factor) = factors_vec.pop() {
        // Despite adding auxvars, I still require all atoms as factors, making this rule act
        // similar to other introduction rules.
        let next_factor_atom: Atom = next_factor.clone().try_into().or(Err(RuleNotApplicable))?;

        // TODO: find this domain without having to make unnecessary Expr and Metadata objects
        // Just using the domain of expr doesn't work
        let aux_domain = Expr::Product(
            Metadata::new(),
            Moo::new(matrix_expr![y.clone().into(), next_factor]),
        )
        .domain_of()
        .ok_or(ApplicationError::DomainError)?;

        let aux_decl = symbols.gensym(&aux_domain);
        let aux_var = Atom::Reference(aux_decl);

        let new_top_expr = Expr::FlatProductEq(
            Metadata::new(),
            Moo::new(y),
            Moo::new(next_factor_atom),
            Moo::new(aux_var.clone()),
        );

        new_tops.push(new_top_expr);
        y = aux_var;
    }

    Ok(Reduction::new(
        Expr::FlatProductEq(Metadata::new(), Moo::new(x), Moo::new(y), Moo::new(val)),
        new_tops,
        symbols,
    ))
}

/// Introduces `FlatWeightedSumLeq`, `FlatWeightedSumGeq`, `FlatSumLeq`, FlatSumGeq` constraints.
///
/// If the input is a weighted sum, the weighted sum constraints are used, otherwise the standard
/// sum constraints are used.
///
/// # Details
/// This rule is a bit unusual compared to other introduction rules in that
/// it does its own flattening.
///
/// Weighted sums are expressed as sums of products, which are not
/// flat. Flattening a weighted sum generically makes it indistinguishable
/// from a sum:
///
///```text
/// 1*a + 2*b + 3*c + d <= 10
///   ~~> flatten_vecop
/// __0 + __1 + __2 + d <= 10
///
///
/// __0 =aux 1*a
/// __1 =aux 2*b
/// __2 =aux 3*c
/// ```
///
/// Therefore, introduce_weightedsumleq_sumgeq does its own flattening.
///
/// Having custom flattening semantics means that we can make more things
/// weighted sums.
///
/// For example, consider `a + 2*b + 3*c*d + (e / f) + 5*(g/h) <= 18`. This
/// rule turns this into a single flat_weightedsumleq constraint:
///
///```text
/// a + 2*b + 3*c*d + (e/f) + 5*(g/h) <= 30
///
///   ~> introduce_weightedsumleq_sumgeq
///
/// flat_weightedsumleq([1,2,3,1,5],[a,b,__0,__1,__2],30)
///
/// with new top level constraints
///
/// __0 = c*d
/// __1 = e/f
/// __2 = g/h
/// ```
///
/// The rules to turn terms into coefficient variable pairs are the following:
///
/// 1. Non-weighted atom: `a ~> (1,a)`
/// 2. Other non-weighted term: `e ~> (1,__0)`, with new constraint `__0 =aux e`
/// 3. Weighted atom: `c*a ~> (c,a)`
/// 4. Weighted non-atom: `c*e ~> (c,__0)` with new constraint` __0 =aux e`
/// 5. Weighted product: `c*e*f ~> (c,__0)` with new constraint `__0 =aux (e*f)`
/// 6. Negated atom: `-x ~> (-1,x)`
/// 7. Negated expression: `-e ~> (-1,__0)` with new constraint `__0 = e`
///
/// Cases 6 and 7 could potentially be a normalising rule `-e ~> -1*e`. However, I think that we
/// should only turn negations into a product when they are inside a sum, not all the time.
#[register_rule(("Minion", 4600))]
fn introduce_weighted_sumleq_sumgeq(expr: &Expr, symtab: &SymbolTable) -> ApplicationResult {
    // Keep track of which type of (in)equality was in the input, and use this to decide what
    // constraints to make at the end

    // We handle Eq directly in this rule instead of letting it be decomposed to <= and >=
    // elsewhere, as this caused cyclic rule application:
    //
    // ```
    // 2*a + b = c
    //
    //   ~~> sumeq_to_inequalities
    //
    // 2*a + b <=c /\ 2*a + b >= c
    //
    // --
    //
    // 2*a + b <= c
    //
    //   ~~> flatten_generic
    // __1 <=c
    //
    // with new top level constraint
    //
    // 2*a + b =aux __1
    //
    // --
    //
    // 2*a + b =aux __1
    //
    //   ~~> sumeq_to_inequalities
    //
    // LOOP!
    // ```
    enum EqualityKind {
        Eq,
        Leq,
        Geq,
    }

    // Given the LHS, RHS, and the type of inequality, return the sum, total, and new inequality.
    //
    // The inequality returned is the one that puts the sum is on the left hand side and the total
    // on the right hand side.
    //
    // For example, `1 <= a + b` will result in ([a,b],1,Geq).
    fn match_sum_total(
        a: Moo<Expr>,
        b: Moo<Expr>,
        equality_kind: EqualityKind,
    ) -> Result<(Vec<Expr>, Atom, EqualityKind), ApplicationError> {
        match (
            Moo::unwrap_or_clone(a),
            Moo::unwrap_or_clone(b),
            equality_kind,
        ) {
            (Expr::Sum(_, sum_terms), Expr::Atomic(_, total), EqualityKind::Leq) => {
                let sum_terms = Moo::unwrap_or_clone(sum_terms)
                    .unwrap_list()
                    .ok_or(RuleNotApplicable)?;
                Ok((sum_terms, total, EqualityKind::Leq))
            }
            (Expr::Atomic(_, total), Expr::Sum(_, sum_terms), EqualityKind::Leq) => {
                let sum_terms = Moo::unwrap_or_clone(sum_terms)
                    .unwrap_list()
                    .ok_or(RuleNotApplicable)?;
                Ok((sum_terms, total, EqualityKind::Geq))
            }
            (Expr::Sum(_, sum_terms), Expr::Atomic(_, total), EqualityKind::Geq) => {
                let sum_terms = Moo::unwrap_or_clone(sum_terms)
                    .unwrap_list()
                    .ok_or(RuleNotApplicable)?;
                Ok((sum_terms, total, EqualityKind::Geq))
            }
            (Expr::Atomic(_, total), Expr::Sum(_, sum_terms), EqualityKind::Geq) => {
                let sum_terms = Moo::unwrap_or_clone(sum_terms)
                    .unwrap_list()
                    .ok_or(RuleNotApplicable)?;
                Ok((sum_terms, total, EqualityKind::Leq))
            }
            (Expr::Sum(_, sum_terms), Expr::Atomic(_, total), EqualityKind::Eq) => {
                let sum_terms = Moo::unwrap_or_clone(sum_terms)
                    .unwrap_list()
                    .ok_or(RuleNotApplicable)?;
                Ok((sum_terms, total, EqualityKind::Eq))
            }
            (Expr::Atomic(_, total), Expr::Sum(_, sum_terms), EqualityKind::Eq) => {
                let sum_terms = Moo::unwrap_or_clone(sum_terms)
                    .unwrap_list()
                    .ok_or(RuleNotApplicable)?;
                Ok((sum_terms, total, EqualityKind::Eq))
            }
            _ => Err(RuleNotApplicable),
        }
    }

    let (sum_exprs, total, equality_kind) = match expr.clone() {
        Expr::Leq(_, a, b) => Ok(match_sum_total(a, b, EqualityKind::Leq)?),
        Expr::Geq(_, a, b) => Ok(match_sum_total(a, b, EqualityKind::Geq)?),
        Expr::Eq(_, a, b) => Ok(match_sum_total(a, b, EqualityKind::Eq)?),
        Expr::AuxDeclaration(_, decl, a) => {
            let total: Atom = Atom::Reference(decl);
            if let Expr::Sum(_, sum_terms) = Moo::unwrap_or_clone(a) {
                let sum_terms = Moo::unwrap_or_clone(sum_terms)
                    .unwrap_list()
                    .ok_or(RuleNotApplicable)?;
                Ok((sum_terms, total, EqualityKind::Eq))
            } else {
                Err(RuleNotApplicable)
            }
        }
        _ => Err(RuleNotApplicable),
    }?;

    let mut new_top_exprs: Vec<Expr> = vec![];
    let mut symtab = symtab.clone();

    #[allow(clippy::mutable_key_type)]
    let mut coefficients_and_vars: HashMap<Atom, i32> = HashMap::new();

    // for each sub-term, get the coefficient and the variable, flattening if necessary.
    //
    for expr in sum_exprs {
        let (coeff, var) = flatten_weighted_sum_term(expr, &mut symtab, &mut new_top_exprs)?;

        if coeff == 0 {
            continue;
        }

        // collect coefficients for like terms, so 2*x + -1*x ~~> 1*x
        coefficients_and_vars
            .entry(var)
            .and_modify(|x| *x += coeff)
            .or_insert(coeff);
    }

    // the expr should use a regular sum instead if the coefficients are all 1.
    let use_weighted_sum = !coefficients_and_vars.values().all(|x| *x == 1 || *x == 0);

    // This needs a consistent iteration order so that the output is deterministic. However,
    // HashMap doesn't provide this. Can't use BTreeMap or Ord to achieve this, as not everything
    // in the AST implements Ord. Instead, order things by their pretty printed value.
    let (vars, coefficients): (Vec<Atom>, Vec<Lit>) = coefficients_and_vars
        .into_iter()
        .filter(|(_, c)| *c != 0)
        .sorted_by(|a, b| {
            let a_atom_str = format!("{}", a.0);
            let b_atom_str = format!("{}", b.0);
            a_atom_str.cmp(&b_atom_str)
        })
        .map(|(v, c)| (v, Lit::Int(c)))
        .unzip();

    let new_expr: Expr = match (equality_kind, use_weighted_sum) {
        (EqualityKind::Eq, true) => Expr::And(
            Metadata::new(),
            Moo::new(matrix_expr![
                Expr::FlatWeightedSumLeq(
                    Metadata::new(),
                    coefficients.clone(),
                    vars.clone(),
                    Moo::new(total.clone()),
                ),
                Expr::FlatWeightedSumGeq(Metadata::new(), coefficients, vars, Moo::new(total)),
            ]),
        ),
        (EqualityKind::Eq, false) => Expr::And(
            Metadata::new(),
            Moo::new(matrix_expr![
                Expr::FlatSumLeq(Metadata::new(), vars.clone(), total.clone()),
                Expr::FlatSumGeq(Metadata::new(), vars, total),
            ]),
        ),
        (EqualityKind::Leq, true) => {
            Expr::FlatWeightedSumLeq(Metadata::new(), coefficients, vars, Moo::new(total))
        }
        (EqualityKind::Leq, false) => Expr::FlatSumLeq(Metadata::new(), vars, total),
        (EqualityKind::Geq, true) => {
            Expr::FlatWeightedSumGeq(Metadata::new(), coefficients, vars, Moo::new(total))
        }
        (EqualityKind::Geq, false) => Expr::FlatSumGeq(Metadata::new(), vars, total),
    };

    Ok(Reduction::new(new_expr, new_top_exprs, symtab))
}

/// For a term inside a weighted sum, return coefficient*variable.
///
///
/// If the term is in the form <coefficient> * <non flat expression>, the expression is flattened
/// to a new auxvar, which is returned as the variable for this term.
///
/// New auxvars are added to `symtab`, and their top level constraints to `top_level_exprs`.
///
/// # Errors
///
/// + Returns [`ApplicationError::RuleNotApplicable`] if a non-flat expression cannot be turned
///   into an atom. See [`flatten_expression_to_atom`].
///
/// + Returns [`ApplicationError::RuleNotApplicable`] if the term is a product containing a matrix
///   literal, and that matrix literal is not a list.
///
///
fn flatten_weighted_sum_term(
    term: Expr,
    symtab: &mut SymbolTable,
    top_level_exprs: &mut Vec<Expr>,
) -> Result<(i32, Atom), ApplicationError> {
    match term {
        // we can only see check the product for coefficients it contains a matrix literal.
        //
        // e.g. the input expression `product([2,x])` returns (2,x), but `product(my_matrix)`
        // returns (1,product(my_matrix)).
        //
        // if the product contains a matrix literal but it is not a list, throw `RuleNotApplicable`
        // to allow it to be changed into a list by another rule.
        Expr::Product(_, factors) if factors.is_matrix_literal() => {
            // this fails if factors is not a matrix literal or that matrix literal is not a list.
            //
            // we already check for the first case above, so this should only error when we have a
            // non-list matrix literal.
            let factors = Moo::unwrap_or_clone(factors)
                .unwrap_list()
                .ok_or(RuleNotApplicable)?;

            match factors.as_slice() {
                // product([]) ~~> (0,0)
                // coefficients of 0 should be ignored by the caller.
                [] => Ok((0, Atom::Literal(Lit::Int(0)))),

                // product([4,y]) ~~> (4,y)
                [Expr::Atomic(_, Atom::Literal(Lit::Int(coeff))), e] => Ok((
                    *coeff,
                    flatten_expression_to_atom(e.clone(), symtab, top_level_exprs)?,
                )),

                // product([y,4]) ~~> (y,4)
                [e, Expr::Atomic(_, Atom::Literal(Lit::Int(coeff)))] => Ok((
                    *coeff,
                    flatten_expression_to_atom(e.clone(), symtab, top_level_exprs)?,
                )),

                // assume the coefficients have been placed at the front by normalisation rules

                // product[1,x,y,...] ~> return (coeff,product([x,y,...]))
                [
                    Expr::Atomic(_, Atom::Literal(Lit::Int(coeff))),
                    e,
                    rest @ ..,
                ] => {
                    let mut product_terms = Vec::from(rest);
                    product_terms.push(e.clone());
                    let product =
                        Expr::Product(Metadata::new(), Moo::new(into_matrix_expr!(product_terms)));
                    Ok((
                        *coeff,
                        flatten_expression_to_atom(product, symtab, top_level_exprs)?,
                    ))
                }

                // no coefficient:
                // product([x,y,z]) ~~> (1,product([x,y,z])
                _ => {
                    let product =
                        Expr::Product(Metadata::new(), Moo::new(into_matrix_expr!(factors)));
                    Ok((
                        1,
                        flatten_expression_to_atom(product, symtab, top_level_exprs)?,
                    ))
                }
            }
        }
        Expr::Neg(_, inner_term) => Ok((
            -1,
            flatten_expression_to_atom(Moo::unwrap_or_clone(inner_term), symtab, top_level_exprs)?,
        )),
        term => Ok((
            1,
            flatten_expression_to_atom(term, symtab, top_level_exprs)?,
        )),
    }
}

/// Converts the input expression to an atom, placing it into a new auxiliary variable if
/// necessary.
///
/// The auxiliary variable will be added to the symbol table and its top-level-constraint to
/// `top_level_exprs`.
///
/// If the expression is already atomic, no auxiliary variables are created, and the atom is
/// returned as-is.
///
/// # Errors
///
///  + Returns [`ApplicationError::RuleNotApplicable`] if the expression cannot be placed into an
///    auxiliary variable. For example, expressions that do not have domains.
///
///    This function supports the same expressions as [`to_aux_var`], except that this functions
///    succeeds when the expression given is atomic.
///
///    See [`to_aux_var`] for more information.
///
fn flatten_expression_to_atom(
    expr: Expr,
    symtab: &mut SymbolTable,
    top_level_exprs: &mut Vec<Expr>,
) -> Result<Atom, ApplicationError> {
    if let Expr::Atomic(_, atom) = expr {
        return Ok(atom);
    }

    let aux_var_info = to_aux_var(&expr, symtab).ok_or(RuleNotApplicable)?;
    *symtab = aux_var_info.symbols();
    top_level_exprs.push(aux_var_info.top_level_expr());

    Ok(aux_var_info.as_atom())
}

#[register_rule(("Minion", 4200))]
fn introduce_diveq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    // div = val
    let val: Atom;
    let div: Moo<Expr>;
    let meta: Metadata;

    match expr.clone() {
        Expr::Eq(m, a, b) => {
            meta = m;

            let a_atom: Option<&Atom> = (&a).try_into().ok();
            let b_atom: Option<&Atom> = (&b).try_into().ok();

            if let Some(f) = a_atom {
                // val = div
                val = f.clone();
                div = b;
            } else if let Some(f) = b_atom {
                // div = val
                val = f.clone();
                div = a;
            } else {
                return Err(RuleNotApplicable);
            }
        }
        Expr::AuxDeclaration(m, decl, e) => {
            meta = m;
            val = Atom::Reference(decl);
            div = e;
        }
        _ => {
            return Err(RuleNotApplicable);
        }
    }

    if !(matches!(&*div, Expr::SafeDiv(_, _, _))) {
        return Err(RuleNotApplicable);
    }

    let children: VecDeque<Expr> = div.as_ref().children();
    let a: &Atom = (&children[0]).try_into().or(Err(RuleNotApplicable))?;
    let b: &Atom = (&children[1]).try_into().or(Err(RuleNotApplicable))?;

    Ok(Reduction::pure(Expr::MinionDivEqUndefZero(
        meta.clone_dirty(),
        Moo::new(a.clone()),
        Moo::new(b.clone()),
        Moo::new(val),
    )))
}

#[register_rule(("Minion", 4200))]
fn introduce_modeq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    // div = val
    let val: Atom;
    let div: Moo<Expr>;
    let meta: Metadata;

    match expr.clone() {
        Expr::Eq(m, a, b) => {
            meta = m;
            let a_atom: Option<&Atom> = (&a).try_into().ok();
            let b_atom: Option<&Atom> = (&b).try_into().ok();

            if let Some(f) = a_atom {
                // val = div
                val = f.clone();
                div = b;
            } else if let Some(f) = b_atom {
                // div = val
                val = f.clone();
                div = a;
            } else {
                return Err(RuleNotApplicable);
            }
        }
        Expr::AuxDeclaration(m, decl, e) => {
            meta = m;
            val = Atom::Reference(decl);
            div = e;
        }
        _ => {
            return Err(RuleNotApplicable);
        }
    }

    if !(matches!(&*div, Expr::SafeMod(_, _, _))) {
        return Err(RuleNotApplicable);
    }

    let children: VecDeque<Expr> = div.as_ref().children();
    let a: &Atom = (&children[0]).try_into().or(Err(RuleNotApplicable))?;
    let b: &Atom = (&children[1]).try_into().or(Err(RuleNotApplicable))?;

    Ok(Reduction::pure(Expr::MinionModuloEqUndefZero(
        meta.clone_dirty(),
        Moo::new(a.clone()),
        Moo::new(b.clone()),
        Moo::new(val),
    )))
}

#[register_rule(("Minion", 4400))]
fn introduce_abseq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (x, abs_y): (Atom, Expr) = match expr.clone() {
        Expr::Eq(_, a, b) => {
            let a: Expr = Moo::unwrap_or_clone(a);
            let b: Expr = Moo::unwrap_or_clone(b);
            let a_atom: Option<&Atom> = (&a).try_into().ok();
            let b_atom: Option<&Atom> = (&b).try_into().ok();

            if let Some(a_atom) = a_atom {
                Ok((a_atom.clone(), b))
            } else if let Some(b_atom) = b_atom {
                Ok((b_atom.clone(), a))
            } else {
                Err(RuleNotApplicable)
            }
        }

        Expr::AuxDeclaration(_, decl, expr) => {
            let a = Atom::Reference(decl);
            let expr = Moo::unwrap_or_clone(expr);
            Ok((a, expr))
        }

        _ => Err(RuleNotApplicable),
    }?;

    let Expr::Abs(_, y) = abs_y else {
        return Err(RuleNotApplicable);
    };

    let y = Moo::unwrap_or_clone(y);
    let y: Atom = y.try_into().or(Err(RuleNotApplicable))?;

    Ok(Reduction::pure(Expr::FlatAbsEq(
        Metadata::new(),
        Moo::new(x),
        Moo::new(y),
    )))
}

/// Introduces a `MinionPowEq` constraint from a `SafePow`
#[register_rule(("Minion", 4200))]
fn introduce_poweq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (a, b, total) = match expr.clone() {
        Expr::Eq(_, e1, e2) => match (Moo::unwrap_or_clone(e1), Moo::unwrap_or_clone(e2)) {
            (Expr::Atomic(_, total), Expr::SafePow(_, a, b)) => Ok((a, b, total)),
            (Expr::SafePow(_, a, b), Expr::Atomic(_, total)) => Ok((a, b, total)),
            _ => Err(RuleNotApplicable),
        },

        Expr::AuxDeclaration(_, total_decl, e) => match Moo::unwrap_or_clone(e) {
            Expr::SafePow(_, a, b) => {
                let total_ref_atom = Atom::Reference(total_decl);
                Ok((a, b, total_ref_atom))
            }
            _ => Err(RuleNotApplicable),
        },
        _ => Err(RuleNotApplicable),
    }?;

    let a: Atom = Moo::unwrap_or_clone(a)
        .try_into()
        .or(Err(RuleNotApplicable))?;
    let b: Atom = Moo::unwrap_or_clone(b)
        .try_into()
        .or(Err(RuleNotApplicable))?;

    Ok(Reduction::pure(Expr::MinionPow(
        Metadata::new(),
        Moo::new(a),
        Moo::new(b),
        Moo::new(total),
    )))
}

/// Introduces a `FlatAlldiff` constraint from an `AllDiff`
#[register_rule(("Minion", 4200))]
fn introduce_flat_alldiff(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::AllDiff(_, es) = expr else {
        return Err(RuleNotApplicable);
    };

    let es = (**es).clone().unwrap_list().ok_or(RuleNotApplicable)?;

    let atoms = es
        .into_iter()
        .map(|e| match e {
            Expr::Atomic(_, atom) => Ok(atom),
            _ => Err(RuleNotApplicable),
        })
        .process_results(|iter| iter.collect_vec())?;

    Ok(Reduction::pure(Expr::FlatAllDiff(Metadata::new(), atoms)))
}

/// Introduces a Minion `MinusEq` constraint from `x = -y`, where x and y are atoms.
///
/// ```text
/// x = -y ~> MinusEq(x,y)
///
///   where x,y are atoms
/// ```
#[register_rule(("Minion", 4400))]
fn introduce_minuseq_from_eq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Eq(_, a, b) = expr else {
        return Err(RuleNotApplicable);
    };

    fn try_get_atoms(a: &Moo<Expr>, b: &Moo<Expr>) -> Option<(Atom, Atom)> {
        let a: &Atom = (&**a).try_into().ok()?;
        let Expr::Neg(_, b) = &**b else {
            return None;
        };

        let b: &Atom = b.try_into().ok()?;

        Some((a.clone(), b.clone()))
    }

    // x = - y. Find this symmetrically (a = - b or b = -a)
    let Some((x, y)) = try_get_atoms(a, b).or_else(|| try_get_atoms(b, a)) else {
        return Err(RuleNotApplicable);
    };

    Ok(Reduction::pure(Expr::FlatMinusEq(
        Metadata::new(),
        Moo::new(x),
        Moo::new(y),
    )))
}

/// Introduces a Minion `MinusEq` constraint from `x =aux -y`, where x and y are atoms.
///
/// ```text
/// x =aux -y ~> MinusEq(x,y)
///
///   where x,y are atoms
/// ```
#[register_rule(("Minion", 4400))]
fn introduce_minuseq_from_aux_decl(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    // a =aux -b
    //
    let Expr::AuxDeclaration(_, decl, b) = expr else {
        return Err(RuleNotApplicable);
    };

    let a = Atom::Reference(decl.clone());

    let Expr::Neg(_, b) = (**b).clone() else {
        return Err(RuleNotApplicable);
    };

    let Ok(b) = b.try_into() else {
        return Err(RuleNotApplicable);
    };

    Ok(Reduction::pure(Expr::FlatMinusEq(
        Metadata::new(),
        Moo::new(a),
        Moo::new(b),
    )))
}

/// Converts an implication to either `ineq` or `reifyimply`
///
/// ```text
/// x -> y ~> ineq(x,y,0)
/// where x is atomic, y is atomic
///
/// x -> y ~> reifyimply(y,x)
/// where x is atomic, y is non-atomic
/// ```
#[register_rule(("Minion", 4400))]
fn introduce_reifyimply_ineq_from_imply(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Imply(_, x, y) = expr else {
        return Err(RuleNotApplicable);
    };

    let x_atom: &Atom = x.as_ref().try_into().or(Err(RuleNotApplicable))?;

    // if both x and y are atoms,  x -> y ~> ineq(x,y,0)
    //
    // if only x is an atom, x -> y ~> reifyimply(y,x)
    if let Ok(y_atom) = TryInto::<&Atom>::try_into(y.as_ref()) {
        Ok(Reduction::pure(Expr::FlatIneq(
            Metadata::new(),
            Moo::new(x_atom.clone()),
            Moo::new(y_atom.clone()),
            Box::new(0.into()),
        )))
    } else {
        Ok(Reduction::pure(Expr::MinionReifyImply(
            Metadata::new(),
            y.clone(),
            x_atom.clone(),
        )))
    }
}

/// Converts `__inDomain(a,domain) to w-inintervalset.
///
/// This applies if domain is integer and finite.
#[register_rule(("Minion", 4400))]
fn introduce_wininterval_set_from_indomain(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::InDomain(_, e, domain) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, atom @ Atom::Reference(_)) = e.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let Domain::Int(ranges) = domain else {
        return Err(RuleNotApplicable);
    };

    let mut out_ranges = vec![];

    for range in ranges {
        match range {
            Range::Single(x) => {
                out_ranges.push(*x);
                out_ranges.push(*x);
            }
            Range::Bounded(x, y) => {
                out_ranges.push(*x);
                out_ranges.push(*y);
            }
            Range::UnboundedR(_) | Range::UnboundedL(_) => {
                return Err(RuleNotApplicable);
            }
        }
    }

    Ok(Reduction::pure(Expr::MinionWInIntervalSet(
        Metadata::new(),
        atom.clone(),
        out_ranges,
    )))
}

/// Converts `[....][i]` to `element_one` if:
///
/// 1. the subject is a list literal
/// 2. the subject is one dimensional
#[register_rule(("Minion", 4400))]
fn introduce_element_from_index(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (equalto, subject, indices) = match expr.clone() {
        Expr::Eq(_, e1, e2) => match (Moo::unwrap_or_clone(e1), Moo::unwrap_or_clone(e2)) {
            (Expr::Atomic(_, eq), Expr::SafeIndex(_, subject, indices)) => {
                Ok((eq, subject, indices))
            }
            (Expr::SafeIndex(_, subject, indices), Expr::Atomic(_, eq)) => {
                Ok((eq, subject, indices))
            }
            _ => Err(RuleNotApplicable),
        },
        Expr::AuxDeclaration(_, decl, expr) => match Moo::unwrap_or_clone(expr) {
            Expr::SafeIndex(_, subject, indices) => Ok((Atom::Reference(decl), subject, indices)),
            _ => Err(RuleNotApplicable),
        },
        _ => Err(RuleNotApplicable),
    }?;

    if indices.len() != 1 {
        return Err(RuleNotApplicable);
    }

    let Some(list) = Moo::unwrap_or_clone(subject).unwrap_list() else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, index) = indices[0].clone() else {
        return Err(RuleNotApplicable);
    };

    let mut atom_list = vec![];

    for elem in list {
        let Expr::Atomic(_, elem) = elem else {
            return Err(RuleNotApplicable);
        };

        atom_list.push(elem);
    }

    Ok(Reduction::pure(Expr::MinionElementOne(
        Metadata::new(),
        atom_list,
        Moo::new(index),
        Moo::new(equalto),
    )))
}

/// Flattens an implication.
///
/// ```text
/// e -> y  (where e is non atomic)
///  ~~>
/// __0 -> y,
///
/// with new top level constraints
/// __0 =aux x
///
/// ```
///
/// Unlike other expressions, only the left hand side of implications are flattened. This is
/// because implications can be expressed as a `reifyimply` constraint, which takes a constraint as
/// an argument:
///
/// ``` text
/// r -> c ~> refifyimply(r,c)
///  where r is an atom, c is a constraint
/// ```
///
/// See [`introduce_reifyimply_ineq_from_imply`].
#[register_rule(("Minion", 4200))]
fn flatten_imply(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Imply(meta, x, y) = expr else {
        return Err(RuleNotApplicable);
    };

    // flatten x
    let aux_var_info = to_aux_var(x.as_ref(), symbols).ok_or(RuleNotApplicable)?;

    let symbols = aux_var_info.symbols();
    let new_x = aux_var_info.as_expr();

    Ok(Reduction::new(
        Expr::Imply(meta.clone(), Moo::new(new_x), y.clone()),
        vec![aux_var_info.top_level_expr()],
        symbols,
    ))
}

#[register_rule(("Minion", 4200))]
fn flatten_generic(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    if !matches!(
        expr,
        Expr::SafeDiv(_, _, _)
            | Expr::Neq(_, _, _)
            | Expr::SafeMod(_, _, _)
            | Expr::SafePow(_, _, _)
            | Expr::Leq(_, _, _)
            | Expr::Geq(_, _, _)
            | Expr::Abs(_, _)
            | Expr::Neg(_, _)
            | Expr::Not(_, _)
            | Expr::SafeIndex(_, _, _)
            | Expr::InDomain(_, _, _)
    ) {
        return Err(RuleNotApplicable);
    }

    let mut children = expr.children();

    let mut symbols = symbols.clone();
    let mut num_changed = 0;
    let mut new_tops: Vec<Expr> = vec![];

    for child in children.iter_mut() {
        if let Some(aux_var_info) = to_aux_var(child, &symbols) {
            symbols = aux_var_info.symbols();
            new_tops.push(aux_var_info.top_level_expr());
            *child = aux_var_info.as_expr();
            num_changed += 1;
        }
    }

    if num_changed == 0 {
        return Err(RuleNotApplicable);
    }

    let expr = expr.with_children(children);

    Ok(Reduction::new(expr, new_tops, symbols))
}

#[register_rule(("Minion", 4200))]
fn flatten_eq(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    if !matches!(expr, Expr::Eq(_, _, _)) {
        return Err(RuleNotApplicable);
    }

    let children = expr.children();
    debug_assert_eq!(children.len(), 2);

    let mut new_children = VecDeque::new();
    let mut symbols = symbols.clone();
    let mut num_changed = 0;
    let mut new_tops: Vec<Expr> = vec![];

    for child in children {
        if let Some(aux_var_info) = to_aux_var(&child, &symbols) {
            symbols = aux_var_info.symbols();
            new_tops.push(aux_var_info.top_level_expr());
            new_children.push_back(aux_var_info.as_expr());
            num_changed += 1;
        }
    }

    // eq: both sides have to be non flat for the rule to be applicable!
    if num_changed != 2 {
        return Err(RuleNotApplicable);
    }

    let expr = expr.with_children(new_children);

    Ok(Reduction::new(expr, new_tops, symbols))
}

/// Flattens products containing lists.
///
/// For example,
///
/// ```plain
/// product([|x|,y,z]) ~~> product([aux1,y,z]), aux1=|x|
/// ```
#[register_rule(("Minion", 4200))]
fn flatten_product(expr: &Expr, symtab: &SymbolTable) -> ApplicationResult {
    // product cannot use flatten_generic as we don't want to put the immediate child in an aux
    // var, as that is the matrix literal. Instead we want to put the children of the matrix
    // literal in an aux var.
    //
    // e.g.
    //
    // flatten_generic would do
    //
    // product([|x|,y,z]) ~~> product(aux1), aux1=[x,y,z]
    //
    // we want to do
    //
    // product([|x|,y,z]) ~~> product([aux1,y,z]), aux1=|x|
    //
    // We only want to flatten products containing matrix literals that are lists but not child terms, e.g.
    //
    //  product(x[1,..]) ~~> product(aux1),aux1 = x[1,..].
    //
    //  Instead, we let the representation and vertical rules for matrices turn x[1,..] into a
    //  matrix literal.
    //
    //  product(x[1,..]) ~~ slice_matrix_to_atom ~~> product([x11,x12,x13,x14])

    let Expr::Product(_, factors) = expr else {
        return Err(RuleNotApplicable);
    };

    let factors = (**factors).clone().unwrap_list().ok_or(RuleNotApplicable)?;

    let mut new_factors = vec![];
    let mut top_level_exprs = vec![];
    let mut symtab = symtab.clone();

    for factor in factors {
        new_factors.push(Expr::Atomic(
            Metadata::new(),
            flatten_expression_to_atom(factor, &mut symtab, &mut top_level_exprs)?,
        ));
    }

    // have we done anything?
    // if we have created any aux-vars, they will have added a top_level_declaration.
    if top_level_exprs.is_empty() {
        return Err(RuleNotApplicable);
    }

    let new_expr = Expr::Product(Metadata::new(), Moo::new(into_matrix_expr![new_factors]));
    Ok(Reduction::new(new_expr, top_level_exprs, symtab))
}

/// Flattens a matrix literal that contains expressions.
///
/// For example,
///
/// ```plain
/// [1,e/2,f*5] ~~> [1,__0,__1],
///
/// where
/// __0 =aux e/2,
/// __1 =aux f*5
/// ```
#[register_rule(("Minion", 1000))] // this should be a lower priority than matrix to list
fn flatten_matrix_literal(expr: &Expr, symtab: &SymbolTable) -> ApplicationResult {
    // do not flatten matrix literals inside sum, or, and, product as these expressions either do
    // their own flattening, or do not need flat expressions.
    if matches!(
        expr,
        Expr::And(_, _) | Expr::Or(_, _) | Expr::Sum(_, _) | Expr::Product(_, _)
    ) {
        return Err(RuleNotApplicable);
    }

    // flatten any children that are matrix literals
    let mut children = expr.children();

    let mut has_changed = false;
    let mut symbols = symtab.clone();
    let mut top_level_exprs = vec![];

    for child in children.iter_mut() {
        // is this a matrix literal?
        //
        // as we arn't changing the number of arguments in the matrix, we can apply this to all
        // matrices, not just those that are lists.
        //
        // this also means that this rule works with n-d matrices -- the inner dimensions of n-d
        // matrices cant be turned into lists, as described the docstring for matrix_to_list.
        let Some((mut es, index_domain)) = child.clone().unwrap_matrix_unchecked() else {
            continue;
        };

        // flatten expressions
        for e in es.iter_mut() {
            if let Some(aux_info) = to_aux_var(e, &symbols) {
                *e = aux_info.as_expr();
                top_level_exprs.push(aux_info.top_level_expr());
                symbols = aux_info.symbols();
                has_changed = true;
            } else if let Expr::SafeIndex(_, subject, _) = e
                && !matches!(**subject, Expr::Atomic(_, Atom::Reference(_)))
            {
                // we dont normally flatten indexing expressions, but we want to do it if they are
                // inside a matrix list.
                //
                // remove_dimension_from_matrix_indexing turns [[1,2,3],[4,5,6]][i,j]
                // into [[1,2,3][j],[4,5,6][j],[7,8,9][j]][i].
                //
                // we want to flatten this to
                // [__0,__1,__2][i]

                let Some(domain) = e.domain_of() else {
                    continue;
                };

                let categories = e.universe_categories();

                // must contain a decision variable
                if !categories.contains(&Category::Decision) {
                    continue;
                }

                // must not contain givens or quantified variables
                if categories.contains(&Category::Parameter)
                    || categories.contains(&Category::Quantified)
                {
                    continue;
                }

                let decl = symbols.gensym(&domain);

                top_level_exprs.push(Expr::AuxDeclaration(
                    Metadata::new(),
                    decl.clone(),
                    Moo::new(e.clone()),
                ));

                *e = Expr::Atomic(Metadata::new(), Atom::Reference(decl));

                has_changed = true;
            }
        }

        *child = into_matrix_expr!(es;index_domain);
    }

    if has_changed {
        Ok(Reduction::new(
            expr.with_children(children),
            top_level_exprs,
            symbols,
        ))
    } else {
        Err(RuleNotApplicable)
    }
}

/// Converts a Geq to an Ineq
///
/// ```text
/// x >= y ~> y <= x + 0 ~> ineq(y,x,0)
/// ```
#[register_rule(("Minion", 4100))]
fn geq_to_ineq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Geq(meta, e1, e2) = expr.clone() else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, x) = Moo::unwrap_or_clone(e1) else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, y) = Moo::unwrap_or_clone(e2) else {
        return Err(RuleNotApplicable);
    };

    Ok(Reduction::pure(Expr::FlatIneq(
        meta.clone_dirty(),
        Moo::new(y),
        Moo::new(x),
        Box::new(Lit::Int(0)),
    )))
}

/// Converts a Leq to an Ineq
///
/// ```text
/// x <= y ~> x <= y + 0 ~> ineq(x,y,0)
/// ```
#[register_rule(("Minion", 4100))]
fn leq_to_ineq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Leq(meta, e1, e2) = expr.clone() else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, x) = Moo::unwrap_or_clone(e1) else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, y) = Moo::unwrap_or_clone(e2) else {
        return Err(RuleNotApplicable);
    };

    Ok(Reduction::pure(Expr::FlatIneq(
        meta.clone_dirty(),
        Moo::new(x),
        Moo::new(y),
        Box::new(Lit::Int(0)),
    )))
}

// TODO: add this rule for geq

/// ```text
/// x <= y + k ~> ineq(x,y,k)
/// ```
#[register_rule(("Minion",4500))]
fn x_leq_y_plus_k_to_ineq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Leq(meta, e1, e2) = expr.clone() else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, x) = Moo::unwrap_or_clone(e1) else {
        return Err(RuleNotApplicable);
    };

    let Expr::Sum(_, sum_exprs) = Moo::unwrap_or_clone(e2) else {
        return Err(RuleNotApplicable);
    };

    let sum_exprs = (*sum_exprs)
        .clone()
        .unwrap_list()
        .ok_or(RuleNotApplicable)?;
    let (y, k) = match sum_exprs.as_slice() {
        [Expr::Atomic(_, y), Expr::Atomic(_, Atom::Literal(k))] => (y, k),
        [Expr::Atomic(_, Atom::Literal(k)), Expr::Atomic(_, y)] => (y, k),
        _ => {
            return Err(RuleNotApplicable);
        }
    };

    Ok(Reduction::pure(Expr::FlatIneq(
        meta.clone_dirty(),
        Moo::new(x),
        Moo::new(y.clone()),
        Box::new(k.clone()),
    )))
}

/// ```text
/// y + k >= x ~> ineq(x,y,k)
/// ```
#[register_rule(("Minion",4800))]
fn y_plus_k_geq_x_to_ineq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    // impl same as x_leq_y_plus_k but with lhs and rhs flipped
    let Expr::Geq(meta, e2, e1) = expr.clone() else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, x) = Moo::unwrap_or_clone(e1) else {
        return Err(RuleNotApplicable);
    };

    let Expr::Sum(_, sum_exprs) = Moo::unwrap_or_clone(e2) else {
        return Err(RuleNotApplicable);
    };

    let sum_exprs = Moo::unwrap_or_clone(sum_exprs)
        .unwrap_list()
        .ok_or(RuleNotApplicable)?;
    let (y, k) = match sum_exprs.as_slice() {
        [Expr::Atomic(_, y), Expr::Atomic(_, Atom::Literal(k))] => (y, k),
        [Expr::Atomic(_, Atom::Literal(k)), Expr::Atomic(_, y)] => (y, k),
        _ => {
            return Err(RuleNotApplicable);
        }
    };

    Ok(Reduction::pure(Expr::FlatIneq(
        meta.clone_dirty(),
        Moo::new(x),
        Moo::new(y.clone()),
        Box::new(k.clone()),
    )))
}

/// Flattening rule for not(bool_lit)
///
/// For some boolean variable x:
/// ```text
///  not(x)      ~>  w-literal(x,0)
/// ```
///
/// ## Rationale
///
/// Minion's watched-and and watched-or constraints only takes other constraints as arguments.
///
/// This restates boolean variables as the equivalent constraint "SAT if x is true".
///
/// The regular bool_lit case is dealt with directly by the Minion solver interface (as it is a
/// trivial match).

#[register_rule(("Minion", 4100))]
fn not_literal_to_wliteral(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    use Domain::Bool;
    match expr {
        Expr::Not(m, expr) => {
            if let Expr::Atomic(_, Atom::Reference(decl)) = (**expr).clone() {
                if decl.domain().is_some_and(|x| matches!(&x as &Domain, Bool)) {
                    return Ok(Reduction::pure(Expr::FlatWatchedLiteral(
                        m.clone_dirty(),
                        decl,
                        Lit::Bool(false),
                    )));
                }
            }
            Err(RuleNotApplicable)
        }
        _ => Err(RuleNotApplicable),
    }
}

/// Flattening rule for not(X) in Minion, where X is a constraint.
///
/// ```text
/// not(X) ~> reify(X,0)
/// ```
///
/// This rule has lower priority than boolean_literal_to_wliteral so that we can assume that the
/// nested expressions are constraints not variables.

#[register_rule(("Minion", 4090))]
fn not_constraint_to_reify(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    if !matches!(expr, Expr::Not(_,c) if !matches!(**c, Expr::Atomic(_,_))) {
        return Err(RuleNotApplicable);
    }

    let Expr::Not(m, e) = expr else {
        unreachable!();
    };

    extra_check! {
        if !is_flat(e) {
            return Err(RuleNotApplicable);
        }
    };

    Ok(Reduction::pure(Expr::MinionReify(
        m.clone(),
        e.clone(),
        Atom::Literal(Lit::Bool(false)),
    )))
}

/// Converts an equality to a boolean into a `reify` constraint.
///
/// ```text
/// x =aux c ~> reify(c,x)
/// x = c ~> reify(c,x)
///
/// where c is a boolean constraint
/// ```
#[register_rule(("Minion", 4400))]
fn bool_eq_to_reify(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (atom, e): (Atom, Moo<Expr>) = match expr {
        Expr::AuxDeclaration(_, decl, e) => Ok((Atom::from(decl.clone()), e.clone())),
        Expr::Eq(_, a, b) => match (a.as_ref(), b.as_ref()) {
            (Expr::Atomic(_, atom), _) => Ok((atom.clone(), b.clone())),
            (_, Expr::Atomic(_, atom)) => Ok((atom.clone(), a.clone())),
            _ => Err(RuleNotApplicable),
        },

        _ => Err(RuleNotApplicable),
    }?;

    // e does not have to be valid minion constraint yet, as long as we know it can turn into one
    // (i.e. it is boolean).
    let Some(ReturnType::Bool) = e.as_ref().return_type() else {
        return Err(RuleNotApplicable);
    };

    Ok(Reduction::pure(Expr::MinionReify(Metadata::new(), e, atom)))
}

/// Converts an iff to an `Eq` constraint.
///
/// ```text
/// Iff(a,b) ~> Eq(a,b)
///
/// ```
#[register_rule(("Minion", 4400))]
fn iff_to_eq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Iff(_, x, y) = expr else {
        return Err(RuleNotApplicable);
    };

    Ok(Reduction::pure(Expr::Eq(
        Metadata::new(),
        x.clone(),
        y.clone(),
    )))
}
