use crate::solver::{SolverError, SolverResult};
use super::proto::{
    CpModelProto, IntegerVariableProto, ConstraintProto, LinearConstraintProto,
    constraint_proto, CpSolverResponse, BoolArgumentProto,
};
use std::collections::HashMap;
use crate::Model;
use crate::ast::{AbstractLiteral, Atom, Expression, GroundDomain, HasDomain, Literal, Metadata, Name, Range, eval_constant};
use ustr::Ustr;

struct TranslationContext {
    var_mapping: HashMap<Name, i32>,
}

#[derive(Clone)]
struct LinearExpr {
    vars: Vec<i32>,
    coeffs: Vec<i64>,
    offset: i64,
}

/// Flattens a Conjure domain (e.g. [1..3, 5..8]) into a flat array of bounds [1, 3, 5, 8] expected by CP-SAT.
fn extract_domain_intervals(domain: &GroundDomain) -> SolverResult<Vec<i64>> {
    match domain {
        GroundDomain::Int(ranges) => {
            let mut intervals = Vec::new();
            for range in ranges {
                match range {
                    Range::Single(v) => {
                        intervals.push((*v as i64, *v as i64));
                    }
                    Range::Bounded(lb, ub) => {
                        intervals.push((*lb as i64, *ub as i64));
                    }
                    Range::UnboundedL(_)
                    | Range::UnboundedR(_)
                    | Range::Unbounded => {
                        return Err(SolverError::ModelFeatureNotSupported(
                            "CP-SAT does not support Unbounded int domains".into(),
                        ));
                    }
                }
            }

            if intervals.is_empty() {
                return Ok(vec![]);
            }

            intervals.sort_by_key(|&(lb, ub)| (lb, ub));

            let mut merged = Vec::new();
            let (mut current_lb, mut current_ub) = intervals[0];

            for &(lb, ub) in &intervals[1..] {
                if lb <= current_ub + 1 {
                    current_ub = std::cmp::max(current_ub, ub);
                } else {
                    merged.push((current_lb, current_ub));
                    current_lb = lb;
                    current_ub = ub;
                }
            }
            merged.push((current_lb, current_ub));

            let mut flat_domain = Vec::new();
            for (lb, ub) in merged {
                flat_domain.push(lb);
                flat_domain.push(ub);
            }
            Ok(flat_domain)
        }
        GroundDomain::Bool => Ok(vec![0, 1]),
        _ => Err(SolverError::ModelFeatureNotSupported(
            "Domain not supported by OR-Tools CP-SAT".into(),
        )),
    }
}

fn complement_domain_intervals(intervals: &[i64]) -> Vec<i64> {
    let mut comp = Vec::new();
    let mut last = i64::MIN;
    for chunk in intervals.chunks_exact(2) {
        let lb = chunk[0];
        let ub = chunk[1];
        if lb > last {
            comp.push(last);
            comp.push(lb - 1);
        }
        if ub == i64::MAX {
            return comp;
        }
        last = ub + 1;
    }
    comp.push(last);
    comp.push(i64::MAX);
    comp
}

fn extract_set_values(expr: &Expression) -> Option<Vec<i64>> {
    match expr {
        Expression::Atomic(_, Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Set(vals)))) => {
            let mut result = Vec::new();
            for val in vals {
                if let Literal::Int(i) = val {
                    result.push(*i as i64);
                } else {
                    return None;
                }
            }
            Some(result)
        }
        Expression::AbstractLiteral(_, AbstractLiteral::Set(vals)) => {
            let mut result = Vec::new();
            for val_expr in vals {
                if let Expression::Atomic(_, Atom::Literal(Literal::Int(i))) = val_expr {
                    result.push(*i as i64);
                } else {
                    return None;
                }
            }
            Some(result)
        }
        _ => None,
    }
}

fn values_to_flat_domain(values: &[i64]) -> Vec<i64> {
    if values.is_empty() {
        return vec![];
    }
    let mut intervals = values.iter().map(|&v| (v, v)).collect::<Vec<_>>();
    intervals.sort_by_key(|&(lb, ub)| (lb, ub));

    let mut merged = Vec::new();
    let (mut current_lb, mut current_ub) = intervals[0];

    for &(lb, ub) in &intervals[1..] {
        if lb <= current_ub + 1 {
            current_ub = std::cmp::max(current_ub, ub);
        } else {
            merged.push((current_lb, current_ub));
            current_lb = lb;
            current_ub = ub;
        }
    }
    merged.push((current_lb, current_ub));

    let mut flat_domain = Vec::new();
    for (lb, ub) in merged {
        flat_domain.push(lb);
        flat_domain.push(ub);
    }
    flat_domain
}

fn get_matrix_element_vars(src_var: &Name, ctx: &TranslationContext) -> Vec<i32> {
    let mut matching_vars = Vec::new();
    for (name, &idx) in &ctx.var_mapping {
        if let Name::Represented(box_tuple) = name {
            let (ref_var, repr_name, suffix) = box_tuple.as_ref();
            if ref_var == src_var && repr_name.as_str() == "matrix_to_atom" {
                let parts: Vec<i32> = suffix.split('_').filter_map(|s| s.parse::<i32>().ok()).collect();
                matching_vars.push((parts, idx));
            }
        }
    }
    matching_vars.sort_by(|(indices_a, _), (indices_b, _)| indices_a.cmp(indices_b));
    matching_vars.into_iter().map(|(_, idx)| idx).collect()
}

fn expr_to_linear_list(expr: &Expression, ctx: &TranslationContext) -> Option<Vec<LinearExpr>> {
    match expr {
        Expression::Atomic(_, Atom::Reference(reference)) => {
            let name = &*reference.name();
            if let Name::Represented(_) = name {
                // A Represented name is a single scalar element of a matrix (e.g. after matrix_to_list)
                if let Ok(linear) = expr_to_linear(expr, ctx) {
                    return Some(vec![linear]);
                } else {
                    return None;
                }
            }
            
            let base_name = match name {
                Name::WithRepresentation(name_box, _) => name_box.as_ref(),
                _ => name,
            };
            let vars = get_matrix_element_vars(base_name, ctx);
            if !vars.is_empty() {
                Some(vars.into_iter().map(|idx| LinearExpr {
                    vars: vec![idx],
                    coeffs: vec![1],
                    offset: 0,
                }).collect())
            } else if let Some(constant_literal) = reference.resolve_constant() {
                expr_to_linear_list(&Expression::Atomic(Metadata::default(), Atom::Literal(constant_literal)), ctx)
            } else {
                None
            }
        }
        Expression::AbstractLiteral(_, AbstractLiteral::Matrix(elems, _)) => {
            let mut list = Vec::new();
            for elem in elems {
                if let Some(sub_list) = expr_to_linear_list(elem, ctx) {
                    list.extend(sub_list);
                } else if let Ok(lin) = expr_to_linear(elem, ctx) {
                    list.push(lin);
                } else {
                    return None;
                }
            }
            Some(list)
        }
        _ => None,
    }
}

fn extract_linear_parts(
    expr: &Expression,
    ctx: &TranslationContext,
) -> SolverResult<Option<(LinearExpr, LinearExpr, Box<dyn Fn(i64) -> SolverResult<Vec<i64>>>)>> {
    let result = match expr {
        Expression::Eq(_, lhs, rhs) => (
            expr_to_linear(lhs.as_ref(), ctx)?,
            expr_to_linear(rhs.as_ref(), ctx)?,
            Box::new(|offset: i64| Ok(vec![-offset, -offset])) as Box<dyn Fn(i64) -> SolverResult<Vec<i64>>>,
        ),
        Expression::Neq(_, lhs, rhs) => (
            expr_to_linear(lhs.as_ref(), ctx)?,
            expr_to_linear(rhs.as_ref(), ctx)?,
            Box::new(|offset: i64| Ok(vec![i64::MIN, -offset - 1, -offset + 1, i64::MAX])) as Box<dyn Fn(i64) -> SolverResult<Vec<i64>>>,
        ),
        Expression::Leq(_, lhs, rhs) => (
            expr_to_linear(lhs.as_ref(), ctx)?,
            expr_to_linear(rhs.as_ref(), ctx)?,
            Box::new(|offset: i64| Ok(vec![i64::MIN, -offset])) as Box<dyn Fn(i64) -> SolverResult<Vec<i64>>>,
        ),
        Expression::Geq(_, lhs, rhs) => (
            expr_to_linear(lhs.as_ref(), ctx)?,
            expr_to_linear(rhs.as_ref(), ctx)?,
            Box::new(|offset: i64| Ok(vec![-offset, i64::MAX])) as Box<dyn Fn(i64) -> SolverResult<Vec<i64>>>,
        ),
        Expression::Lt(_, lhs, rhs) => (
            expr_to_linear(lhs.as_ref(), ctx)?,
            expr_to_linear(rhs.as_ref(), ctx)?,
            Box::new(|offset: i64| Ok(vec![i64::MIN, -offset - 1])) as Box<dyn Fn(i64) -> SolverResult<Vec<i64>>>,
        ),
        Expression::Gt(_, lhs, rhs) => (
            expr_to_linear(lhs.as_ref(), ctx)?,
            expr_to_linear(rhs.as_ref(), ctx)?,
            Box::new(|offset: i64| Ok(vec![-offset + 1, i64::MAX])) as Box<dyn Fn(i64) -> SolverResult<Vec<i64>>>,
        ),
        Expression::FlatSumLeq(_, vars, total) => {
            let mut lhs_linear = LinearExpr { vars: vec![], coeffs: vec![], offset: 0 };
            for var in vars {
                let var_linear = expr_to_linear(&Expression::Atomic(Metadata::default(), var.clone()), ctx)?;
                lhs_linear.vars.extend(var_linear.vars);
                lhs_linear.coeffs.extend(var_linear.coeffs);
                lhs_linear.offset += var_linear.offset;
            }
            let rhs_linear = expr_to_linear(&Expression::Atomic(Metadata::default(), total.clone()), ctx)?;
            (lhs_linear, rhs_linear, Box::new(|offset: i64| Ok(vec![i64::MIN, -offset])) as Box<dyn Fn(i64) -> SolverResult<Vec<i64>>>)
        },
        Expression::FlatWeightedSumLeq(_, coeffs, vars, total) => {
            let mut lhs_linear = LinearExpr { vars: vec![], coeffs: vec![], offset: 0 };
            for (coeff_lit, var) in coeffs.iter().zip(vars) {
                let Literal::Int(coeff_val) = coeff_lit else {
                    return Err(SolverError::ModelInvalid("Weighted sum coefficient is not an integer".into()));
                };
                let var_linear = expr_to_linear(&Expression::Atomic(Metadata::default(), var.clone()), ctx)?;
                lhs_linear.vars.extend(var_linear.vars.clone());
                lhs_linear.coeffs.extend(var_linear.coeffs.iter().map(|c| c * *coeff_val as i64));
                lhs_linear.offset += var_linear.offset * *coeff_val as i64;
            }
            let rhs_linear = expr_to_linear(&Expression::Atomic(Metadata::default(), total.as_ref().clone()), ctx)?;
            (lhs_linear, rhs_linear, Box::new(|offset: i64| Ok(vec![i64::MIN, -offset])) as Box<dyn Fn(i64) -> SolverResult<Vec<i64>>>)
        },
        Expression::FlatSumGeq(_, vars, total) => {
            let mut lhs_linear = LinearExpr { vars: vec![], coeffs: vec![], offset: 0 };
            for var in vars {
                let var_linear = expr_to_linear(&Expression::Atomic(Metadata::default(), var.clone()), ctx)?;
                lhs_linear.vars.extend(var_linear.vars);
                lhs_linear.coeffs.extend(var_linear.coeffs);
                lhs_linear.offset += var_linear.offset;
            }
            let rhs_linear = expr_to_linear(&Expression::Atomic(Metadata::default(), total.clone()), ctx)?;
            (lhs_linear, rhs_linear, Box::new(|offset: i64| Ok(vec![-offset, i64::MAX])) as Box<dyn Fn(i64) -> SolverResult<Vec<i64>>>)
        },
        Expression::FlatWeightedSumGeq(_, coeffs, vars, total) => {
            let mut lhs_linear = LinearExpr { vars: vec![], coeffs: vec![], offset: 0 };
            for (coeff_lit, var) in coeffs.iter().zip(vars) {
                let Literal::Int(coeff_val) = coeff_lit else {
                    return Err(SolverError::ModelInvalid("Weighted sum coefficient is not an integer".into()));
                };
                let var_linear = expr_to_linear(&Expression::Atomic(Metadata::default(), var.clone()), ctx)?;
                lhs_linear.vars.extend(var_linear.vars.clone());
                lhs_linear.coeffs.extend(var_linear.coeffs.iter().map(|c| c * *coeff_val as i64));
                lhs_linear.offset += var_linear.offset * *coeff_val as i64;
            }
            let rhs_linear = expr_to_linear(&Expression::Atomic(Metadata::default(), total.as_ref().clone()), ctx)?;
            (lhs_linear, rhs_linear, Box::new(|offset: i64| Ok(vec![-offset, i64::MAX])) as Box<dyn Fn(i64) -> SolverResult<Vec<i64>>>)
        },
        _ => return Ok(None),
    };
    Ok(Some(result))
}

fn expr_to_linear(expr: &Expression, ctx: &TranslationContext) -> SolverResult<LinearExpr> {
    match expr {
        Expression::Atomic(_, Atom::Literal(Literal::Int(value))) => Ok(LinearExpr {
            vars: vec![],
            coeffs: vec![],
            offset: *value as i64,
        }),
        Expression::Atomic(_, Atom::Literal(Literal::Bool(value))) => Ok(LinearExpr {
            vars: vec![],
            coeffs: vec![],
            offset: i64::from(*value),
        }),
        Expression::Atomic(_, Atom::Reference(reference)) => {
            if let Some(Literal::Int(val)) = reference.resolve_constant() {
                return Ok(LinearExpr { vars: vec![], coeffs: vec![], offset: val as i64 });
            }
            if let Some(Literal::Bool(val)) = reference.resolve_constant() {
                return Ok(LinearExpr { vars: vec![], coeffs: vec![], offset: i64::from(val) });
            }

            let name = reference.name();
            let var_index = ctx.var_mapping.get(&name).ok_or_else(|| {
                SolverError::ModelInvalid(format!("Unknown variable in constraint: {}", name))
            })?;

            Ok(LinearExpr {
                vars: vec![*var_index],
                coeffs: vec![1],
                offset: 0,
            })
        }
        Expression::Neg(_, inner) => {
            let lin = expr_to_linear(inner, ctx)?;
            Ok(LinearExpr {
                vars: lin.vars,
                coeffs: lin.coeffs.into_iter().map(|c| -c).collect(),
                offset: -lin.offset,
            })
        }
        Expression::Not(_, inner) => {
            let lin = expr_to_linear(inner, ctx)?;
            Ok(LinearExpr {
                vars: lin.vars,
                coeffs: lin.coeffs.into_iter().map(|c| -c).collect(),
                offset: 1 - lin.offset,
            })
        }
        Expression::Sum(_, inner) => {
            match inner.as_ref() {
                Expression::AbstractLiteral(_, AbstractLiteral::Matrix(elems, _)) => {
                    let mut vars = Vec::new();
                    let mut coeffs = Vec::new();
                    let mut offset = 0;
                    for elem in elems {
                        let lin = expr_to_linear(elem, ctx)?;
                        vars.extend(lin.vars);
                        coeffs.extend(lin.coeffs);
                        offset += lin.offset;
                    }
                    Ok(LinearExpr {
                        vars,
                        coeffs,
                        offset,
                    })
                }
                _ => Err(SolverError::ModelFeatureNotSupported(format!(
                    "Unsupported sum argument in linear constraint: {:?}",
                    inner
                ))),
            }
        }
        Expression::ToInt(_, inner) => {
            expr_to_linear(inner, ctx)
        }
        _ => Err(SolverError::ModelFeatureNotSupported(format!(
            "Unsupported expression in linear constraint: {expr:?}"
        ))),
    }
}

/// Helper to build a Protobuf linear constraint that enforces exactly one specific value.
fn exact_linear_constraint(linear_expr: LinearExpr, value: i64) -> ConstraintProto {
    ConstraintProto {
        name: String::new(),
        enforcement_literal: vec![],
        constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
            vars: linear_expr.vars,
            coeffs: linear_expr.coeffs,
            domain: vec![value - linear_expr.offset, value - linear_expr.offset],
        })),
    }
}

fn get_or_create_var_for_linear(
    linear: LinearExpr,
    cp_model: &mut CpModelProto,
) -> i32 {
    if linear.vars.len() == 1 && linear.coeffs == vec![1] && linear.offset == 0 {
        linear.vars[0]
    } else {
        let var_index = cp_model.variables.len() as i32;
        let mut var_proto = IntegerVariableProto::default();
        var_proto.domain = vec![-1000000000, 1000000000];
        cp_model.variables.push(var_proto);
        let mut vars = vec![var_index];
        let mut coeffs = vec![1];
        for (v, c) in linear.vars.iter().zip(linear.coeffs) {
            vars.push(*v);
            coeffs.push(-c);
        }
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![],
            constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                vars,
                coeffs,
                domain: vec![linear.offset, linear.offset],
            })),
        });
        var_index
    }
}

/// Subtracts the right-hand linear expression from the left-hand one, effectively moving RHS to LHS (LHS - RHS).
fn subtract_linear_exprs(lhs: LinearExpr, rhs: LinearExpr) -> LinearExpr {
    let mut vars = lhs.vars;
    vars.extend(rhs.vars);

    let mut coeffs = lhs.coeffs;
    coeffs.extend(rhs.coeffs.into_iter().map(|coeff| -coeff));

    LinearExpr {
        vars,
        coeffs,
        offset: lhs.offset - rhs.offset,
    }
}

fn domain_contains(domain: &[i64], value: i64) -> bool {
    for chunk in domain.chunks_exact(2) {
        if chunk[0] <= value && value <= chunk[1] {
            return true;
        }
    }
    false
}

fn translate_div_mod_undef_zero(
    is_div: bool,
    a_expr: &LinearExpr,
    b_expr: &LinearExpr,
    target_expr: &LinearExpr,
    cp_model: &mut CpModelProto,
) -> SolverResult<ConstraintProto> {
    use super::proto::{LinearArgumentProto, LinearExpressionProto};

    let mut get_or_create_constant_var = |model: &mut CpModelProto, val: i64| -> i32 {
        let name = format!("const_{}", val);
        for (idx, var) in model.variables.iter().enumerate() {
            if var.name == name {
                return idx as i32;
            }
        }
        let idx = model.variables.len() as i32;
        model.variables.push(IntegerVariableProto {
            name,
            domain: vec![val, val],
        });
        idx
    };

    let mut create_bool_var = |model: &mut CpModelProto, name: &str| -> i32 {
        let idx = model.variables.len() as i32;
        model.variables.push(IntegerVariableProto {
            name: format!("{}_{}", name, idx),
            domain: vec![0, 1],
        });
        idx
    };

    let estimate_bounds = |model: &CpModelProto, expr: &LinearExpr| -> (i64, i64) {
        let mut min_val = expr.offset;
        let mut max_val = expr.offset;
        for (&var, &coeff) in expr.vars.iter().zip(expr.coeffs.iter()) {
            let var_domain = &model.variables[var as usize].domain;
            let var_min = var_domain[0];
            let var_max = var_domain[var_domain.len() - 1];
            if coeff > 0 {
                min_val += coeff * var_min;
                max_val += coeff * var_max;
            } else {
                min_val += coeff * var_max;
                max_val += coeff * var_min;
            }
        }
        (min_val, max_val)
    };

    let get_prod_bounds = |q_min: i64, q_max: i64, b_min: i64, b_max: i64| -> (i64, i64) {
        let candidates = [
            q_min * b_min,
            q_min * b_max,
            q_max * b_min,
            q_max * b_max,
        ];
        let min_c = *candidates.iter().min().unwrap();
        let max_c = *candidates.iter().max().unwrap();
        (min_c, max_c)
    };

    let (a_min, a_max) = estimate_bounds(cp_model, &a_expr);
    let m = a_min.abs().max(a_max.abs());
    let q_bound_min = -m - 1;
    let q_bound_max = m + 1;

    // If divisor is a constant
    if b_expr.vars.is_empty() {
        let val = b_expr.offset;
        if val == 0 {
            // target == 0
            return Ok(exact_linear_constraint(target_expr.clone(), 0));
        } else if val > 0 {
            // q = target_expr if is_div else aux_var
            // r = aux_var if is_div else target_expr
            let (q, r) = if is_div {
                let r_var = cp_model.variables.len() as i32;
                cp_model.variables.push(IntegerVariableProto {
                    name: format!("r_var_const_{}", r_var),
                    domain: vec![0, val - 1],
                });
                (
                    target_expr.clone(),
                    LinearExpr {
                        vars: vec![r_var],
                        coeffs: vec![1],
                        offset: 0,
                    },
                )
            } else {
                // target_expr must be in [0, val - 1]
                cp_model.constraints.push(ConstraintProto {
                    name: String::new(),
                    enforcement_literal: vec![],
                    constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                        vars: target_expr.vars.clone(),
                        coeffs: target_expr.coeffs.clone(),
                        domain: vec![0 - target_expr.offset, val - 1 - target_expr.offset],
                    })),
                });
                let q_var = cp_model.variables.len() as i32;
                cp_model.variables.push(IntegerVariableProto {
                    name: format!("q_var_const_{}", q_var),
                    domain: vec![q_bound_min, q_bound_max],
                });
                (
                    LinearExpr {
                        vars: vec![q_var],
                        coeffs: vec![1],
                        offset: 0,
                    },
                    target_expr.clone(),
                )
            };

            // Enforce a - val * q - r = 0
            let mut vars = a_expr.vars.clone();
            let mut coeffs = a_expr.coeffs.clone();
            let mut offset = a_expr.offset;

            for (v, c) in q.vars.iter().zip(q.coeffs.iter()) {
                vars.push(*v);
                coeffs.push(-val * c);
            }
            offset -= val * q.offset;

            for (v, c) in r.vars.iter().zip(r.coeffs.iter()) {
                vars.push(*v);
                coeffs.push(-c);
            }
            offset -= r.offset;

            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                    vars,
                    coeffs,
                    domain: vec![-offset, -offset],
                })),
            });
            return Ok(exact_linear_constraint(LinearExpr { vars: vec![], coeffs: vec![], offset: 0 }, 0));
        } else {
            // val < 0
            // q = target_expr if is_div else aux_var
            // r = aux_var if is_div else target_expr
            let (q, r) = if is_div {
                let r_var = cp_model.variables.len() as i32;
                cp_model.variables.push(IntegerVariableProto {
                    name: format!("r_var_const_{}", r_var),
                    domain: vec![val + 1, 0],
                });
                (
                    target_expr.clone(),
                    LinearExpr {
                        vars: vec![r_var],
                        coeffs: vec![1],
                        offset: 0,
                    },
                )
            } else {
                // target_expr must be in [val + 1, 0]
                cp_model.constraints.push(ConstraintProto {
                    name: String::new(),
                    enforcement_literal: vec![],
                    constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                        vars: target_expr.vars.clone(),
                        coeffs: target_expr.coeffs.clone(),
                        domain: vec![val + 1 - target_expr.offset, 0 - target_expr.offset],
                    })),
                });
                let q_var = cp_model.variables.len() as i32;
                cp_model.variables.push(IntegerVariableProto {
                    name: format!("q_var_const_{}", q_var),
                    domain: vec![q_bound_min, q_bound_max],
                });
                (
                    LinearExpr {
                        vars: vec![q_var],
                        coeffs: vec![1],
                        offset: 0,
                    },
                    target_expr.clone(),
                )
            };

            // Enforce a - val * q - r = 0
            let mut vars = a_expr.vars.clone();
            let mut coeffs = a_expr.coeffs.clone();
            let mut offset = a_expr.offset;

            for (v, c) in q.vars.iter().zip(q.coeffs.iter()) {
                vars.push(*v);
                coeffs.push(-val * c);
            }
            offset -= val * q.offset;

            for (v, c) in r.vars.iter().zip(r.coeffs.iter()) {
                vars.push(*v);
                coeffs.push(-c);
            }
            offset -= r.offset;

            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                    vars,
                    coeffs,
                    domain: vec![-offset, -offset],
                })),
            });
            return Ok(exact_linear_constraint(LinearExpr { vars: vec![], coeffs: vec![], offset: 0 }, 0));
        }
    }

    // Divisor is a variable
    let b_var = b_expr.vars[0];
    let b_domain = cp_model.variables[b_var as usize].domain.clone();
    let b_min = b_domain[0];
    let b_max = b_domain[b_domain.len() - 1];
    let has_zero = domain_contains(&b_domain, 0);

    let mut partition_vars = vec![];

    if b_max > 0 {
        let is_pos = create_bool_var(cp_model, "div_is_pos");
        partition_vars.push(is_pos);

        // Enforce b >= 1 if is_pos
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![is_pos],
            constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                vars: vec![b_var],
                coeffs: vec![1],
                domain: vec![1, i64::MAX],
            })),
        });

        // Define b_pos variable in [0, b_max]
        let b_pos = cp_model.variables.len() as i32;
        cp_model.variables.push(IntegerVariableProto {
            name: format!("b_pos_{}", b_pos),
            domain: vec![0, b_max],
        });

        // Enforce b_pos == b if is_pos
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![is_pos],
            constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                vars: vec![b_pos, b_var],
                coeffs: vec![1, -1],
                domain: vec![0, 0],
            })),
        });

        // Now implement floor division relation under is_pos:
        // q_pos and r_pos variables.
        let q_pos = cp_model.variables.len() as i32;
        cp_model.variables.push(IntegerVariableProto {
            name: format!("q_pos_{}", q_pos),
            domain: vec![q_bound_min, q_bound_max],
        });

        let r_pos = cp_model.variables.len() as i32;
        cp_model.variables.push(IntegerVariableProto {
            name: format!("r_pos_{}", r_pos),
            domain: vec![0, b_max - 1], // remainder is positive and less than b_pos
        });

        // If is_div is true, target_expr == q_pos under is_pos
        // If is_div is false, target_expr == r_pos under is_pos
        let target_var_in_partition = if is_div { q_pos } else { r_pos };
        let mut vars = target_expr.vars.clone();
        let mut coeffs = target_expr.coeffs.clone();
        vars.push(target_var_in_partition);
        coeffs.push(-1);
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![is_pos],
            constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                vars,
                coeffs,
                domain: vec![-target_expr.offset, -target_expr.offset],
            })),
        });

        // Enforce product: prod_var == q_pos * b_pos
        let prod_var = cp_model.variables.len() as i32;
        let (prod_min, prod_max) = get_prod_bounds(q_bound_min, q_bound_max, 1, b_max);
        cp_model.variables.push(IntegerVariableProto {
            name: format!("prod_pos_{}", prod_var),
            domain: vec![prod_min, prod_max],
        });
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![],
            constraint: Some(constraint_proto::Constraint::IntProd(LinearArgumentProto {
                target: Some(LinearExpressionProto {
                    vars: vec![prod_var],
                    coeffs: vec![1],
                    offset: 0,
                }),
                exprs: vec![
                    LinearExpressionProto { vars: vec![q_pos], coeffs: vec![1], offset: 0 },
                    LinearExpressionProto { vars: vec![b_pos], coeffs: vec![1], offset: 0 },
                ],
            })),
        });

        // Enforce a == prod_var + r_pos under is_pos
        let mut vars = a_expr.vars.clone();
        let mut coeffs = a_expr.coeffs.clone();
        vars.push(prod_var);
        coeffs.push(-1);
        vars.push(r_pos);
        coeffs.push(-1);
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![is_pos],
            constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                vars,
                coeffs,
                domain: vec![-a_expr.offset, -a_expr.offset],
            })),
        });

        // Enforce remainder bounds under is_pos:
        // 0 <= r_pos <= b_pos - 1 => r_pos >= 0 and r_pos - b_pos <= -1
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![is_pos],
            constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                vars: vec![r_pos, b_pos],
                coeffs: vec![1, -1],
                domain: vec![i64::MIN, -1],
            })),
        });

        // Enforce q_pos == 0 if !is_pos
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![-is_pos - 1],
            constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                vars: vec![q_pos],
                coeffs: vec![1],
                domain: vec![0, 0],
            })),
        });

        // Enforce r_pos == 0 if !is_pos
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![-is_pos - 1],
            constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                vars: vec![r_pos],
                coeffs: vec![1],
                domain: vec![0, 0],
            })),
        });

        // Enforce b_pos == 0 if !is_pos
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![-is_pos - 1],
            constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                vars: vec![b_pos],
                coeffs: vec![1],
                domain: vec![0, 0],
            })),
        });
    }

    if b_min < 0 {
        let is_neg = create_bool_var(cp_model, "div_is_neg");
        partition_vars.push(is_neg);

        // Enforce b <= -1 if is_neg
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![is_neg],
            constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                vars: vec![b_var],
                coeffs: vec![1],
                domain: vec![i64::MIN, -1],
            })),
        });

        // Define b_neg variable in [b_min, 0]
        let b_neg = cp_model.variables.len() as i32;
        cp_model.variables.push(IntegerVariableProto {
            name: format!("b_neg_{}", b_neg),
            domain: vec![b_min, 0],
        });

        // Enforce b_neg == b if is_neg
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![is_neg],
            constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                vars: vec![b_neg, b_var],
                coeffs: vec![1, -1],
                domain: vec![0, 0],
            })),
        });

        // Now implement floor division relation under is_neg:
        // q_neg and r_neg variables.
        let q_neg = cp_model.variables.len() as i32;
        cp_model.variables.push(IntegerVariableProto {
            name: format!("q_neg_{}", q_neg),
            domain: vec![q_bound_min, q_bound_max],
        });

        let r_neg = cp_model.variables.len() as i32;
        cp_model.variables.push(IntegerVariableProto {
            name: format!("r_neg_{}", r_neg),
            domain: vec![b_min + 1, 0], // remainder is negative or zero, and greater than b_neg
        });

        // If is_div is true, target_expr == q_neg under is_neg
        // If is_div is false, target_expr == r_neg under is_neg
        let target_var_in_partition = if is_div { q_neg } else { r_neg };
        let mut vars = target_expr.vars.clone();
        let mut coeffs = target_expr.coeffs.clone();
        vars.push(target_var_in_partition);
        coeffs.push(-1);
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![is_neg],
            constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                vars,
                coeffs,
                domain: vec![-target_expr.offset, -target_expr.offset],
            })),
        });

        // Enforce product: prod_var == q_neg * b_neg
        let prod_var = cp_model.variables.len() as i32;
        let (prod_min, prod_max) = get_prod_bounds(q_bound_min, q_bound_max, b_min, -1);
        cp_model.variables.push(IntegerVariableProto {
            name: format!("prod_neg_{}", prod_var),
            domain: vec![prod_min, prod_max],
        });
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![],
            constraint: Some(constraint_proto::Constraint::IntProd(LinearArgumentProto {
                target: Some(LinearExpressionProto {
                    vars: vec![prod_var],
                    coeffs: vec![1],
                    offset: 0,
                }),
                exprs: vec![
                    LinearExpressionProto { vars: vec![q_neg], coeffs: vec![1], offset: 0 },
                    LinearExpressionProto { vars: vec![b_neg], coeffs: vec![1], offset: 0 },
                ],
            })),
        });

        // Enforce a == prod_var + r_neg under is_neg
        let mut vars = a_expr.vars.clone();
        let mut coeffs = a_expr.coeffs.clone();
        vars.push(prod_var);
        coeffs.push(-1);
        vars.push(r_neg);
        coeffs.push(-1);
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![is_neg],
            constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                vars,
                coeffs,
                domain: vec![-a_expr.offset, -a_expr.offset],
            })),
        });

        // Enforce remainder bounds under is_neg:
        // b_neg + 1 <= r_neg <= 0 => r_neg <= 0 and r_neg - b_neg >= 1
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![is_neg],
            constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                vars: vec![r_neg, b_neg],
                coeffs: vec![1, -1],
                domain: vec![1, i64::MAX],
            })),
        });

        // Enforce q_neg == 0 if !is_neg
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![-is_neg - 1],
            constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                vars: vec![q_neg],
                coeffs: vec![1],
                domain: vec![0, 0],
            })),
        });

        // Enforce r_neg == 0 if !is_neg
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![-is_neg - 1],
            constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                vars: vec![r_neg],
                coeffs: vec![1],
                domain: vec![0, 0],
            })),
        });

        // Enforce b_neg == 0 if !is_neg
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![-is_neg - 1],
            constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                vars: vec![b_neg],
                coeffs: vec![1],
                domain: vec![0, 0],
            })),
        });
    }

    if has_zero {
        let is_zero = create_bool_var(cp_model, "div_is_zero");
        partition_vars.push(is_zero);

        // Enforce b == 0 if is_zero
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![is_zero],
            constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                vars: vec![b_var],
                coeffs: vec![1],
                domain: vec![0, 0],
            })),
        });

        // Enforce target == 0 if is_zero
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![is_zero],
            constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                vars: target_expr.vars.clone(),
                coeffs: target_expr.coeffs.clone(),
                domain: vec![0 - target_expr.offset, 0 - target_expr.offset],
            })),
        });
    }

    // Enforce exactly one partition variable is active
    let num_partition_vars = partition_vars.len();
    cp_model.constraints.push(ConstraintProto {
        name: String::new(),
        enforcement_literal: vec![],
        constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
            vars: partition_vars,
            coeffs: vec![1; num_partition_vars],
            domain: vec![1, 1],
        })),
    });

    // Return a dummy constraint that is always true
    Ok(exact_linear_constraint(LinearExpr { vars: vec![], coeffs: vec![], offset: 0 }, 0))
}

/// Maps a relational operator (e.g. <=, >=, =) to a valid CP-SAT interval bound, applying the given offset.
fn comparison_domain(expr: &Expression, offset: i64) -> SolverResult<Vec<i64>> {
    match expr {
        Expression::Eq(_, _, _) => Ok(vec![-offset, -offset]),
        Expression::Leq(_, _, _) => Ok(vec![i64::MIN, -offset]),
        Expression::Geq(_, _, _) => Ok(vec![-offset, i64::MAX]),
        Expression::Lt(_, _, _) => Ok(vec![i64::MIN, -offset - 1]),
        Expression::Gt(_, _, _) => Ok(vec![-offset + 1, i64::MAX]),
        _ => Err(SolverError::ModelFeatureNotSupported(format!(
            "Unsupported constraint: {expr:?}"
        ))),
    }
}

fn get_literal(expr: &Expression, ctx: &TranslationContext) -> SolverResult<i32> {
    match expr {
        Expression::Atomic(_, Atom::Reference(reference)) => {
            let name = reference.name();
            let var_index = ctx.var_mapping.get(&name).ok_or_else(|| {
                SolverError::ModelInvalid(format!("Unknown variable in constraint: {}", name))
            })?;
            Ok(*var_index)
        }
        Expression::Atomic(_, Atom::Literal(Literal::Bool(value))) => {
            Err(SolverError::ModelFeatureNotSupported("Constant boolean literal inside logical constraint not supported yet".to_string()))
        }
        Expression::Not(_, inner) => {
            let inner_lit = get_literal(inner.as_ref(), ctx)?;
            Ok(-inner_lit - 1)
        }
        _ => {
            Err(SolverError::ModelFeatureNotSupported(format!(
                "Logical constraint children must be atomic or negation of atomic, got {:?}",
                expr
            )))
        }
    }
}

fn equate_literal_and_var(ref_var: i32, lit: i32, cp_model: &mut CpModelProto) {
    if lit >= 0 {
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![],
            constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                vars: vec![ref_var, lit],
                coeffs: vec![1, -1],
                domain: vec![0, 0],
            })),
        });
    } else {
        let var = -lit - 1;
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![],
            constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                vars: vec![ref_var, var],
                coeffs: vec![1, 1],
                domain: vec![1, 1],
            })),
        });
    }
}

fn get_domain_values(domain: &[i64]) -> Vec<i64> {
    let mut values = Vec::new();
    for chunk in domain.chunks_exact(2) {
        let lb = chunk[0];
        let ub = chunk[1];
        for v in lb..=ub {
            values.push(v);
        }
    }
    values
}

fn checked_pow(base: i64, exp: i64) -> Option<i64> {
    if exp == 0 {
        return Some(1);
    }
    let mut result: i64 = 1;
    let mut base_val = base;
    let mut exp_val = exp;
    while exp_val > 0 {
        if exp_val % 2 == 1 {
            result = result.checked_mul(base_val)?;
        }
        exp_val /= 2;
        if exp_val > 0 {
            base_val = base_val.checked_mul(base_val)?;
        }
    }
    Some(result)
}

fn translate_table_constraint(
    negated: bool,
    tuple_expr: &Expression,
    rows_expr: &Expression,
    ctx: &TranslationContext,
) -> SolverResult<ConstraintProto> {
    let Expression::AbstractLiteral(_, AbstractLiteral::Matrix(tuple_elems, _)) = tuple_expr else {
        return Err(SolverError::ModelInvalid("Table first argument is not a matrix".into()));
    };
    
    // Identify which tuple elements are constant vs variable
    let mut active_indices = Vec::new();
    let mut constant_values = Vec::new(); // Store (index, value) for constant elements
    let mut vars = Vec::new();
    
    for (i, elem) in tuple_elems.iter().enumerate() {
        let linear = expr_to_linear(elem, ctx)?;
        if linear.vars.is_empty() {
            constant_values.push((i, linear.offset));
        } else {
            active_indices.push(i);
            if linear.vars.len() == 1 && linear.coeffs == vec![1] && linear.offset == 0 {
                vars.push(linear.vars[0]);
            } else {
                return Err(SolverError::ModelFeatureNotSupported("Complex expression in Table constraint".into()));
            }
        }
    }
    
    let Some(Literal::AbstractLiteral(AbstractLiteral::Matrix(rows, _))) = eval_constant(rows_expr) else {
        return Err(SolverError::ModelInvalid("Table second argument is not a constant matrix".into()));
    };
    
    let mut values = Vec::new();
    let mut matched_any_row = false;
    for row in rows {
        let Literal::AbstractLiteral(AbstractLiteral::Matrix(row_elems, _)) = row else {
            return Err(SolverError::ModelInvalid("Table row is not a constant matrix".into()));
        };
        if row_elems.len() != tuple_elems.len() {
            return Err(SolverError::ModelInvalid("Table row width does not match tuple width".into()));
        }
        
        // Check if constant elements match the row's values
        let mut row_values = Vec::new();
        for elem in row_elems {
            match elem {
                Literal::Int(val) => {
                    row_values.push(val as i64);
                }
                Literal::Bool(val) => {
                    row_values.push(if val { 1 } else { 0 });
                }
                _ => {
                    return Err(SolverError::ModelInvalid("Table row contains non-integer/bool literal".into()));
                }
            }
        }
        
        let mut matches_constants = true;
        for &(idx, const_val) in &constant_values {
            if row_values[idx] != const_val {
                matches_constants = false;
                break;
            }
        }
        
        if matches_constants {
            matched_any_row = true;
            // Project row values to only active indices
            for &idx in &active_indices {
                values.push(row_values[idx]);
            }
        }
    }

    if vars.is_empty() {
        if matched_any_row {
            let target_val = if negated { 1 } else { 0 };
            return Ok(exact_linear_constraint(LinearExpr { vars: vec![], coeffs: vec![], offset: 0 }, target_val));
        } else {
            let target_val = if negated { 0 } else { 1 };
            return Ok(exact_linear_constraint(LinearExpr { vars: vec![], coeffs: vec![], offset: 0 }, target_val));
        }
    }
    
    use super::proto::TableConstraintProto;
    Ok(ConstraintProto {
        name: String::new(),
        enforcement_literal: vec![],
        constraint: Some(constraint_proto::Constraint::Table(TableConstraintProto {
            vars,
            values,
            exprs: vec![],
            negated,
        })),
    })
}

fn translate_reified_constraint(
    ref_var: i32,
    inner_expr: &Expression,
    cp_model: &mut CpModelProto,
    ctx: &TranslationContext,
) -> SolverResult<ConstraintProto> {
    // 1. Check if it's a constant boolean
    if let Some(Literal::Bool(val)) = eval_constant(inner_expr) {
        let target_val = if val { 1 } else { 0 };
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![],
            constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                vars: vec![ref_var],
                coeffs: vec![1],
                domain: vec![target_val, target_val],
            })),
        });
        return Ok(exact_linear_constraint(LinearExpr { vars: vec![], coeffs: vec![], offset: 0 }, 0));
    }

    // Check for logical operators: And, Or
    match inner_expr {
        Expression::And(_, inner) => {
            let Expression::AbstractLiteral(_, AbstractLiteral::Matrix(elems, _)) = inner.as_ref() else {
                return Err(SolverError::ModelFeatureNotSupported(format!(
                    "Unsupported And argument in reification: {:?}",
                    inner
                )));
            };
            let mut literals = Vec::new();
            for elem in elems {
                literals.push(get_literal(elem, ctx)?);
            }
            
            // ref_var <=> And(literals)
            // 1. ref_var => each literal
            for &lit in &literals {
                cp_model.constraints.push(ConstraintProto {
                    name: String::new(),
                    enforcement_literal: vec![],
                    constraint: Some(constraint_proto::Constraint::BoolOr(BoolArgumentProto {
                        literals: vec![-ref_var - 1, lit],
                    })),
                });
            }
            // 2. And(literals) => ref_var (i.e. not lit1 \/ not lit2 \/ ... \/ ref_var)
            let mut or_literals = literals.iter().map(|&lit| -lit - 1).collect::<Vec<_>>();
            or_literals.push(ref_var);
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::BoolOr(BoolArgumentProto {
                    literals: or_literals,
                })),
            });
            
            return Ok(exact_linear_constraint(LinearExpr { vars: vec![], coeffs: vec![], offset: 0 }, 0));
        }
        Expression::Or(_, inner) => {
            let Expression::AbstractLiteral(_, AbstractLiteral::Matrix(elems, _)) = inner.as_ref() else {
                return Err(SolverError::ModelFeatureNotSupported(format!(
                    "Unsupported Or argument in reification: {:?}",
                    inner
                )));
            };
            let mut literals = Vec::new();
            for elem in elems {
                literals.push(get_literal(elem, ctx)?);
            }
            
            // ref_var <=> Or(literals)
            // 1. each literal => ref_var
            for &lit in &literals {
                cp_model.constraints.push(ConstraintProto {
                    name: String::new(),
                    enforcement_literal: vec![],
                    constraint: Some(constraint_proto::Constraint::BoolOr(BoolArgumentProto {
                        literals: vec![-lit - 1, ref_var],
                    })),
                });
            }
            // 2. ref_var => Or(literals) (i.e. not ref_var \/ lit1 \/ lit2 \/ ...)
            let mut or_literals = literals.clone();
            or_literals.push(-ref_var - 1);
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::BoolOr(BoolArgumentProto {
                    literals: or_literals,
                })),
            });
            
            return Ok(exact_linear_constraint(LinearExpr { vars: vec![], coeffs: vec![], offset: 0 }, 0));
        }
        Expression::Eq(meta, lhs, rhs) => {
            if let (Some(elems_l), Some(elems_r)) = (expr_to_linear_list(lhs.as_ref(), ctx), expr_to_linear_list(rhs.as_ref(), ctx)) {
                if elems_l.len() != elems_r.len() {
                    return Err(SolverError::ModelInvalid("Matrix equality with different lengths".into()));
                }
                let mut aux_vars = Vec::new();
                for (el, er) in elems_l.into_iter().zip(elems_r) {
                    let diff = subtract_linear_exprs(el, er);
                    let aux_idx = cp_model.variables.len() as i32;
                    cp_model.variables.push(IntegerVariableProto {
                        name: format!("aux_matrix_eq_{}", aux_idx),
                        domain: vec![0, 1],
                    });
                    
                    cp_model.constraints.push(ConstraintProto {
                        name: String::new(),
                        enforcement_literal: vec![aux_idx],
                        constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                            vars: diff.vars.clone(),
                            coeffs: diff.coeffs.clone(),
                            domain: vec![0, 0],
                        })),
                    });
                    cp_model.constraints.push(ConstraintProto {
                        name: String::new(),
                        enforcement_literal: vec![-aux_idx - 1],
                        constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                            vars: diff.vars,
                            coeffs: diff.coeffs,
                            domain: vec![i64::MIN, -1, 1, i64::MAX],
                        })),
                    });
                    aux_vars.push(aux_idx);
                }
                
                for &xi in &aux_vars {
                    cp_model.constraints.push(ConstraintProto {
                        name: String::new(),
                        enforcement_literal: vec![],
                        constraint: Some(constraint_proto::Constraint::BoolOr(BoolArgumentProto {
                            literals: vec![-ref_var - 1, xi],
                        })),
                    });
                }
                let mut or_literals = aux_vars.iter().map(|&x| -x - 1).collect::<Vec<_>>();
                or_literals.push(ref_var);
                cp_model.constraints.push(ConstraintProto {
                    name: String::new(),
                    enforcement_literal: vec![],
                    constraint: Some(constraint_proto::Constraint::BoolOr(BoolArgumentProto {
                        literals: or_literals,
                })),
                });
                
                return Ok(exact_linear_constraint(LinearExpr { vars: vec![], coeffs: vec![], offset: 0 }, 0));
            }
        }
        Expression::Imply(_, lhs, rhs) => {
            let lhs_lit = get_literal(lhs.as_ref(), ctx)?;
            let rhs_lit = get_literal(rhs.as_ref(), ctx)?;
            
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![ref_var],
                constraint: Some(constraint_proto::Constraint::BoolOr(BoolArgumentProto {
                    literals: vec![-lhs_lit - 1, rhs_lit],
                })),
            });
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![-ref_var - 1],
                constraint: Some(constraint_proto::Constraint::BoolOr(BoolArgumentProto {
                    literals: vec![lhs_lit],
                })),
            });
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![-ref_var - 1],
                constraint: Some(constraint_proto::Constraint::BoolOr(BoolArgumentProto {
                    literals: vec![-rhs_lit - 1],
                })),
            });
            return Ok(exact_linear_constraint(LinearExpr { vars: vec![], coeffs: vec![], offset: 0 }, 0));
        }
        Expression::Neq(meta, lhs, rhs) => {
            if let (Some(elems_l), Some(elems_r)) = (expr_to_linear_list(lhs.as_ref(), ctx), expr_to_linear_list(rhs.as_ref(), ctx)) {
                if elems_l.len() != elems_r.len() {
                    return Err(SolverError::ModelInvalid("Matrix inequality with different lengths".into()));
                }
                let mut aux_vars = Vec::new();
                for (el, er) in elems_l.into_iter().zip(elems_r) {
                    let diff = subtract_linear_exprs(el, er);
                    let aux_idx = cp_model.variables.len() as i32;
                    cp_model.variables.push(IntegerVariableProto {
                        name: format!("aux_matrix_neq_{}", aux_idx),
                        domain: vec![0, 1],
                    });
                    
                    cp_model.constraints.push(ConstraintProto {
                        name: String::new(),
                        enforcement_literal: vec![aux_idx],
                        constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                            vars: diff.vars.clone(),
                            coeffs: diff.coeffs.clone(),
                            domain: vec![i64::MIN, -1, 1, i64::MAX],
                        })),
                    });
                    cp_model.constraints.push(ConstraintProto {
                        name: String::new(),
                        enforcement_literal: vec![-aux_idx - 1],
                        constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                            vars: diff.vars,
                            coeffs: diff.coeffs,
                            domain: vec![0, 0],
                        })),
                    });
                    aux_vars.push(aux_idx);
                }
                
                let mut or_literals = aux_vars.clone();
                or_literals.push(-ref_var - 1);
                cp_model.constraints.push(ConstraintProto {
                    name: String::new(),
                    enforcement_literal: vec![],
                    constraint: Some(constraint_proto::Constraint::BoolOr(BoolArgumentProto {
                        literals: or_literals,
                    })),
                });
                for &xi in &aux_vars {
                    cp_model.constraints.push(ConstraintProto {
                        name: String::new(),
                        enforcement_literal: vec![],
                        constraint: Some(constraint_proto::Constraint::BoolOr(BoolArgumentProto {
                            literals: vec![-xi - 1, ref_var],
                        })),
                    });
                }
                
                return Ok(exact_linear_constraint(LinearExpr { vars: vec![], coeffs: vec![], offset: 0 }, 0));
            }
        }
                Expression::MinionDivEqUndefZero(_, a, b, target) | Expression::MinionModuloEqUndefZero(_, a, b, target) => {
            let is_div = matches!(inner_expr, Expression::MinionDivEqUndefZero(..));
            let a_expr = expr_to_linear(&Expression::Atomic(Metadata::default(), a.as_ref().clone()), ctx)?;
            let b_expr = expr_to_linear(&Expression::Atomic(Metadata::default(), b.as_ref().clone()), ctx)?;
            let target_expr = expr_to_linear(&Expression::Atomic(Metadata::default(), target.as_ref().clone()), ctx)?;
            
            let target_aux = cp_model.variables.len() as i32;
            cp_model.variables.push(IntegerVariableProto {
                name: format!("div_target_aux_{}", target_aux),
                domain: vec![i32::MIN as i64, i32::MAX as i64],
            });
            let target_aux_expr = LinearExpr {
                vars: vec![target_aux],
                coeffs: vec![1],
                offset: 0,
            };
            
            let constraint = translate_div_mod_undef_zero(is_div, &a_expr, &b_expr, &target_aux_expr, cp_model)?;
            cp_model.constraints.push(constraint);
            
            let diff = subtract_linear_exprs(target_aux_expr, target_expr);
            let domain_true = vec![-diff.offset, -diff.offset];
            let domain_false = vec![i64::MIN, -diff.offset - 1, -diff.offset + 1, i64::MAX];

            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![ref_var],
                constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                    vars: diff.vars.clone(),
                    coeffs: diff.coeffs.clone(),
                    domain: domain_true,
                })),
            });
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![-ref_var - 1],
                constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                    vars: diff.vars,
                    coeffs: diff.coeffs,
                    domain: domain_false,
                })),
            });
            
            return Ok(exact_linear_constraint(LinearExpr { vars: vec![], coeffs: vec![], offset: 0 }, 0));
        }

        Expression::In(_, lhs, rhs) => {
            let lhs_linear = expr_to_linear(lhs.as_ref(), ctx)?;
            let vals = extract_set_values(rhs.as_ref()).ok_or_else(|| {
                SolverError::ModelFeatureNotSupported(format!("Unsupported In set: {:?}", rhs))
            })?;
            let domain_intervals = values_to_flat_domain(&vals);
            let shifted_domain = domain_intervals.into_iter().map(|v| v - lhs_linear.offset).collect::<Vec<_>>();

            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![ref_var],
                constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                    vars: lhs_linear.vars.clone(),
                    coeffs: lhs_linear.coeffs.clone(),
                    domain: shifted_domain.clone(),
                })),
            });

            let comp_intervals = complement_domain_intervals(&shifted_domain);
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![-ref_var - 1],
                constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                    vars: lhs_linear.vars,
                    coeffs: lhs_linear.coeffs,
                    domain: comp_intervals,
                })),
            });

            return Ok(exact_linear_constraint(LinearExpr { vars: vec![], coeffs: vec![], offset: 0 }, 0));
        }
        _ => {}
    }

    // Check for linear expressions using extract_linear_parts
    if let Some((lhs_expr, rhs_expr, domain_func)) = extract_linear_parts(inner_expr, ctx)? {
        let diff = subtract_linear_exprs(lhs_expr, rhs_expr);
        let domain_true = domain_func(diff.offset)?;
        let domain_false = complement_domain_intervals(&domain_true);

        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![ref_var],
            constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                vars: diff.vars.clone(),
                coeffs: diff.coeffs.clone(),
                domain: domain_true,
            })),
        });

        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![-ref_var - 1],
            constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                vars: diff.vars,
                coeffs: diff.coeffs,
                domain: domain_false,
            })),
        });

        return Ok(exact_linear_constraint(LinearExpr { vars: vec![], coeffs: vec![], offset: 0 }, 0));
    }

    // Rest of the match inner_expr
    match inner_expr {
        Expression::MinionElementOne(_, array, index, target) => {
            use super::proto::ElementConstraintProto;
            let index_linear = expr_to_linear(&Expression::Atomic(Metadata::default(), index.as_ref().clone()), ctx)?;
            let target_linear = expr_to_linear(&Expression::Atomic(Metadata::default(), target.as_ref().clone()), ctx)?;
            let index_1_var = get_or_create_var_for_linear(index_linear, cp_model);
            let target_var = get_or_create_var_for_linear(target_linear, cp_model);

            let mut element_vars = Vec::new();
            for elem in array {
                let elem_expr = Expression::Atomic(Metadata::default(), elem.clone());
                let elem_linear = expr_to_linear(&elem_expr, ctx)?;
                let elem_var = get_or_create_var_for_linear(elem_linear, cp_model);
                element_vars.push(elem_var);
            }

            let in_bounds_var = cp_model.variables.len() as i32;
            let mut in_bounds_proto = IntegerVariableProto::default();
            in_bounds_proto.domain = vec![0, 1];
            cp_model.variables.push(in_bounds_proto);

            let bounds_domain = vec![1, array.len() as i64];
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![in_bounds_var],
                constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                    vars: vec![index_1_var],
                    coeffs: vec![1],
                    domain: bounds_domain.clone(),
                })),
            });
            let bounds_comp = complement_domain_intervals(&bounds_domain);
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![-in_bounds_var - 1],
                constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                    vars: vec![index_1_var],
                    coeffs: vec![1],
                    domain: bounds_comp,
                })),
            });

            let index_0_var = cp_model.variables.len() as i32;
            let mut index_0_proto = IntegerVariableProto::default();
            index_0_proto.domain = vec![0, (array.len() - 1) as i64];
            cp_model.variables.push(index_0_proto);

            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![in_bounds_var],
                constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                    vars: vec![index_1_var, index_0_var],
                    coeffs: vec![1, -1],
                    domain: vec![1, 1],
                })),
            });

            let mut combined_domain = Vec::new();
            for &var in &element_vars {
                combined_domain.extend(cp_model.variables[var as usize].domain.clone());
            }
            let mut min_val = i64::MAX;
            let mut max_val = i64::MIN;
            for &val in &combined_domain {
                min_val = std::cmp::min(min_val, val);
                max_val = std::cmp::max(max_val, val);
            }
            if min_val > max_val {
                min_val = -1000000;
                max_val = 1000000;
            }

            let element_val_var = cp_model.variables.len() as i32;
            let mut element_val_proto = IntegerVariableProto::default();
            element_val_proto.domain = vec![min_val, max_val];
            cp_model.variables.push(element_val_proto);

            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::Element(ElementConstraintProto {
                    index: index_0_var,
                    target: element_val_var,
                    vars: element_vars,
                    linear_index: None,
                    linear_target: None,
                    exprs: vec![],
                })),
            });

            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![ref_var, in_bounds_var],
                constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                    vars: vec![element_val_var, target_var],
                    coeffs: vec![1, -1],
                    domain: vec![0, 0],
                })),
            });

            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![-ref_var - 1, in_bounds_var],
                constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                    vars: vec![element_val_var, target_var],
                    coeffs: vec![1, -1],
                    domain: vec![i64::MIN, -1, 1, i64::MAX],
                })),
            });

            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![-in_bounds_var - 1],
                constraint: Some(constraint_proto::Constraint::BoolOr(BoolArgumentProto {
                    literals: vec![-ref_var - 1],
                })),
            });

            Ok(exact_linear_constraint(LinearExpr { vars: vec![], coeffs: vec![], offset: 0 }, 0))
        }
        Expression::InDomain(_, var_expr, domain) => {
            let var_linear = expr_to_linear(var_expr.as_ref(), ctx)?;
            let var_idx = get_or_create_var_for_linear(var_linear, cp_model);
            let resolved_domain = domain.resolve();
            let domain = resolved_domain.as_deref().ok_or_else(|| {
                SolverError::ModelInvalid("InDomain without resolvable domain".into())
            })?;
            let domain_intervals = extract_domain_intervals(domain)?;

            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![ref_var],
                constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                    vars: vec![var_idx],
                    coeffs: vec![1],
                    domain: domain_intervals.clone(),
                })),
            });

            let comp_intervals = complement_domain_intervals(&domain_intervals);
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![-ref_var - 1],
                constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                    vars: vec![var_idx],
                    coeffs: vec![1],
                    domain: comp_intervals,
                })),
            });

            Ok(exact_linear_constraint(LinearExpr { vars: vec![], coeffs: vec![], offset: 0 }, 0))
        }
        Expression::FlatAllDiff(_, vars) => {
            let mut exprs = Vec::new();
            for var in vars {
                let var_expr = expr_to_linear(&Expression::Atomic(Metadata::default(), var.clone()), ctx)?;
                exprs.push(var_expr);
            }
            
            let mut pair_eq_literals = Vec::new();
            for i in 0..exprs.len() {
                for j in (i + 1)..exprs.len() {
                    let eq_lit = cp_model.variables.len() as i32;
                    cp_model.variables.push(IntegerVariableProto {
                        name: format!("alldiff_eq_{}_{}", i, j),
                        domain: vec![0, 1],
                    });
                    pair_eq_literals.push(eq_lit);

                    let mut diff_vars = exprs[i].vars.clone();
                    let mut diff_coeffs = exprs[i].coeffs.clone();
                    for (v, c) in exprs[j].vars.iter().zip(exprs[j].coeffs.iter()) {
                        diff_vars.push(*v);
                        diff_coeffs.push(-c);
                    }
                    let diff_offset = exprs[i].offset - exprs[j].offset;

                    // 1. eq_lit => exprs[i] == exprs[j]
                    cp_model.constraints.push(ConstraintProto {
                        name: String::new(),
                        enforcement_literal: vec![eq_lit],
                        constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                            vars: diff_vars.clone(),
                            coeffs: diff_coeffs.clone(),
                            domain: vec![-diff_offset, -diff_offset],
                        })),
                    });

                    // 2. ref_var => exprs[i] != exprs[j]
                    cp_model.constraints.push(ConstraintProto {
                        name: String::new(),
                        enforcement_literal: vec![ref_var],
                        constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                            vars: diff_vars,
                            coeffs: diff_coeffs,
                            // x_i - x_j != 0 translates to domain [MIN, -1] U [1, MAX]
                            domain: vec![i64::MIN, -diff_offset - 1, -diff_offset + 1, i64::MAX],
                        })),
                    });
                }
            }

            // 3. !ref_var => Or(pair_eq_literals) -> equivalent to ref_var \/ Or(pair_eq_literals)
            let mut or_literals = pair_eq_literals;
            or_literals.push(ref_var);
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::BoolOr(BoolArgumentProto {
                    literals: or_literals,
                })),
            });

            Ok(exact_linear_constraint(LinearExpr { vars: vec![], coeffs: vec![], offset: 0 }, 0))
        }
        _ => {
            if let Ok(lit) = get_literal(inner_expr, ctx) {
                equate_literal_and_var(ref_var, lit, cp_model);
                return Ok(exact_linear_constraint(LinearExpr { vars: vec![], coeffs: vec![], offset: 0 }, 0));
            }
            
            Err(SolverError::ModelFeatureNotSupported(format!(
                "Unsupported expression inside AuxDeclaration: {:?}",
                inner_expr
            )))
        }
    }
}

fn translate_aux_declaration(
    reference: &crate::ast::Reference,
    inner_expr: &Expression,
    cp_model: &mut CpModelProto,
    ctx: &TranslationContext,
) -> SolverResult<ConstraintProto> {
    let ref_var = get_literal(&Expression::Atomic(Metadata::default(), Atom::Reference(reference.clone())), ctx)?;
    if let Ok(inner_linear) = expr_to_linear(inner_expr, ctx) {
        let ref_linear = LinearExpr {
            vars: vec![ref_var],
            coeffs: vec![1],
            offset: 0,
        };
        let diff = subtract_linear_exprs(ref_linear, inner_linear);
        return Ok(exact_linear_constraint(diff, 0));
    }
    translate_reified_constraint(ref_var, inner_expr, cp_model, ctx)
}

fn translate_pow_constraint(
    a: &Expression,
    b: &Expression,
    target: &Expression,
    cp_model: &mut CpModelProto,
    ctx: &TranslationContext,
) -> SolverResult<ConstraintProto> {
    let a_expr = expr_to_linear(a, ctx)?;
    let b_expr = expr_to_linear(b, ctx)?;
    let target_expr = expr_to_linear(target, ctx)?;

    let a_vals = if a_expr.vars.is_empty() {
        vec![a_expr.offset]
    } else if a_expr.vars.len() == 1 && a_expr.coeffs == vec![1] && a_expr.offset == 0 {
        let var = a_expr.vars[0];
        get_domain_values(&cp_model.variables[var as usize].domain)
    } else {
        return Err(SolverError::ModelFeatureNotSupported("Complex base expression in Pow not supported".into()));
    };

    let b_vals = if b_expr.vars.is_empty() {
        vec![b_expr.offset]
    } else if b_expr.vars.len() == 1 && b_expr.coeffs == vec![1] && b_expr.offset == 0 {
        let var = b_expr.vars[0];
        get_domain_values(&cp_model.variables[var as usize].domain)
    } else {
        return Err(SolverError::ModelFeatureNotSupported("Complex exponent expression in Pow not supported".into()));
    };

    let target_domain = if target_expr.vars.is_empty() {
        vec![target_expr.offset, target_expr.offset]
    } else if target_expr.vars.len() == 1 && target_expr.coeffs == vec![1] && target_expr.offset == 0 {
        let var = target_expr.vars[0];
        cp_model.variables[var as usize].domain.clone()
    } else {
        return Err(SolverError::ModelFeatureNotSupported("Complex target expression in Pow not supported".into()));
    };

    let mut vars = Vec::new();
    let mut active_cols = Vec::new();

    if !a_expr.vars.is_empty() {
        active_cols.push(0);
        vars.push(a_expr.vars[0]);
    }
    if !b_expr.vars.is_empty() {
        active_cols.push(1);
        vars.push(b_expr.vars[0]);
    }
    if !target_expr.vars.is_empty() {
        active_cols.push(2);
        vars.push(target_expr.vars[0]);
    }

    let mut values = Vec::new();
    let mut matched_any = false;
    for &val_a in &a_vals {
        for &val_b in &b_vals {
            if val_b < 0 {
                continue;
            }
            if let Some(val_target) = checked_pow(val_a, val_b) {
                if domain_contains(&target_domain, val_target) {
                    matched_any = true;
                    for &col in &active_cols {
                        match col {
                            0 => values.push(val_a),
                            1 => values.push(val_b),
                            2 => values.push(val_target),
                            _ => unreachable!(),
                        }
                    }
                }
            }
        }
    }

    if vars.is_empty() {
        let target_val = if matched_any { 0 } else { 1 };
        return Ok(exact_linear_constraint(LinearExpr { vars: vec![], coeffs: vec![], offset: 0 }, target_val));
    }

    use super::proto::TableConstraintProto;
    let table_constraint = TableConstraintProto {
        vars,
        values,
        exprs: vec![],
        negated: false,
    };
    Ok(ConstraintProto {
        name: String::new(),
        enforcement_literal: vec![],
        constraint: Some(constraint_proto::Constraint::Table(table_constraint)),
    })
}

fn translate_iff_constraint(
    lhs: &Expression,
    rhs: &Expression,
    cp_model: &mut CpModelProto,
    ctx: &TranslationContext,
) -> SolverResult<ConstraintProto> {
    let (target_lit, expr) = if let Ok(lit) = get_literal(lhs, ctx) {
        (lit, rhs)
    } else if let Ok(lit) = get_literal(rhs, ctx) {
        (lit, lhs)
    } else {
        return Err(SolverError::ModelFeatureNotSupported(
            "Complex Iff constraints not supported".into(),
        ));
    };

    match expr {
        Expression::And(_, inner) => {
            let Expression::AbstractLiteral(_, AbstractLiteral::Matrix(elems, _)) = inner.as_ref() else {
                return Err(SolverError::ModelFeatureNotSupported("Unsupported And in Iff".into()));
            };
            let mut literals = Vec::new();
            for elem in elems {
                literals.push(get_literal(elem, ctx)?);
            }
            for &lit in &literals {
                cp_model.constraints.push(ConstraintProto {
                    name: String::new(),
                    enforcement_literal: vec![target_lit],
                    constraint: Some(constraint_proto::Constraint::BoolOr(BoolArgumentProto {
                        literals: vec![lit],
                    })),
                });
            }
            let mut or_lits: Vec<i32> = literals.iter().map(|&lit| -lit - 1).collect();
            or_lits.push(target_lit);
            Ok(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::BoolOr(BoolArgumentProto {
                    literals: or_lits,
                })),
            })
        }
        Expression::Or(_, inner) => {
            let Expression::AbstractLiteral(_, AbstractLiteral::Matrix(elems, _)) = inner.as_ref() else {
                return Err(SolverError::ModelFeatureNotSupported("Unsupported Or in Iff".into()));
            };
            let mut literals = Vec::new();
            for elem in elems {
                literals.push(get_literal(elem, ctx)?);
            }
            for &lit in &literals {
                cp_model.constraints.push(ConstraintProto {
                    name: String::new(),
                    enforcement_literal: vec![lit],
                    constraint: Some(constraint_proto::Constraint::BoolOr(BoolArgumentProto {
                        literals: vec![target_lit],
                    })),
                });
            }
            let mut or_lits = literals.clone();
            or_lits.push(-target_lit - 1);
            Ok(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::BoolOr(BoolArgumentProto {
                    literals: or_lits,
                })),
            })
        }
        _ => {
            let other_lit = get_literal(expr, ctx)?;
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![target_lit],
                constraint: Some(constraint_proto::Constraint::BoolOr(BoolArgumentProto {
                    literals: vec![other_lit],
                })),
            });
            Ok(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![other_lit],
                constraint: Some(constraint_proto::Constraint::BoolOr(BoolArgumentProto {
                    literals: vec![target_lit],
                })),
            })
        }
    }
}

/// Main dispatcher: takes a Conjure constraint, extracts LHS and RHS, linearizes them, and builds a Protobuf constraint.
fn translate_constraint(expr: &Expression, cp_model: &mut CpModelProto, ctx: &TranslationContext) -> SolverResult<ConstraintProto> {
    match expr {
        // Top-level boolean constraints must evaluate to true.
        Expression::Atomic(_, Atom::Literal(Literal::Bool(_)))
        | Expression::Atomic(_, Atom::Reference(_)) => {
            return Ok(exact_linear_constraint(expr_to_linear(expr, ctx)?, 1));
        }
        Expression::Not(_, inner) => {
            return Ok(exact_linear_constraint(expr_to_linear(inner, ctx)?, 0));
        }
        Expression::AuxDeclaration(_, reference, inner_expr) => {
            return translate_aux_declaration(reference, inner_expr.as_ref(), cp_model, ctx);
        }
        Expression::Imply(_, lhs, rhs) => {
            let enforcement_lit = get_literal(lhs.as_ref(), ctx)?;
            let mut constraint = translate_constraint(rhs.as_ref(), cp_model, ctx)?;
            constraint.enforcement_literal.push(enforcement_lit);
            return Ok(constraint);
        }
        _ => {}
    }

    // 1. Matrix Equality/Inequality, Neq, and In constraints
    match expr {
        Expression::Eq(meta, lhs, rhs) => {
            if let (Some(elems_l), Some(elems_r)) = (expr_to_linear_list(lhs.as_ref(), ctx), expr_to_linear_list(rhs.as_ref(), ctx)) {
                if elems_l.len() != elems_r.len() {
                    return Err(SolverError::ModelInvalid("Matrix equality with different lengths".into()));
                }
                for (el, er) in elems_l.into_iter().zip(elems_r) {
                    let diff = subtract_linear_exprs(el, er);
                    cp_model.constraints.push(exact_linear_constraint(diff, 0));
                }
                return Ok(exact_linear_constraint(LinearExpr { vars: vec![], coeffs: vec![], offset: 0 }, 0));
            }
        }
        Expression::Neq(meta, lhs, rhs) => {
            if let (Some(elems_l), Some(elems_r)) = (expr_to_linear_list(lhs.as_ref(), ctx), expr_to_linear_list(rhs.as_ref(), ctx)) {
                if elems_l.len() != elems_r.len() {
                    return Err(SolverError::ModelInvalid("Matrix inequality with different lengths".into()));
                }
                let mut aux_vars = Vec::new();
                for (el, er) in elems_l.into_iter().zip(elems_r) {
                    let diff = subtract_linear_exprs(el, er);
                    let aux_idx = cp_model.variables.len() as i32;
                    cp_model.variables.push(IntegerVariableProto {
                        name: format!("aux_matrix_neq_{}", aux_idx),
                        domain: vec![0, 1],
                    });
                    
                    let domain_true = vec![i64::MIN, -1, 1, i64::MAX];
                    let domain_false = vec![0, 0];
                    
                    cp_model.constraints.push(ConstraintProto {
                        name: String::new(),
                        enforcement_literal: vec![aux_idx],
                        constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                            vars: diff.vars.clone(),
                            coeffs: diff.coeffs.clone(),
                            domain: domain_true,
                        })),
                    });

                    cp_model.constraints.push(ConstraintProto {
                        name: String::new(),
                        enforcement_literal: vec![-aux_idx - 1],
                        constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                            vars: diff.vars,
                            coeffs: diff.coeffs,
                            domain: domain_false,
                        })),
                    });
                    
                    aux_vars.push(aux_idx);
                }
                return Ok(ConstraintProto {
                    name: String::new(),
                    enforcement_literal: vec![],
                    constraint: Some(constraint_proto::Constraint::BoolOr(BoolArgumentProto {
                        literals: aux_vars,
                    })),
                });
            }
            
            // Otherwise, fallback to AllDiff for scalar Neq:
            use super::proto::AllDifferentConstraintProto;
            let lhs_linear = expr_to_linear(lhs.as_ref(), ctx)?;
            let rhs_linear = expr_to_linear(rhs.as_ref(), ctx)?;
            return Ok(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::AllDiff(AllDifferentConstraintProto {
                    exprs: vec![
                        super::proto::LinearExpressionProto {
                            vars: lhs_linear.vars,
                            coeffs: lhs_linear.coeffs,
                            offset: lhs_linear.offset,
                        },
                        super::proto::LinearExpressionProto {
                            vars: rhs_linear.vars,
                            coeffs: rhs_linear.coeffs,
                            offset: rhs_linear.offset,
                        },
                    ],
                })),
            });
        }
        Expression::In(_, lhs, rhs) => {
            let lhs_linear = expr_to_linear(lhs.as_ref(), ctx)?;
            let vals = extract_set_values(rhs.as_ref()).ok_or_else(|| {
                SolverError::ModelFeatureNotSupported(format!("Unsupported In set: {:?}", rhs))
            })?;
            let domain = values_to_flat_domain(&vals);
            let shifted_domain = domain.into_iter().map(|v| v - lhs_linear.offset).collect::<Vec<_>>();
            
            return Ok(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                    vars: lhs_linear.vars,
                    coeffs: lhs_linear.coeffs,
                    domain: shifted_domain,
                })),
            });
        }
        _ => {}
    }

    if let Expression::Eq(_, lhs, rhs) = expr {
        if let Ok(ref_var) = get_literal(lhs.as_ref(), ctx) {
            if matches!(rhs.as_ref(), Expression::FlatAllDiff(_, _)) {
                return translate_reified_constraint(ref_var, rhs.as_ref(), cp_model, ctx);
            }
        } else if let Ok(ref_var) = get_literal(rhs.as_ref(), ctx) {
            if matches!(lhs.as_ref(), Expression::FlatAllDiff(_, _)) {
                return translate_reified_constraint(ref_var, lhs.as_ref(), cp_model, ctx);
            }
        }
    }

    // 2. Linear constraints using extract_linear_parts helper
    if let Some((lhs_expr, rhs_expr, domain_func)) = extract_linear_parts(expr, ctx)? {
        let linear_expr = subtract_linear_exprs(lhs_expr, rhs_expr);
        let domain = domain_func(linear_expr.offset)?;

        return Ok(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![],
            constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                vars: linear_expr.vars,
                coeffs: linear_expr.coeffs,
                domain,
            })),
        });
    }

    match expr {
        Expression::FlatAbsEq(_, a, b) => {
            // a = |b| 
            // In CP-SAT, LinMax(target=a, exprs=[b, -b])
            let a_expr = expr_to_linear(&Expression::Atomic(Metadata::default(), a.as_ref().clone()), ctx)?;
            let b_expr = expr_to_linear(&Expression::Atomic(Metadata::default(), b.as_ref().clone()), ctx)?;
            
            let mut minus_b_expr = b_expr.clone();
            for c in &mut minus_b_expr.coeffs { *c = -*c; }
            minus_b_expr.offset = -minus_b_expr.offset;

            use super::proto::{LinearExpressionProto, LinearArgumentProto};
            return Ok(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::LinMax(LinearArgumentProto {
                    target: Some(LinearExpressionProto {
                        vars: a_expr.vars,
                        coeffs: a_expr.coeffs,
                        offset: a_expr.offset,
                    }),
                    exprs: vec![
                        LinearExpressionProto {
                            vars: b_expr.vars,
                            coeffs: b_expr.coeffs,
                            offset: b_expr.offset,
                        },
                        LinearExpressionProto {
                            vars: minus_b_expr.vars,
                            coeffs: minus_b_expr.coeffs,
                            offset: minus_b_expr.offset,
                        },
                    ]
                }))
            });
        },
        Expression::FlatAllDiff(_, vars) => {
            use super::proto::AllDifferentConstraintProto;
            let mut exprs = Vec::new();
            for var in vars {
                let var_expr = expr_to_linear(&Expression::Atomic(Metadata::default(), var.clone()), ctx)?;
                exprs.push(super::proto::LinearExpressionProto {
                    vars: var_expr.vars,
                    coeffs: var_expr.coeffs,
                    offset: var_expr.offset,
                });
            }
            return Ok(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::AllDiff(AllDifferentConstraintProto {
                    exprs,
                })),
            });
        },
        Expression::Table(_, tuple, allowed_rows) => {
            return translate_table_constraint(false, tuple.as_ref(), allowed_rows.as_ref(), ctx);
        }
        Expression::NegativeTable(_, tuple, forbidden_rows) => {
            return translate_table_constraint(true, tuple.as_ref(), forbidden_rows.as_ref(), ctx);
        }
        Expression::MinionPow(_, a, b, target) => {
            let a_expr = expr_to_linear(&Expression::Atomic(Metadata::default(), a.as_ref().clone()), ctx)?;
            let b_expr = expr_to_linear(&Expression::Atomic(Metadata::default(), b.as_ref().clone()), ctx)?;
            let target_expr = expr_to_linear(&Expression::Atomic(Metadata::default(), target.as_ref().clone()), ctx)?;

            let a_vals = if a_expr.vars.is_empty() {
                vec![a_expr.offset]
            } else if a_expr.vars.len() == 1 && a_expr.coeffs == vec![1] && a_expr.offset == 0 {
                let var = a_expr.vars[0];
                get_domain_values(&cp_model.variables[var as usize].domain)
            } else {
                return Err(SolverError::ModelFeatureNotSupported("Complex base expression in Pow not supported".into()));
            };

            let b_vals = if b_expr.vars.is_empty() {
                vec![b_expr.offset]
            } else if b_expr.vars.len() == 1 && b_expr.coeffs == vec![1] && b_expr.offset == 0 {
                let var = b_expr.vars[0];
                get_domain_values(&cp_model.variables[var as usize].domain)
            } else {
                return Err(SolverError::ModelFeatureNotSupported("Complex exponent expression in Pow not supported".into()));
            };

            let target_domain = if target_expr.vars.is_empty() {
                vec![target_expr.offset, target_expr.offset]
            } else if target_expr.vars.len() == 1 && target_expr.coeffs == vec![1] && target_expr.offset == 0 {
                let var = target_expr.vars[0];
                cp_model.variables[var as usize].domain.clone()
            } else {
                return Err(SolverError::ModelFeatureNotSupported("Complex target expression in Pow not supported".into()));
            };

            // Identify active (non-constant) columns
            let mut vars = Vec::new();
            let mut active_cols = Vec::new(); // 0 for a, 1 for b, 2 for target

            if !a_expr.vars.is_empty() {
                active_cols.push(0);
                vars.push(a_expr.vars[0]);
            }
            if !b_expr.vars.is_empty() {
                active_cols.push(1);
                vars.push(b_expr.vars[0]);
            }
            if !target_expr.vars.is_empty() {
                active_cols.push(2);
                vars.push(target_expr.vars[0]);
            }

            let mut values = Vec::new();
            let mut matched_any = false;
            for &val_a in &a_vals {
                for &val_b in &b_vals {
                    if val_b < 0 {
                        continue; // Pow is undefined for negative exponents in CP
                    }
                    if let Some(val_target) = checked_pow(val_a, val_b) {
                        if domain_contains(&target_domain, val_target) {
                            matched_any = true;
                            // Only push values for active variables
                            for &col in &active_cols {
                                match col {
                                    0 => values.push(val_a),
                                    1 => values.push(val_b),
                                    2 => values.push(val_target),
                                    _ => unreachable!(),
                                }
                            }
                        }
                    }
                }
            }

            if vars.is_empty() {
                let target_val = if matched_any { 0 } else { 1 };
                return Ok(exact_linear_constraint(LinearExpr { vars: vec![], coeffs: vec![], offset: 0 }, target_val));
            }

            let table_constraint = super::proto::TableConstraintProto {
                vars,
                values,
                exprs: vec![],
                negated: false,
            };
            return Ok(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::Table(table_constraint)),
            });
        }
        Expression::MinionDivEqUndefZero(_, a, b, target) => {
            let a_expr = expr_to_linear(&Expression::Atomic(Metadata::default(), a.as_ref().clone()), ctx)?;
            let b_expr = expr_to_linear(&Expression::Atomic(Metadata::default(), b.as_ref().clone()), ctx)?;
            let target_expr = expr_to_linear(&Expression::Atomic(Metadata::default(), target.as_ref().clone()), ctx)?;
            let constraint = translate_div_mod_undef_zero(true, &a_expr, &b_expr, &target_expr, cp_model)?;
            return Ok(constraint);
        }
        Expression::MinionModuloEqUndefZero(_, a, b, target) => {
            let a_expr = expr_to_linear(&Expression::Atomic(Metadata::default(), a.as_ref().clone()), ctx)?;
            let b_expr = expr_to_linear(&Expression::Atomic(Metadata::default(), b.as_ref().clone()), ctx)?;
            let target_expr = expr_to_linear(&Expression::Atomic(Metadata::default(), target.as_ref().clone()), ctx)?;
            let constraint = translate_div_mod_undef_zero(false, &a_expr, &b_expr, &target_expr, cp_model)?;
            return Ok(constraint);
        }
        Expression::FlatProductEq(_, a, b, target) => {
            use super::proto::{LinearArgumentProto, LinearExpressionProto};
            let a_expr = expr_to_linear(&Expression::Atomic(Metadata::default(), a.as_ref().clone()), ctx)?;
            let b_expr = expr_to_linear(&Expression::Atomic(Metadata::default(), b.as_ref().clone()), ctx)?;
            let target_expr = expr_to_linear(&Expression::Atomic(Metadata::default(), target.as_ref().clone()), ctx)?;
            return Ok(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::IntProd(LinearArgumentProto {
                    target: Some(LinearExpressionProto {
                        vars: target_expr.vars,
                        coeffs: target_expr.coeffs,
                        offset: target_expr.offset,
                    }),
                    exprs: vec![
                        LinearExpressionProto {
                            vars: a_expr.vars,
                            coeffs: a_expr.coeffs,
                            offset: a_expr.offset,
                        },
                        LinearExpressionProto {
                            vars: b_expr.vars,
                            coeffs: b_expr.coeffs,
                            offset: b_expr.offset,
                        },
                    ],
                })),
            });
        },
        Expression::Or(_, inner) => {
            let Expression::AbstractLiteral(_, AbstractLiteral::Matrix(elems, _)) = inner.as_ref() else {
                return Err(SolverError::ModelFeatureNotSupported(format!(
                    "Unsupported Or argument in constraint: {:?}",
                    inner
                )));
            };
            let mut literals = Vec::new();
            for elem in elems {
                literals.push(get_literal(elem, ctx)?);
            }
            return Ok(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::BoolOr(BoolArgumentProto {
                    literals,
                })),
            });
        },
        Expression::And(_, inner) => {
            let Expression::AbstractLiteral(_, AbstractLiteral::Matrix(elems, _)) = inner.as_ref() else {
                return Err(SolverError::ModelFeatureNotSupported(format!(
                    "Unsupported And argument in constraint: {:?}",
                    inner
                )));
            };
            let mut literals = Vec::new();
            for elem in elems {
                literals.push(get_literal(elem, ctx)?);
            }
            return Ok(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::BoolAnd(BoolArgumentProto {
                    literals,
                })),
            });
        },
        Expression::MinionElementOne(_, array, index, target) => {
            use super::proto::ElementConstraintProto;
            let index_linear = expr_to_linear(&Expression::Atomic(Metadata::default(), index.as_ref().clone()), ctx)?;
            let target_linear = expr_to_linear(&Expression::Atomic(Metadata::default(), target.as_ref().clone()), ctx)?;

            let index_1_var = get_or_create_var_for_linear(index_linear, cp_model);
            let target_var = get_or_create_var_for_linear(target_linear, cp_model);

            let mut element_vars = Vec::new();
            for elem in array {
                let elem_expr = Expression::Atomic(Metadata::default(), elem.clone());
                let elem_linear = expr_to_linear(&elem_expr, ctx)?;
                let elem_var = get_or_create_var_for_linear(elem_linear, cp_model);
                element_vars.push(elem_var);
            }

            let index_0_var = cp_model.variables.len() as i32;
            let mut index_0_proto = IntegerVariableProto::default();
            index_0_proto.domain = vec![0, (array.len() - 1) as i64];
            cp_model.variables.push(index_0_proto);

            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                    vars: vec![index_1_var, index_0_var],
                    coeffs: vec![1, -1],
                    domain: vec![1, 1],
                })),
            });

            return Ok(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::Element(ElementConstraintProto {
                    index: index_0_var,
                    target: target_var,
                    vars: element_vars,
                    linear_index: None,
                    linear_target: None,
                    exprs: vec![],
                })),
            });
        }
        Expression::InDomain(_, var_expr, domain) => {
            let var_linear = expr_to_linear(var_expr.as_ref(), ctx)?;
            let var_idx = get_or_create_var_for_linear(var_linear, cp_model);
            let resolved_domain = domain.resolve();
            let domain = resolved_domain.as_deref().ok_or_else(|| {
                SolverError::ModelInvalid("InDomain without resolvable domain".into())
            })?;
            let domain_intervals = extract_domain_intervals(domain)?;
            return Ok(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                    vars: vec![var_idx],
                    coeffs: vec![1],
                    domain: domain_intervals,
                })),
            });
        }
        Expression::Iff(_, lhs, rhs) => {
            return translate_iff_constraint(lhs.as_ref(), rhs.as_ref(), cp_model, ctx);
        }
        Expression::Atomic(_, Atom::Literal(Literal::Bool(val))) => {
            if *val {
                return Ok(exact_linear_constraint(LinearExpr { vars: vec![], coeffs: vec![], offset: 0 }, 0));
            } else {
                return Ok(ConstraintProto {
                    name: String::new(),
                    enforcement_literal: vec![],
                    constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                        vars: vec![],
                        coeffs: vec![],
                        domain: vec![1, 0], // Unsatisfiable domain
                    })),
                });
            }
        }
        _ => {
            return Err(SolverError::ModelFeatureNotSupported(format!(
                "Unsupported top-level constraint: {expr:?}"
            )))
        }
    }
}


#[derive(Clone)]
pub(super) struct SolutionVar {
    pub name: Name,
    pub is_bool: bool,
}

/// Entry point for translation: iterates over all variables and constraints to build the final CpModelProto.
pub(super) fn model_to_cp_sat(model: Model) -> SolverResult<(CpModelProto, Vec<SolutionVar>)> {
    let mut cp_model = CpModelProto::default();
    let mut solution_vars = Vec::new();
    let mut ctx = TranslationContext {
        var_mapping: HashMap::new(),
    };

    for (name, decl) in model.symbols().iter_local() {
        if let Some(find_var) = decl.as_find() {
            if !model
                .symbols()
                .representations_for(&name)
                .is_none_or(|x| x.is_empty())
            {
                continue;
            }
            let mut var_proto = IntegerVariableProto::default();
            var_proto.name = name.to_string();

            let resolved_domain = find_var.domain_of().resolve();
            let domain = resolved_domain.as_deref().ok_or_else(|| {
                SolverError::ModelInvalid(format!("Variable {} without resolvable domain", name))
            })?;
            
            var_proto.domain = extract_domain_intervals(domain)?;

            let is_bool = matches!(domain, GroundDomain::Bool);
            solution_vars.push(SolutionVar {
                name: name.clone(),
                is_bool,
            });

            let var_index = cp_model.variables.len() as i32;
            cp_model.variables.push(var_proto);
            
            ctx.var_mapping.insert(name.clone(), var_index);
        }
    }

    for constraint in model.constraints() {
        let constraint_proto = translate_constraint(constraint, &mut cp_model, &ctx)?;
        cp_model.constraints.push(constraint_proto);
    }

    Ok((cp_model, solution_vars))
}

/// Reverse mapping: takes the raw integer array from C++ and maps the values back to the original Conjure variable names.
pub(super) fn response_to_solution(
    response: &CpSolverResponse,
    solution_vars: &[SolutionVar],
) -> Result<std::collections::HashMap<Name, Literal>, SolverError> {
    if response.solution.len() < solution_vars.len() {
        return Err(SolverError::Runtime(format!(
            "OR-Tools returned {} values for {} decision variables",
            response.solution.len(),
            solution_vars.len()
        )));
    }

    let mut solution = std::collections::HashMap::with_capacity(solution_vars.len());
    for (var, value) in solution_vars.iter().zip(response.solution.iter()) {
        let literal = if var.is_bool {
            Literal::Bool(*value != 0)
        } else {
            Literal::Int(*value as i32)
        };
        solution.insert(var.name.clone(), literal);
    }

    Ok(solution)
}

