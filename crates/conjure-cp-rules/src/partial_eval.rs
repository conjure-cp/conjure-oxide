use std::collections::HashSet;

use conjure_cp::rule_engine::register_rule;
use conjure_cp::{
    ast::Metadata,
    ast::{Domain, Moo, ReturnType, Typeable as _},
    into_matrix_expr,
    rule_engine::{ApplicationResult, Reduction},
};
use itertools::iproduct;

use conjure_cp::ast::{Atom, Expression as Expr, Literal as Lit, SymbolTable};

#[register_rule(("Base",9000))]
fn partial_evaluator(expr: &Expr, symtab: &SymbolTable) -> ApplicationResult {
    run_partial_evaluator(expr, symtab)
}

pub(super) fn run_partial_evaluator(expr: &Expr, symtab: &SymbolTable) -> ApplicationResult {
    use conjure_cp::rule_engine::ApplicationError::RuleNotApplicable;
    // NOTE: If nothing changes, we must return RuleNotApplicable, or the rewriter will try this
    // rule infinitely!
    // This is why we always check whether we found a constant or not.
    match expr.clone() {
        Expr::Union(_, _, _) => Err(RuleNotApplicable),
        Expr::In(_, _, _) => Err(RuleNotApplicable),
        Expr::Intersect(_, _, _) => Err(RuleNotApplicable),
        Expr::Supset(_, _, _) => Err(RuleNotApplicable),
        Expr::SupsetEq(_, _, _) => Err(RuleNotApplicable),
        Expr::Subset(_, _, _) => Err(RuleNotApplicable),
        Expr::SubsetEq(_, _, _) => Err(RuleNotApplicable),
        Expr::AbstractLiteral(_, _) => Err(RuleNotApplicable),
        Expr::Comprehension(_, _) => Err(RuleNotApplicable),
        Expr::DominanceRelation(_, _) => Err(RuleNotApplicable),
        Expr::FromSolution(_, _) => Err(RuleNotApplicable),
        Expr::Metavar(_, _) => Err(RuleNotApplicable),
        Expr::UnsafeIndex(_, _, _) => Err(RuleNotApplicable),
        Expr::UnsafeSlice(_, _, _) => Err(RuleNotApplicable),
        Expr::SafeIndex(_, subject, indices) => {
            // partially evaluate matrix literals indexed by a constant.

            // subject must be a matrix literal
            let (es, index_domain) = Moo::unwrap_or_clone(subject)
                .unwrap_matrix_unchecked()
                .ok_or(RuleNotApplicable)?;

            // must be indexing a 1d matrix.
            //
            // for n-d matrices, wait for the `remove_dimension_from_matrix_indexing` rule to run
            // first. This reduces n-d indexing operations to 1d.
            if indices.len() != 1 {
                return Err(RuleNotApplicable);
            }

            // the index must be a number
            let index: i32 = (&indices[0]).try_into().map_err(|_| RuleNotApplicable)?;

            // index domain must be a single integer range with a lower bound
            if let Domain::Int(ranges) = index_domain
                && ranges.len() == 1
                && let Some(from) = ranges[0].lower_bound()
            {
                let zero_indexed_index = index - from;
                Ok(Reduction::pure(es[zero_indexed_index as usize].clone()))
            } else {
                Err(RuleNotApplicable)
            }
        }
        Expr::SafeSlice(_, _, _) => Err(RuleNotApplicable),
        Expr::InDomain(_, x, domain) => {
            if let Expr::Atomic(_, Atom::Reference(decl)) = &*x {
                let decl_domain = decl.domain().ok_or(RuleNotApplicable)?.resolve(symtab);
                let domain = domain.resolve(symtab);

                let intersection = decl_domain
                    .intersect(&domain)
                    .map_err(|_| RuleNotApplicable)?;

                // if the declaration's domain is a subset of domain, expr is always true.
                if intersection == decl_domain {
                    Ok(Reduction::pure(Expr::Atomic(Metadata::new(), true.into())))
                }
                // if no elements of declaration's domain are in the domain (i.e. they have no
                // intersection), expr is always false.
                //
                // Only check this when the intersection is a finite integer domain, as we
                // currently don't have a way to check whether other domain kinds are empty or not.
                //
                // we should expand this to cover more domain types in the future.
                else if let Ok(values_in_domain) = intersection.values_i32()
                    && values_in_domain.is_empty()
                {
                    Ok(Reduction::pure(Expr::Atomic(Metadata::new(), false.into())))
                } else {
                    return Err(RuleNotApplicable);
                }
            } else if let Expr::Atomic(_, Atom::Literal(lit)) = &*x {
                if domain
                    .resolve(symtab)
                    .contains(lit)
                    .map_err(|_| RuleNotApplicable)?
                {
                    Ok(Reduction::pure(Expr::Atomic(Metadata::new(), true.into())))
                } else {
                    Ok(Reduction::pure(Expr::Atomic(Metadata::new(), false.into())))
                }
            } else {
                Err(RuleNotApplicable)
            }
        }
        Expr::Bubble(_, expr, cond) => {
            // definition of bubble is "expr is valid as long as cond is true"
            //
            // check if cond is true and pop the bubble!
            if let Expr::Atomic(_, Atom::Literal(Lit::Bool(true))) = *cond {
                Ok(Reduction::pure(Moo::unwrap_or_clone(expr)))
            } else {
                Err(RuleNotApplicable)
            }
        }
        Expr::Atomic(_, _) => Err(RuleNotApplicable),
        Expr::Scope(_, _) => Err(RuleNotApplicable),
        Expr::ToInt(_, expression) => {
            if let Some(ReturnType::Int) = expression.return_type() {
                Ok(Reduction::pure(Moo::unwrap_or_clone(expression)))
            } else {
                Err(RuleNotApplicable)
            }
        }
        Expr::Abs(m, e) => match Moo::unwrap_or_clone(e) {
            Expr::Neg(_, inner) => Ok(Reduction::pure(Expr::Abs(m, inner))),
            _ => Err(RuleNotApplicable),
        },
        Expr::Sum(m, vec) => {
            let vec = Moo::unwrap_or_clone(vec)
                .unwrap_list()
                .ok_or(RuleNotApplicable)?;
            let mut acc = 0;
            let mut n_consts = 0;
            let mut new_vec: Vec<Expr> = Vec::new();
            for expr in vec {
                if let Expr::Atomic(_, Atom::Literal(Lit::Int(x))) = expr {
                    acc += x;
                    n_consts += 1;
                } else {
                    new_vec.push(expr);
                }
            }
            if acc != 0 {
                new_vec.push(Expr::Atomic(
                    Default::default(),
                    Atom::Literal(Lit::Int(acc)),
                ));
            }

            if n_consts <= 1 {
                Err(RuleNotApplicable)
            } else {
                Ok(Reduction::pure(Expr::Sum(
                    m,
                    Moo::new(into_matrix_expr![new_vec]),
                )))
            }
        }

        Expr::Product(m, vec) => {
            let mut acc = 1;
            let mut n_consts = 0;
            let mut new_vec: Vec<Expr> = Vec::new();
            let vec = Moo::unwrap_or_clone(vec)
                .unwrap_list()
                .ok_or(RuleNotApplicable)?;
            for expr in vec {
                if let Expr::Atomic(_, Atom::Literal(Lit::Int(x))) = expr {
                    acc *= x;
                    n_consts += 1;
                } else {
                    new_vec.push(expr);
                }
            }

            if n_consts == 0 {
                return Err(RuleNotApplicable);
            }

            new_vec.push(Expr::Atomic(
                Default::default(),
                Atom::Literal(Lit::Int(acc)),
            ));
            let new_product = Expr::Product(m, Moo::new(into_matrix_expr![new_vec]));

            if acc == 0 {
                // if safe, 0 * exprs ~> 0
                // otherwise, just return 0* exprs
                if new_product.is_safe() {
                    Ok(Reduction::pure(Expr::Atomic(
                        Default::default(),
                        Atom::Literal(Lit::Int(0)),
                    )))
                } else {
                    Ok(Reduction::pure(new_product))
                }
            } else if n_consts == 1 {
                // acc !=0, only one constant
                Err(RuleNotApplicable)
            } else {
                // acc !=0, multiple constants found
                Ok(Reduction::pure(new_product))
            }
        }

        Expr::Min(m, e) => {
            let Some(vec) = Moo::unwrap_or_clone(e).unwrap_list() else {
                return Err(RuleNotApplicable);
            };
            let mut acc: Option<i32> = None;
            let mut n_consts = 0;
            let mut new_vec: Vec<Expr> = Vec::new();
            for expr in vec {
                if let Expr::Atomic(_, Atom::Literal(Lit::Int(x))) = expr {
                    n_consts += 1;
                    acc = match acc {
                        Some(i) => {
                            if i > x {
                                Some(x)
                            } else {
                                Some(i)
                            }
                        }
                        None => Some(x),
                    };
                } else {
                    new_vec.push(expr);
                }
            }

            if let Some(i) = acc {
                new_vec.push(Expr::Atomic(Default::default(), Atom::Literal(Lit::Int(i))));
            }

            if n_consts <= 1 {
                Err(RuleNotApplicable)
            } else {
                Ok(Reduction::pure(Expr::Min(
                    m,
                    Moo::new(into_matrix_expr![new_vec]),
                )))
            }
        }

        Expr::Max(m, e) => {
            let Some(vec) = Moo::unwrap_or_clone(e).unwrap_list() else {
                return Err(RuleNotApplicable);
            };

            let mut acc: Option<i32> = None;
            let mut n_consts = 0;
            let mut new_vec: Vec<Expr> = Vec::new();
            for expr in vec {
                if let Expr::Atomic(_, Atom::Literal(Lit::Int(x))) = expr {
                    n_consts += 1;
                    acc = match acc {
                        Some(i) => {
                            if i < x {
                                Some(x)
                            } else {
                                Some(i)
                            }
                        }
                        None => Some(x),
                    };
                } else {
                    new_vec.push(expr);
                }
            }

            if let Some(i) = acc {
                new_vec.push(Expr::Atomic(Default::default(), Atom::Literal(Lit::Int(i))));
            }

            if n_consts <= 1 {
                Err(RuleNotApplicable)
            } else {
                Ok(Reduction::pure(Expr::Max(
                    m,
                    Moo::new(into_matrix_expr![new_vec]),
                )))
            }
        }
        Expr::Not(_, _) => Err(RuleNotApplicable),
        Expr::Or(m, e) => {
            let Some(terms) = Moo::unwrap_or_clone(e).unwrap_list() else {
                return Err(RuleNotApplicable);
            };

            let mut has_changed = false;

            // 2. boolean literals
            let mut new_terms = vec![];
            for expr in terms {
                if let Expr::Atomic(_, Atom::Literal(Lit::Bool(x))) = expr {
                    has_changed = true;

                    // true ~~> entire or is true
                    // false ~~> remove false from the or
                    if x {
                        return Ok(Reduction::pure(true.into()));
                    }
                } else {
                    new_terms.push(expr);
                }
            }

            // 2. check pairwise tautologies.
            if check_pairwise_or_tautologies(&new_terms) {
                return Ok(Reduction::pure(true.into()));
            }

            // 3. empty or ~~> false
            if new_terms.is_empty() {
                return Ok(Reduction::pure(false.into()));
            }

            if !has_changed {
                return Err(RuleNotApplicable);
            }

            Ok(Reduction::pure(Expr::Or(
                m,
                Moo::new(into_matrix_expr![new_terms]),
            )))
        }
        Expr::And(_, e) => {
            let Some(vec) = Moo::unwrap_or_clone(e).unwrap_list() else {
                return Err(RuleNotApplicable);
            };
            let mut new_vec: Vec<Expr> = Vec::new();
            let mut has_const: bool = false;
            for expr in vec {
                if let Expr::Atomic(_, Atom::Literal(Lit::Bool(x))) = expr {
                    has_const = true;
                    if !x {
                        return Ok(Reduction::pure(Expr::Atomic(
                            Default::default(),
                            Atom::Literal(Lit::Bool(false)),
                        )));
                    }
                } else {
                    new_vec.push(expr);
                }
            }

            if !has_const {
                Err(RuleNotApplicable)
            } else {
                Ok(Reduction::pure(Expr::And(
                    Metadata::new(),
                    Moo::new(into_matrix_expr![new_vec]),
                )))
            }
        }

        // similar to And, but booleans are returned wrapped in Root.
        Expr::Root(_, es) => {
            match es.as_slice() {
                [] => Err(RuleNotApplicable),
                // want to unwrap nested ands
                [Expr::And(_, _)] => Ok(()),
                // root([true]) / root([false]) are already evaluated
                [_] => Err(RuleNotApplicable),
                [_, _, ..] => Ok(()),
            }?;

            let mut new_vec: Vec<Expr> = Vec::new();
            let mut has_changed: bool = false;
            for expr in es {
                match expr {
                    Expr::Atomic(_, Atom::Literal(Lit::Bool(x))) => {
                        has_changed = true;
                        if !x {
                            // false
                            return Ok(Reduction::pure(Expr::Root(
                                Metadata::new(),
                                vec![Expr::Atomic(
                                    Default::default(),
                                    Atom::Literal(Lit::Bool(false)),
                                )],
                            )));
                        }
                        // remove trues
                    }

                    // flatten ands in root
                    Expr::And(_, ref vecs) => match (**vecs).clone().unwrap_list() {
                        Some(mut list) => {
                            has_changed = true;
                            new_vec.append(&mut list);
                        }
                        None => new_vec.push(expr),
                    },
                    _ => new_vec.push(expr),
                }
            }

            if !has_changed {
                Err(RuleNotApplicable)
            } else {
                if new_vec.is_empty() {
                    new_vec.push(true.into());
                }
                Ok(Reduction::pure(Expr::Root(Metadata::new(), new_vec)))
            }
        }
        Expr::Imply(_m, x, y) => {
            if let Expr::Atomic(_, Atom::Literal(Lit::Bool(x))) = *x {
                if x {
                    // (true) -> y ~~> y
                    return Ok(Reduction::pure(Moo::unwrap_or_clone(y)));
                } else {
                    // (false) -> y ~~> true
                    return Ok(Reduction::pure(Expr::Atomic(Metadata::new(), true.into())));
                }
            };

            // reflexivity: p -> p ~> true

            // instead of checking syntactic equivalence of a possibly deep expression,
            // let identical-CSE turn them into identical variables first. Then, check if they are
            // identical variables.

            if x.identical_atom_to(y.as_ref()) {
                return Ok(Reduction::pure(true.into()));
            }

            Err(RuleNotApplicable)
        }
        Expr::Iff(_m, x, y) => {
            if let Expr::Atomic(_, Atom::Literal(Lit::Bool(x))) = *x {
                if x {
                    // (true) <-> y ~~> y
                    return Ok(Reduction::pure(Moo::unwrap_or_clone(y)));
                } else {
                    // (false) <-> y ~~> !y
                    return Ok(Reduction::pure(Expr::Not(Metadata::new(), y)));
                }
            };
            if let Expr::Atomic(_, Atom::Literal(Lit::Bool(y))) = *y {
                if y {
                    // x <-> (true) ~~> x
                    return Ok(Reduction::pure(Moo::unwrap_or_clone(x)));
                } else {
                    // x <-> (false) ~~> !x
                    return Ok(Reduction::pure(Expr::Not(Metadata::new(), x)));
                }
            };

            // reflexivity: p <-> p ~> true

            // instead of checking syntactic equivalence of a possibly deep expression,
            // let identical-CSE turn them into identical variables first. Then, check if they are
            // identical variables.

            if x.identical_atom_to(y.as_ref()) {
                return Ok(Reduction::pure(true.into()));
            }

            Err(RuleNotApplicable)
        }
        Expr::Eq(_, _, _) => Err(RuleNotApplicable),
        Expr::Neq(_, _, _) => Err(RuleNotApplicable),
        Expr::Geq(_, _, _) => Err(RuleNotApplicable),
        Expr::Leq(_, _, _) => Err(RuleNotApplicable),
        Expr::Gt(_, _, _) => Err(RuleNotApplicable),
        Expr::Lt(_, _, _) => Err(RuleNotApplicable),
        Expr::SafeDiv(_, _, _) => Err(RuleNotApplicable),
        Expr::UnsafeDiv(_, _, _) => Err(RuleNotApplicable),
        Expr::AllDiff(m, e) => {
            let Some(vec) = Moo::unwrap_or_clone(e).unwrap_list() else {
                return Err(RuleNotApplicable);
            };

            let mut consts: HashSet<i32> = HashSet::new();

            // check for duplicate constant values which would fail the constraint
            for expr in vec {
                if let Expr::Atomic(_, Atom::Literal(Lit::Int(x))) = expr {
                    if !consts.insert(x) {
                        return Ok(Reduction::pure(Expr::Atomic(
                            m,
                            Atom::Literal(Lit::Bool(false)),
                        )));
                    }
                }
            }

            // nothing has changed
            Err(RuleNotApplicable)
        }
        Expr::Neg(_, _) => Err(RuleNotApplicable),
        Expr::AuxDeclaration(_, _, _) => Err(RuleNotApplicable),
        Expr::UnsafeMod(_, _, _) => Err(RuleNotApplicable),
        Expr::SafeMod(_, _, _) => Err(RuleNotApplicable),
        Expr::UnsafePow(_, _, _) => Err(RuleNotApplicable),
        Expr::SafePow(_, _, _) => Err(RuleNotApplicable),
        Expr::Minus(_, _, _) => Err(RuleNotApplicable),

        // As these are in a low level solver form, I'm assuming that these have already been
        // simplified and partially evaluated.
        Expr::FlatAllDiff(_, _) => Err(RuleNotApplicable),
        Expr::FlatAbsEq(_, _, _) => Err(RuleNotApplicable),
        Expr::FlatIneq(_, _, _, _) => Err(RuleNotApplicable),
        Expr::FlatMinusEq(_, _, _) => Err(RuleNotApplicable),
        Expr::FlatProductEq(_, _, _, _) => Err(RuleNotApplicable),
        Expr::FlatSumLeq(_, _, _) => Err(RuleNotApplicable),
        Expr::FlatSumGeq(_, _, _) => Err(RuleNotApplicable),
        Expr::FlatWatchedLiteral(_, _, _) => Err(RuleNotApplicable),
        Expr::FlatWeightedSumLeq(_, _, _, _) => Err(RuleNotApplicable),
        Expr::FlatWeightedSumGeq(_, _, _, _) => Err(RuleNotApplicable),
        Expr::MinionDivEqUndefZero(_, _, _, _) => Err(RuleNotApplicable),
        Expr::MinionModuloEqUndefZero(_, _, _, _) => Err(RuleNotApplicable),
        Expr::MinionPow(_, _, _, _) => Err(RuleNotApplicable),
        Expr::MinionReify(_, _, _) => Err(RuleNotApplicable),
        Expr::MinionReifyImply(_, _, _) => Err(RuleNotApplicable),
        Expr::MinionWInIntervalSet(_, _, _) => Err(RuleNotApplicable),
        Expr::MinionWInSet(_, _, _) => Err(RuleNotApplicable),
        Expr::MinionElementOne(_, _, _, _) => Err(RuleNotApplicable),
    }
}

/// Checks for tautologies involving pairs of terms inside an or, returning true if one is found.
///
/// This applies the following rules:
///
/// ```text
/// (p->q) \/ (q->p) ~> true    [totality of implication]
/// (p->q) \/ (p-> !q) ~> true  [conditional excluded middle]
/// ```
///
fn check_pairwise_or_tautologies(or_terms: &[Expr]) -> bool {
    // Collect terms that are structurally identical to the rule input.
    // Then, try the rules on these terms, also checking the other conditions of the rules.

    // stores (p,q) in p -> q
    let mut p_implies_q: Vec<(&Expr, &Expr)> = vec![];

    // stores (p,q) in p -> !q
    let mut p_implies_not_q: Vec<(&Expr, &Expr)> = vec![];

    for term in or_terms.iter() {
        if let Expr::Imply(_, p, q) = term {
            // we use identical_atom_to for equality later on, so these sets are mutually exclusive.
            //
            // in general however, p -> !q would be in p_implies_q as (p,!q)
            if let Expr::Not(_, q_1) = q.as_ref() {
                p_implies_not_q.push((p.as_ref(), q_1.as_ref()));
            } else {
                p_implies_q.push((p.as_ref(), q.as_ref()));
            }
        }
    }

    // `(p->q) \/ (q->p) ~> true    [totality of implication]`
    for ((p1, q1), (q2, p2)) in iproduct!(p_implies_q.iter(), p_implies_q.iter()) {
        if p1.identical_atom_to(p2) && q1.identical_atom_to(q2) {
            return true;
        }
    }

    // `(p->q) \/ (p-> !q) ~> true`    [conditional excluded middle]
    for ((p1, q1), (p2, q2)) in iproduct!(p_implies_q.iter(), p_implies_not_q.iter()) {
        if p1.identical_atom_to(p2) && q1.identical_atom_to(q2) {
            return true;
        }
    }

    false
}
