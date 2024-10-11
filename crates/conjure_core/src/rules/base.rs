use conjure_core::ast::{
    Constant as Const, DecisionVariable, Domain, Expression as Expr, Range, SymbolTable,
};
use conjure_core::metadata::Metadata;
use conjure_core::rule_engine::{
    register_rule, register_rule_set, ApplicationError, ApplicationResult, Reduction,
};
use conjure_core::Model;
use uniplate::Uniplate;

/*****************************************************************************/
/*        This file contains basic rules for simplifying expressions         */
/*****************************************************************************/

register_rule_set!("Base", 150, ());

/**
 * Remove nothing's from expressions:
 * ```text
 * and([a, nothing, b]) = and([a, b])
 * sum([a, nothing, b]) = sum([a, b])
 * sum_leq([a, nothing, b], c) = sum_leq([a, b], c)
 * ...
 * ```
*/
#[register_rule(("Base", 100))]
fn remove_nothings(expr: &Expr, _: &Model) -> ApplicationResult {
    fn remove_nothings(exprs: Vec<Expr>) -> Result<Vec<Expr>, ApplicationError> {
        let mut changed = false;
        let mut new_exprs = Vec::new();

        for e in exprs {
            match e.clone() {
                Expr::Nothing => {
                    changed = true;
                }
                _ => new_exprs.push(e),
            }
        }

        if changed {
            Ok(new_exprs)
        } else {
            Err(ApplicationError::RuleNotApplicable)
        }
    }

    fn get_lhs_rhs(sub: Vec<Expr>) -> (Vec<Expr>, Box<Expr>) {
        if sub.is_empty() {
            return (Vec::new(), Box::new(Expr::Nothing));
        }

        let lhs = sub[..(sub.len() - 1)].to_vec();
        let rhs = Box::new(sub[sub.len() - 1].clone());
        (lhs, rhs)
    }

    // FIXME (niklasdewally): temporary conversion until I get the Uniplate APIs figured out
    // Uniplate *should* support Vec<> not im::Vector
    let new_sub = remove_nothings(expr.children().into_iter().collect())?;

    match expr {
        Expr::And(md, _) => Ok(Reduction::pure(Expr::And(md.clone(), new_sub))),
        Expr::Or(md, _) => Ok(Reduction::pure(Expr::Or(md.clone(), new_sub))),
        Expr::Sum(md, _) => Ok(Reduction::pure(Expr::Sum(md.clone(), new_sub))),
        Expr::SumEq(md, _, _) => {
            let (lhs, rhs) = get_lhs_rhs(new_sub);
            Ok(Reduction::pure(Expr::SumEq(md.clone(), lhs, rhs)))
        }
        Expr::SumLeq(md, _lhs, _rhs) => {
            let (lhs, rhs) = get_lhs_rhs(new_sub);
            Ok(Reduction::pure(Expr::SumLeq(md.clone(), lhs, rhs)))
        }
        Expr::SumGeq(md, _lhs, _rhs) => {
            let (lhs, rhs) = get_lhs_rhs(new_sub);
            Ok(Reduction::pure(Expr::SumGeq(md.clone(), lhs, rhs)))
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
 * Remove empty expressions:
 * ```text
 * [] = Nothing
 * ```
 */
#[register_rule(("Base", 100))]
fn empty_to_nothing(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Expr::Nothing | Expr::Reference(_, _) | Expr::Constant(_, _) => {
            Err(ApplicationError::RuleNotApplicable)
        }
        _ => {
            if expr.children().is_empty() {
                Ok(Reduction::pure(Expr::Nothing))
            } else {
                Err(ApplicationError::RuleNotApplicable)
            }
        }
    }
}

/**
 * Evaluate sum of constants:
 * ```text
 * sum([1, 2, 3]) = 6
 * ```
 */
#[register_rule(("Base", 100))]
fn sum_constants(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Expr::Sum(_, exprs) => {
            let mut sum = 0;
            let mut new_exprs = Vec::new();
            let mut changed = false;
            for e in exprs {
                match e {
                    Expr::Constant(_metadata, Const::Int(i)) => {
                        sum += i;
                        changed = true;
                    }
                    _ => new_exprs.push(e.clone()),
                }
            }
            if !changed {
                return Err(ApplicationError::RuleNotApplicable);
            }
            // TODO (kf77): Get existing metadata instead of creating a new one
            new_exprs.push(Expr::Constant(Metadata::new(), Const::Int(sum)));
            Ok(Reduction::pure(Expr::Sum(Metadata::new(), new_exprs))) // Let other rules handle only one Expr being contained in the sum
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
 * Unwrap trivial sums:
 * ```text
 * sum([a]) = a
 * ```
 */
#[register_rule(("Base", 100))]
fn unwrap_sum(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Expr::Sum(_, exprs) if (exprs.len() == 1) => Ok(Reduction::pure(exprs[0].clone())),
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
 * Flatten nested sums:
 * ```text
 * sum(sum(a, b), c) = sum(a, b, c)
 * ```
 */
#[register_rule(("Base", 100))]
pub fn flatten_nested_sum(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Expr::Sum(metadata, exprs) => {
            let mut new_exprs = Vec::new();
            let mut changed = false;
            for e in exprs {
                match e {
                    Expr::Sum(_, sub_exprs) => {
                        changed = true;
                        for e in sub_exprs {
                            new_exprs.push(e.clone());
                        }
                    }
                    _ => new_exprs.push(e.clone()),
                }
            }
            if !changed {
                return Err(ApplicationError::RuleNotApplicable);
            }
            Ok(Reduction::pure(Expr::Sum(
                metadata.clone_dirty(),
                new_exprs,
            )))
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
* Unwrap nested `or`

* ```text
* or(or(a, b), c) = or(a, b, c)
* ```
 */
#[register_rule(("Base", 100))]
fn unwrap_nested_or(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Expr::Or(metadata, exprs) => {
            let mut new_exprs = Vec::new();
            let mut changed = false;
            for e in exprs {
                match e {
                    Expr::Or(_, exprs) => {
                        changed = true;
                        for e in exprs {
                            new_exprs.push(e.clone());
                        }
                    }
                    _ => new_exprs.push(e.clone()),
                }
            }
            if !changed {
                return Err(ApplicationError::RuleNotApplicable);
            }
            Ok(Reduction::pure(Expr::Or(metadata.clone_dirty(), new_exprs)))
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
* Unwrap nested `and`

* ```text
* and(and(a, b), c) = and(a, b, c)
* ```
 */
#[register_rule(("Base", 100))]
fn unwrap_nested_and(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Expr::And(metadata, exprs) => {
            let mut new_exprs = Vec::new();
            let mut changed = false;
            for e in exprs {
                match e {
                    Expr::And(_, exprs) => {
                        changed = true;
                        for e in exprs {
                            new_exprs.push(e.clone());
                        }
                    }
                    _ => new_exprs.push(e.clone()),
                }
            }
            if !changed {
                return Err(ApplicationError::RuleNotApplicable);
            }
            Ok(Reduction::pure(Expr::And(
                metadata.clone_dirty(),
                new_exprs,
            )))
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
* Remove double negation:

* ```text
* not(not(a)) = a
* ```
 */
#[register_rule(("Base", 100))]
fn remove_double_negation(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Expr::Not(_, contents) => match contents.as_ref() {
            Expr::Not(_, expr_box) => Ok(Reduction::pure(*expr_box.clone())),
            _ => Err(ApplicationError::RuleNotApplicable),
        },
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
 * Remove trivial `and` (only one element):
 * ```text
 * and([a]) = a
 * ```
 */
#[register_rule(("Base", 100))]
fn remove_trivial_and(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Expr::And(_, exprs) => {
            if exprs.len() == 1 {
                return Ok(Reduction::pure(exprs[0].clone()));
            }
            Err(ApplicationError::RuleNotApplicable)
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
 * Remove trivial `or` (only one element):
 * ```text
 * or([a]) = a
 * ```
 */
#[register_rule(("Base", 100))]
fn remove_trivial_or(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Expr::Or(_, exprs) => {
            if exprs.len() == 1 {
                return Ok(Reduction::pure(exprs[0].clone()));
            }
            Err(ApplicationError::RuleNotApplicable)
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
 * Remove constant bools from or expressions
 * ```text
 * or([true, a]) = true
 * or([false, a]) = a
 * ```
 */
#[register_rule(("Base", 100))]
fn remove_constants_from_or(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Expr::Or(metadata, exprs) => {
            let mut new_exprs = Vec::new();
            let mut changed = false;
            for e in exprs {
                match e {
                    Expr::Constant(metadata, Const::Bool(val)) => {
                        if *val {
                            // If we find a true, the whole expression is true
                            return Ok(Reduction::pure(Expr::Constant(
                                metadata.clone_dirty(),
                                Const::Bool(true),
                            )));
                        } else {
                            // If we find a false, we can ignore it
                            changed = true;
                        }
                    }
                    _ => new_exprs.push(e.clone()),
                }
            }
            if !changed {
                return Err(ApplicationError::RuleNotApplicable);
            }
            Ok(Reduction::pure(Expr::Or(metadata.clone_dirty(), new_exprs)))
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
 * Remove constant bools from and expressions
 * ```text
 * and([true, a]) = a
 * and([false, a]) = false
 * ```
 */
#[register_rule(("Base", 100))]
fn remove_constants_from_and(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Expr::And(metadata, exprs) => {
            let mut new_exprs = Vec::new();
            let mut changed = false;
            for e in exprs {
                match e {
                    Expr::Constant(metadata, Const::Bool(val)) => {
                        if !*val {
                            // If we find a false, the whole expression is false
                            return Ok(Reduction::pure(Expr::Constant(
                                metadata.clone_dirty(),
                                Const::Bool(false),
                            )));
                        } else {
                            // If we find a true, we can ignore it
                            changed = true;
                        }
                    }
                    _ => new_exprs.push(e.clone()),
                }
            }
            if !changed {
                return Err(ApplicationError::RuleNotApplicable);
            }
            Ok(Reduction::pure(Expr::And(
                metadata.clone_dirty(),
                new_exprs,
            )))
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
 * Evaluate Not expressions with constant bools
 * ```text
 * not(true) = false
 * not(false) = true
 * ```
 */
#[register_rule(("Base", 100))]
fn evaluate_constant_not(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Expr::Not(_, contents) => match contents.as_ref() {
            Expr::Constant(metadata, Const::Bool(val)) => Ok(Reduction::pure(Expr::Constant(
                metadata.clone_dirty(),
                Const::Bool(!val),
            ))),
            _ => Err(ApplicationError::RuleNotApplicable),
        },
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
 * Turn a Min into a new variable and post a global constraint to ensure the new variable is the minimum.
 * ```text
 * min([a, b]) ~> c ; c <= a & c <= b & (c = a | c = b)
 * ```
 */
#[register_rule(("Base", 100))]
fn min_to_var(expr: &Expr, mdl: &Model) -> ApplicationResult {
    match expr {
        Expr::Min(metadata, exprs) => {
            let new_name = mdl.gensym();

            let mut new_top = Vec::new(); // the new variable must be less than or equal to all the other variables
            let mut disjunction = Vec::new(); // the new variable must be equal to one of the variables
            for e in exprs {
                new_top.push(Expr::Leq(
                    Metadata::new(),
                    Box::new(Expr::Reference(Metadata::new(), new_name.clone())),
                    Box::new(e.clone()),
                ));
                disjunction.push(Expr::Eq(
                    Metadata::new(),
                    Box::new(Expr::Reference(Metadata::new(), new_name.clone())),
                    Box::new(e.clone()),
                ));
            }
            new_top.push(Expr::Or(Metadata::new(), disjunction));

            let mut new_vars = SymbolTable::new();
            let domain = expr
                .domain_of(&mdl.variables)
                .ok_or(ApplicationError::DomainError)?;
            new_vars.insert(new_name.clone(), DecisionVariable::new(domain));

            Ok(Reduction::new(
                Expr::Reference(Metadata::new(), new_name),
                Expr::And(metadata.clone_dirty(), new_top),
                new_vars,
            ))
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
 * Turn a Max into a new variable and post a global constraint to ensure the new variable is the maximum.
 * ```text
 * max([a, b]) ~> c ; c >= a & c >= b & (c = a | c = b)
 * ```
 */
#[register_rule(("Base", 100))]
fn max_to_var(expr: &Expr, mdl: &Model) -> ApplicationResult {
    match expr {
        Expr::Max(metadata, exprs) => {
            let new_name = mdl.gensym();

            let mut new_top = Vec::new(); // the new variable must be greater than or equal to all the other variables
            let mut disjunction = Vec::new(); // the new variable must be equal to one of the variables
            for e in exprs {
                new_top.push(Expr::Geq(
                    Metadata::new(),
                    Box::new(Expr::Reference(Metadata::new(), new_name.clone())),
                    Box::new(e.clone()),
                ));
                disjunction.push(Expr::Eq(
                    Metadata::new(),
                    Box::new(Expr::Reference(Metadata::new(), new_name.clone())),
                    Box::new(e.clone()),
                ));
            }
            new_top.push(Expr::Or(Metadata::new(), disjunction));

            let mut new_vars = SymbolTable::new();
            let domain = expr
                .domain_of(&mdl.variables)
                .ok_or(ApplicationError::DomainError)?;
            new_vars.insert(new_name.clone(), DecisionVariable::new(domain));

            Ok(Reduction::new(
                Expr::Reference(Metadata::new(), new_name),
                Expr::And(metadata.clone_dirty(), new_top),
                new_vars,
            ))
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
* Apply the Distributive Law to expressions like `Or([..., And(a, b)])`

* ```text
* or(and(a, b), c) = and(or(a, c), or(b, c))
* ```
 */
#[register_rule(("Base", 100))]
fn distribute_or_over_and(expr: &Expr, _: &Model) -> ApplicationResult {
    fn find_and(exprs: &[Expr]) -> Option<usize> {
        // ToDo: may be better to move this to some kind of utils module?
        for (i, e) in exprs.iter().enumerate() {
            if let Expr::And(_, _) = e {
                return Some(i);
            }
        }
        None
    }

    match expr {
        Expr::Or(_, exprs) => match find_and(exprs) {
            Some(idx) => {
                let mut rest = exprs.clone();
                let and_expr = rest.remove(idx);

                match and_expr {
                    Expr::And(metadata, and_exprs) => {
                        let mut new_and_contents = Vec::new();

                        for e in and_exprs {
                            // ToDo: Cloning everything may be a bit inefficient - discuss
                            let mut new_or_contents = rest.clone();
                            new_or_contents.push(e.clone());
                            new_and_contents.push(Expr::Or(metadata.clone_dirty(), new_or_contents))
                        }

                        Ok(Reduction::pure(Expr::And(
                            metadata.clone_dirty(),
                            new_and_contents,
                        )))
                    }
                    _ => Err(ApplicationError::RuleNotApplicable),
                }
            }
            None => Err(ApplicationError::RuleNotApplicable),
        },
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
* Distribute `not` over `and` (De Morgan's Law):

* ```text
* not(and(a, b)) = or(not a, not b)
* ```
 */
#[register_rule(("Base", 100))]
fn distribute_not_over_and(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Expr::Not(_, contents) => match contents.as_ref() {
            Expr::And(metadata, exprs) => {
                if exprs.len() == 1 {
                    let single_expr = exprs[0].clone();
                    return Ok(Reduction::pure(Expr::Not(
                        Metadata::new(),
                        Box::new(single_expr.clone()),
                    )));
                }
                let mut new_exprs = Vec::new();
                for e in exprs {
                    new_exprs.push(Expr::Not(metadata.clone(), Box::new(e.clone())));
                }
                Ok(Reduction::pure(Expr::Or(metadata.clone(), new_exprs)))
            }
            _ => Err(ApplicationError::RuleNotApplicable),
        },
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
* Distribute `not` over `or` (De Morgan's Law):

* ```text
* not(or(a, b)) = and(not a, not b)
* ```
 */
#[register_rule(("Base", 100))]
fn distribute_not_over_or(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Expr::Not(_, contents) => match contents.as_ref() {
            Expr::Or(metadata, exprs) => {
                if exprs.len() == 1 {
                    let single_expr = exprs[0].clone();
                    return Ok(Reduction::pure(Expr::Not(
                        Metadata::new(),
                        Box::new(single_expr.clone()),
                    )));
                }
                let mut new_exprs = Vec::new();
                for e in exprs {
                    new_exprs.push(Expr::Not(metadata.clone(), Box::new(e.clone())));
                }
                Ok(Reduction::pure(Expr::And(metadata.clone(), new_exprs)))
            }
            _ => Err(ApplicationError::RuleNotApplicable),
        },
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}
