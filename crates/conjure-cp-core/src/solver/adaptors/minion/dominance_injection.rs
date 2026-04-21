use std::collections::{BTreeMap, HashMap, HashSet};
use std::env;
use std::fs::OpenOptions;
use std::io::Write;

use minion_sys::ast as minion_ast;
use minion_sys::error::{MinionError, RuntimeError};
use minion_sys::{add_aux_var_during_search, add_constraint_during_search};
use uniplate::Uniplate;

use crate::ast::{Atom, Expression, GroundDomain, Literal, Metadata, Moo, Name};
use crate::rule_engine::{get_rule_sets_for_solver_family, rewrite_model_with_configured_rewriter};
use crate::settings::{current_rewriter, SolverFamily};
use crate::solver::SolverError;
use crate::solver::SolverError::{Runtime, RuntimeNotImplemented};
use crate::Model as ConjureModel;

use super::parse_model::model_to_minion;

fn sub_in_solution_into_current_refs(
    expr: &Expression,
    solution: &HashMap<Name, Literal>,
) -> Option<Expression> {
    match expr {
        Expression::Atomic(_, Atom::Reference(reference)) => {
            let var_name = reference.name();
            let value = solution.get(&var_name)?;
            let value = if let Some(domain) = reference.resolved_domain() {
                if domain.as_ref() == &GroundDomain::Bool {
                    match value {
                        Literal::Bool(x) => Literal::Bool(*x),
                        Literal::Int(1) => Literal::Bool(true),
                        Literal::Int(0) => Literal::Bool(false),
                        _ => return None,
                    }
                } else {
                    value.clone()
                }
            } else {
                value.clone()
            };

            Some(Expression::Atomic(Metadata::new(), Atom::Literal(value)))
        }
        _ => Some(expr.clone()),
    }
}

fn swap_from_solution_to_current_ref(expr: &Expression) -> Option<Expression> {
    match expr {
        Expression::FromSolution(_, atom_expr) => Some(Expression::Atomic(
            Metadata::new(),
            atom_expr.as_ref().clone(),
        )),
        _ => Some(expr.clone()),
    }
}

pub(super) fn add_represented_decision_values(
    solution: &mut HashMap<Name, Literal>,
    model: &ConjureModel,
) {
    let symbols = model.symbols().clone();
    let mut representations = Vec::new();

    for (name, _) in symbols.clone().into_iter() {
        let Some(reprs) = symbols.representations_for(&name) else {
            continue;
        };
        if reprs.is_empty() {
            continue;
        }
        if reprs.len() > 1 || reprs[0].len() != 1 {
            continue;
        }
        representations.push((name, reprs[0][0].clone()));
    }

    if representations.is_empty() {
        return;
    }

    let mut solution_btree = solution
        .clone()
        .into_iter()
        .collect::<BTreeMap<Name, Literal>>();
    for (name, representation) in representations {
        let Ok(value) = representation.value_up(&solution_btree) else {
            continue;
        };
        solution.insert(name.clone(), value.clone());
        solution_btree.insert(name, value);
    }
}

pub(super) fn minion_error_to_solver_error(err: MinionError) -> SolverError {
    match err {
        MinionError::RuntimeError(x) => Runtime(format!("{x:#?}")),
        MinionError::Other(x) => Runtime(format!("{x:#?}")),
        MinionError::NotImplemented(x) => RuntimeNotImplemented(x),
        x => Runtime(format!("unknown minion_sys error: {x:#?}")),
    }
}

fn remap_var_names_in_var(
    var: &minion_ast::Var,
    remap: &HashMap<minion_ast::VarName, minion_ast::VarName>,
) -> minion_ast::Var {
    match var {
        minion_ast::Var::NameRef(name) => {
            minion_ast::Var::NameRef(remap.get(name).cloned().unwrap_or_else(|| name.clone()))
        }
        minion_ast::Var::ConstantAsVar(x) => minion_ast::Var::ConstantAsVar(*x),
    }
}

fn remap_var_names_in_constraint(
    constraint: minion_ast::Constraint,
    remap: &HashMap<minion_ast::VarName, minion_ast::VarName>,
) -> minion_ast::Constraint {
    use minion_ast::Constraint as C;
    match constraint {
        C::Difference((a, b), c) => C::Difference(
            (
                remap_var_names_in_var(&a, remap),
                remap_var_names_in_var(&b, remap),
            ),
            remap_var_names_in_var(&c, remap),
        ),
        C::Div((a, b), c) => C::Div(
            (
                remap_var_names_in_var(&a, remap),
                remap_var_names_in_var(&b, remap),
            ),
            remap_var_names_in_var(&c, remap),
        ),
        C::DivUndefZero((a, b), c) => C::DivUndefZero(
            (
                remap_var_names_in_var(&a, remap),
                remap_var_names_in_var(&b, remap),
            ),
            remap_var_names_in_var(&c, remap),
        ),
        C::Modulo((a, b), c) => C::Modulo(
            (
                remap_var_names_in_var(&a, remap),
                remap_var_names_in_var(&b, remap),
            ),
            remap_var_names_in_var(&c, remap),
        ),
        C::ModuloUndefZero((a, b), c) => C::ModuloUndefZero(
            (
                remap_var_names_in_var(&a, remap),
                remap_var_names_in_var(&b, remap),
            ),
            remap_var_names_in_var(&c, remap),
        ),
        C::Pow((a, b), c) => C::Pow(
            (
                remap_var_names_in_var(&a, remap),
                remap_var_names_in_var(&b, remap),
            ),
            remap_var_names_in_var(&c, remap),
        ),
        C::Product((a, b), c) => C::Product(
            (
                remap_var_names_in_var(&a, remap),
                remap_var_names_in_var(&b, remap),
            ),
            remap_var_names_in_var(&c, remap),
        ),
        C::WeightedSumGeq(cs, vars, out) => C::WeightedSumGeq(
            cs,
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            remap_var_names_in_var(&out, remap),
        ),
        C::WeightedSumLeq(cs, vars, out) => C::WeightedSumLeq(
            cs,
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            remap_var_names_in_var(&out, remap),
        ),
        C::CheckAssign(inner) => {
            C::CheckAssign(Box::new(remap_var_names_in_constraint(*inner, remap)))
        }
        C::CheckGsa(inner) => C::CheckGsa(Box::new(remap_var_names_in_constraint(*inner, remap))),
        C::ForwardChecking(inner) => {
            C::ForwardChecking(Box::new(remap_var_names_in_constraint(*inner, remap)))
        }
        C::Reify(inner, var) => C::Reify(
            Box::new(remap_var_names_in_constraint(*inner, remap)),
            remap_var_names_in_var(&var, remap),
        ),
        C::ReifyImply(inner, var) => C::ReifyImply(
            Box::new(remap_var_names_in_constraint(*inner, remap)),
            remap_var_names_in_var(&var, remap),
        ),
        C::ReifyImplyQuick(inner, var) => C::ReifyImplyQuick(
            Box::new(remap_var_names_in_constraint(*inner, remap)),
            remap_var_names_in_var(&var, remap),
        ),
        C::WatchedAnd(inners) => C::WatchedAnd(
            inners
                .into_iter()
                .map(|c| remap_var_names_in_constraint(c, remap))
                .collect(),
        ),
        C::WatchedOr(inners) => C::WatchedOr(
            inners
                .into_iter()
                .map(|c| remap_var_names_in_constraint(c, remap))
                .collect(),
        ),
        C::GacAllDiff(vars) => C::GacAllDiff(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
        ),
        C::AllDiff(vars) => C::AllDiff(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
        ),
        C::AllDiffMatrix(vars, k) => C::AllDiffMatrix(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            k,
        ),
        C::WatchSumGeq(vars, k) => C::WatchSumGeq(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            k,
        ),
        C::WatchSumLeq(vars, k) => C::WatchSumLeq(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            k,
        ),
        C::OccurrenceGeq(vars, a, b) => C::OccurrenceGeq(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            a,
            b,
        ),
        C::OccurrenceLeq(vars, a, b) => C::OccurrenceLeq(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            a,
            b,
        ),
        C::Occurrence(vars, a, out) => C::Occurrence(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            a,
            remap_var_names_in_var(&out, remap),
        ),
        C::LitSumGeq(vars, cs, k) => C::LitSumGeq(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            cs,
            k,
        ),
        C::Gcc(vars, cs, counts) => C::Gcc(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            cs,
            counts
                .into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
        ),
        C::GccWeak(vars, cs, counts) => C::GccWeak(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            cs,
            counts
                .into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
        ),
        C::LexLeqRv(a, b) => C::LexLeqRv(
            a.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            b.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
        ),
        C::LexLeq(a, b) => C::LexLeq(
            a.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            b.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
        ),
        C::LexLess(a, b) => C::LexLess(
            a.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            b.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
        ),
        C::LexLeqQuick(a, b) => C::LexLeqQuick(
            a.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            b.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
        ),
        C::LexLessQuick(a, b) => C::LexLessQuick(
            a.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            b.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
        ),
        C::WatchVecNeq(a, b) => C::WatchVecNeq(
            a.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            b.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
        ),
        C::WatchVecExistsLess(a, b) => C::WatchVecExistsLess(
            a.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            b.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
        ),
        C::Hamming(a, b, k) => C::Hamming(
            a.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            b.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            k,
        ),
        C::NotHamming(a, b, k) => C::NotHamming(
            a.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            b.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            k,
        ),
        C::FrameUpdate(a, b, c, d, k) => C::FrameUpdate(
            a.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            b.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            c.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            d.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            k,
        ),
        C::NegativeTable(vars, tuples) => C::NegativeTable(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            tuples,
        ),
        C::Table(vars, tuples) => C::Table(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            tuples,
        ),
        C::GacSchema(vars, tuples) => C::GacSchema(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            tuples,
        ),
        C::LightTable(vars, tuples) => C::LightTable(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            tuples,
        ),
        C::Mddc(vars, tuples) => C::Mddc(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            tuples,
        ),
        C::NegativeMddc(vars, tuples) => C::NegativeMddc(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            tuples,
        ),
        C::Str2Plus(vars, out) => C::Str2Plus(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            remap_var_names_in_var(&out, remap),
        ),
        C::Max(vars, out) => C::Max(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            remap_var_names_in_var(&out, remap),
        ),
        C::Min(vars, out) => C::Min(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            remap_var_names_in_var(&out, remap),
        ),
        C::NvalueGeq(vars, out) => C::NvalueGeq(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            remap_var_names_in_var(&out, remap),
        ),
        C::NvalueLeq(vars, out) => C::NvalueLeq(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            remap_var_names_in_var(&out, remap),
        ),
        C::SumLeq(vars, out) => C::SumLeq(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            remap_var_names_in_var(&out, remap),
        ),
        C::SumGeq(vars, out) => C::SumGeq(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            remap_var_names_in_var(&out, remap),
        ),
        C::Element(vars, a, b) => C::Element(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            remap_var_names_in_var(&a, remap),
            remap_var_names_in_var(&b, remap),
        ),
        C::ElementOne(vars, a, b) => C::ElementOne(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            remap_var_names_in_var(&a, remap),
            remap_var_names_in_var(&b, remap),
        ),
        C::ElementUndefZero(vars, a, b) => C::ElementUndefZero(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            remap_var_names_in_var(&a, remap),
            remap_var_names_in_var(&b, remap),
        ),
        C::WatchElement(vars, a, b) => C::WatchElement(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            remap_var_names_in_var(&a, remap),
            remap_var_names_in_var(&b, remap),
        ),
        C::WatchElementOne(vars, a, b) => C::WatchElementOne(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            remap_var_names_in_var(&a, remap),
            remap_var_names_in_var(&b, remap),
        ),
        C::WatchElementOneUndefZero(vars, a, b) => C::WatchElementOneUndefZero(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            remap_var_names_in_var(&a, remap),
            remap_var_names_in_var(&b, remap),
        ),
        C::WatchElementUndefZero(vars, a, b) => C::WatchElementUndefZero(
            vars.into_iter()
                .map(|v| remap_var_names_in_var(&v, remap))
                .collect(),
            remap_var_names_in_var(&a, remap),
            remap_var_names_in_var(&b, remap),
        ),
        C::WLiteral(v, k) => C::WLiteral(remap_var_names_in_var(&v, remap), k),
        C::WNotLiteral(v, k) => C::WNotLiteral(remap_var_names_in_var(&v, remap), k),
        C::WInIntervalSet(v, cs) => C::WInIntervalSet(remap_var_names_in_var(&v, remap), cs),
        C::WInRange(v, cs) => C::WInRange(remap_var_names_in_var(&v, remap), cs),
        C::WInset(v, cs) => C::WInset(remap_var_names_in_var(&v, remap), cs),
        C::WNotInRange(v, cs) => C::WNotInRange(remap_var_names_in_var(&v, remap), cs),
        C::WNotInset(v, cs) => C::WNotInset(remap_var_names_in_var(&v, remap), cs),
        C::Abs(a, b) => C::Abs(
            remap_var_names_in_var(&a, remap),
            remap_var_names_in_var(&b, remap),
        ),
        C::DisEq(a, b) => C::DisEq(
            remap_var_names_in_var(&a, remap),
            remap_var_names_in_var(&b, remap),
        ),
        C::Eq(a, b) => C::Eq(
            remap_var_names_in_var(&a, remap),
            remap_var_names_in_var(&b, remap),
        ),
        C::MinusEq(a, b) => C::MinusEq(
            remap_var_names_in_var(&a, remap),
            remap_var_names_in_var(&b, remap),
        ),
        C::GacEq(a, b) => C::GacEq(
            remap_var_names_in_var(&a, remap),
            remap_var_names_in_var(&b, remap),
        ),
        C::WatchLess(a, b) => C::WatchLess(
            remap_var_names_in_var(&a, remap),
            remap_var_names_in_var(&b, remap),
        ),
        C::WatchNeq(a, b) => C::WatchNeq(
            remap_var_names_in_var(&a, remap),
            remap_var_names_in_var(&b, remap),
        ),
        C::Ineq(a, b, c) => C::Ineq(
            remap_var_names_in_var(&a, remap),
            remap_var_names_in_var(&b, remap),
            c,
        ),
        C::False => C::False,
        C::True => C::True,
        _ => constraint,
    }
}

fn minion_injection_log_path() -> Option<String> {
    env::var("CONJURE_MINION_INJECTION_LOG")
        .ok()
        .filter(|p| !p.trim().is_empty())
}

fn append_minion_injection_log(line: &str) {
    let Some(path) = minion_injection_log_path() else {
        return;
    };

    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    let _ = writeln!(file, "{line}");
}

fn dump_minion_model(model: &minion_ast::Model) -> String {
    let mut buf = Vec::<u8>::new();
    if minion_sys::print::write_minion_file(&mut buf, model).is_ok() {
        String::from_utf8_lossy(&buf).into_owned()
    } else {
        format!("{model:#?}")
    }
}

fn rewrite_dominance_to_block_dominated_futures(
    dominance_expression: &Expression,
    solution: &HashMap<Name, Literal>,
) -> Expression {
    Expression::Not(
        Metadata::new(),
        Moo::new(
            dominance_expression
                .rewrite(&|e| sub_in_solution_into_current_refs(&e, solution))
                .rewrite(&|e| swap_from_solution_to_current_ref(&e)),
        ),
    )
}

pub(super) fn add_dominance_constraints_for_solution(
    dominance_expression: Option<&Expression>,
    dominance_model_template: Option<&ConjureModel>,
    solution: &HashMap<Name, Literal>,
    known_var_names: &mut HashSet<minion_ast::VarName>,
    next_midsearch_aux_var_id: &mut usize,
    solution_ordinal: usize,
) -> Result<(), SolverError> {
    let Some(dominance_expression) = dominance_expression else {
        return Ok(());
    };

    let Some(model_template) = dominance_model_template else {
        return Ok(());
    };

    let rewritten_dominance =
        rewrite_dominance_to_block_dominated_futures(dominance_expression, solution);

    let mut solution_pairs = solution
        .iter()
        .map(|(k, v)| (k.to_string(), format!("{v:?}")))
        .collect::<Vec<_>>();
    solution_pairs.sort_by(|a, b| a.0.cmp(&b.0));
    append_minion_injection_log(&format!(
        "[minion-inject] BEGIN solution#{solution_ordinal}; solution={solution_pairs:?}"
    ));
    append_minion_injection_log(&format!(
        "[minion-inject] rewritten_dominance(solution#{solution_ordinal}) = {rewritten_dominance:#?}"
    ));

    let mut dominance_model = model_template.clone();
    dominance_model.replace_constraints(vec![]);
    dominance_model.replace_clauses(vec![]);
    dominance_model.dominance = None;
    dominance_model.add_constraint(rewritten_dominance);

    let rewritten = rewrite_model_with_configured_rewriter(
        dominance_model,
        &get_rule_sets_for_solver_family(SolverFamily::Minion),
        current_rewriter(),
    )
    .map_err(|e| {
        Runtime(format!(
            "failed to rewrite dominance constraint for Minion injection: {e}"
        ))
    })?;

    let dominance_minion_model = model_to_minion(rewritten)?;
    append_minion_injection_log(&format!(
        "[minion-inject] minion_model(solution#{solution_ordinal}) START"
    ));
    append_minion_injection_log(&dump_minion_model(&dominance_minion_model));
    append_minion_injection_log(&format!(
        "[minion-inject] minion_model(solution#{solution_ordinal}) END"
    ));
    let search_var_names = dominance_minion_model
        .named_variables
        .get_search_variable_order()
        .into_iter()
        .collect::<HashSet<_>>();

    let mut remap = HashMap::<minion_ast::VarName, minion_ast::VarName>::new();

    for var_name in dominance_minion_model.named_variables.get_variable_order() {
        if search_var_names.contains(&var_name) {
            if !known_var_names.contains(&var_name) {
                return Err(Runtime(format!(
                    "dominance injection references unknown search variable '{var_name}'"
                )));
            }
            continue;
        }

        let domain = dominance_minion_model
            .named_variables
            .get_vartype(var_name.clone())
            .ok_or_else(|| {
                Runtime(format!(
                    "dominance injection variable '{var_name}' is missing a Minion domain"
                ))
            })?;

        let fresh_name = format!(
            "__conjure_dominance_midsearch_aux_{}",
            *next_midsearch_aux_var_id
        );
        *next_midsearch_aux_var_id += 1;

        add_aux_var_during_search(fresh_name.clone(), domain).map_err(|e| {
            Runtime(format!(
                "failed to add Minion dominance aux variable '{fresh_name}' (from '{var_name}', domain={domain:?}): {e:#?}"
            ))
        })?;
        append_minion_injection_log(&format!(
            "[minion-inject] add_aux(solution#{solution_ordinal}): {var_name} -> {fresh_name}, domain={domain:?}"
        ));
        known_var_names.insert(fresh_name.clone());
        remap.insert(var_name, fresh_name);
    }

    for (constraint_idx, constraint) in dominance_minion_model.constraints.into_iter().enumerate() {
        let remapped_constraint = remap_var_names_in_constraint(constraint.clone(), &remap);
        append_minion_injection_log(&format!(
            "[minion-inject] add_constraint(solution#{solution_ordinal}, idx={constraint_idx}) original={constraint:?} remapped={remapped_constraint:?}"
        ));
        match add_constraint_during_search(remapped_constraint.clone()) {
            Ok(()) => append_minion_injection_log(&format!(
                "[minion-inject] add_constraint(solution#{solution_ordinal}, idx={constraint_idx}) => OK"
            )),
            Err(MinionError::RuntimeError(RuntimeError::InvalidInstance(msg)))
                if msg.contains("propagation failure when adding constraint midsearch") =>
            {
                append_minion_injection_log(&format!(
                    "[minion-inject] add_constraint(solution#{solution_ordinal}, idx={constraint_idx}) => PROPAGATION_FAILURE (treated as ok): {msg}"
                ));
            }
            Err(other) => {
                let solver_err = minion_error_to_solver_error(other);
                append_minion_injection_log(&format!(
                    "[minion-inject] add_constraint(solution#{solution_ordinal}, idx={constraint_idx}) => ERROR: {:#?}",
                    solver_err
                ));
                return Err(Runtime(format!(
                    "failed to inject Minion dominance constraint #{constraint_idx}: original={constraint:?}; remapped={remapped_constraint:?}; error={:#?}",
                    solver_err
                )));
            }
        }
    }

    append_minion_injection_log(&format!("[minion-inject] END solution#{solution_ordinal}"));
    Ok(())
}
