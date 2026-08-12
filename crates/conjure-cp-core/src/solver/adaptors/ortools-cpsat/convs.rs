use super::proto::{
    BoolArgumentProto, ConstraintProto, CpModelProto, CpObjectiveProto, CpSolverResponse,
    DecisionStrategyProto, IntegerVariableProto, LinearConstraintProto, constraint_proto,
};
use crate::Model;
use crate::ast::{
    AbstractLiteral, Atom, Expression, GroundDomain, HasDomain, Literal, Metadata, Name,
    OptimiseDirection, Range, eval_constant,
};
use crate::solver::{SolverError, SolverResult};
use std::cell::RefCell;
use std::collections::HashMap;
use ustr::Ustr;

struct TranslationContext {
    var_mapping: RefCell<HashMap<Name, i32>>,
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
                    Range::UnboundedL(_) | Range::UnboundedR(_) | Range::Unbounded => {
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
        Expression::Atomic(
            _,
            Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Set(vals))),
        ) => {
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
    if let Some(&idx) = ctx.var_mapping.borrow().get(src_var) {
        return vec![idx];
    }
    let mut matching_vars = Vec::new();
    let src_str = src_var.to_string();
    let src_base = src_str.split('#').next().unwrap_or(&src_str);

    let var_map = ctx.var_mapping.borrow();
    for (name, &idx) in var_map.iter() {
        let name_str = name.to_string();
        if let Name::Represented(box_tuple) = name {
            let (ref_var, repr_name, suffix) = box_tuple.as_ref();
            let ref_str = ref_var.to_string();
            let ref_base = ref_str.split('#').next().unwrap_or(&ref_str);

            if (ref_var == src_var || ref_str == src_str || ref_base == src_base)
                && repr_name.as_str() == "matrix_to_atom"
            {
                let parts: Vec<i32> = suffix
                    .split('_')
                    .filter_map(|s| s.parse::<i32>().ok())
                    .collect();
                matching_vars.push((parts, idx));
            }
        } else if name_str.starts_with(&(src_str.clone() + "_"))
            || name_str.starts_with(&(src_base.to_string() + "#matrix_to_atom_"))
        {
            let suffix = name_str.split("matrix_to_atom_").last().unwrap_or("");
            let parts: Vec<i32> = suffix
                .split('_')
                .filter_map(|s| s.parse::<i32>().ok())
                .collect();
            matching_vars.push((parts, idx));
        }
    }
    matching_vars.sort_by(|(indices_a, _), (indices_b, _)| indices_a.cmp(indices_b));
    matching_vars.into_iter().map(|(_, idx)| idx).collect()
}

fn expr_to_linear_list(expr: &Expression, ctx: &TranslationContext) -> Option<Vec<LinearExpr>> {
    match expr {
        Expression::Atomic(_, Atom::Reference(reference)) => {
            let name = reference.name();
            let vars = get_matrix_element_vars(&name, ctx);
            if !vars.is_empty() {
                Some(
                    vars.into_iter()
                        .map(|idx| LinearExpr {
                            vars: vec![idx],
                            coeffs: vec![1],
                            offset: 0,
                        })
                        .collect(),
                )
            } else if let Ok(linear) = expr_to_linear(expr, ctx) {
                Some(vec![linear])
            } else if let Some(constant_literal) = reference.resolve_constant() {
                expr_to_linear_list(
                    &Expression::Atomic(Metadata::default(), Atom::Literal(constant_literal)),
                    ctx,
                )
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
        Expression::Atomic(
            _,
            Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Matrix(elems, _))),
        ) => {
            let mut list = Vec::new();
            for elem in elems {
                let expr = Expression::Atomic(Metadata::default(), Atom::Literal(elem.clone()));
                if let Some(sub_list) = expr_to_linear_list(&expr, ctx) {
                    list.extend(sub_list);
                } else if let Ok(lin) = expr_to_linear(&expr, ctx) {
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
) -> SolverResult<
    Option<(
        LinearExpr,
        LinearExpr,
        Box<dyn Fn(i64) -> SolverResult<Vec<i64>>>,
    )>,
> {
    let result = match expr {
        Expression::Eq(_, lhs, rhs) | Expression::Iff(_, lhs, rhs) => {
            let (Ok(lhs_lin), Ok(rhs_lin)) =
                (expr_to_linear(lhs.as_ref(), ctx), expr_to_linear(rhs.as_ref(), ctx))
            else {
                return Ok(None);
            };
            (
                lhs_lin,
                rhs_lin,
                Box::new(|offset: i64| Ok(vec![-offset, -offset]))
                    as Box<dyn Fn(i64) -> SolverResult<Vec<i64>>>,
            )
        }
        Expression::Neq(_, lhs, rhs) => {
            let (Ok(lhs_lin), Ok(rhs_lin)) =
                (expr_to_linear(lhs.as_ref(), ctx), expr_to_linear(rhs.as_ref(), ctx))
            else {
                return Ok(None);
            };
            (
                lhs_lin,
                rhs_lin,
                Box::new(|offset: i64| Ok(vec![i64::MIN, -offset - 1, -offset + 1, i64::MAX]))
                    as Box<dyn Fn(i64) -> SolverResult<Vec<i64>>>,
            )
        }
        Expression::Leq(_, lhs, rhs) => {
            let (Ok(lhs_lin), Ok(rhs_lin)) =
                (expr_to_linear(lhs.as_ref(), ctx), expr_to_linear(rhs.as_ref(), ctx))
            else {
                return Ok(None);
            };
            (
                lhs_lin,
                rhs_lin,
                Box::new(|offset: i64| Ok(vec![i64::MIN, -offset]))
                    as Box<dyn Fn(i64) -> SolverResult<Vec<i64>>>,
            )
        }
        Expression::Geq(_, lhs, rhs) => {
            let (Ok(lhs_lin), Ok(rhs_lin)) =
                (expr_to_linear(lhs.as_ref(), ctx), expr_to_linear(rhs.as_ref(), ctx))
            else {
                return Ok(None);
            };
            (
                lhs_lin,
                rhs_lin,
                Box::new(|offset: i64| Ok(vec![-offset, i64::MAX]))
                    as Box<dyn Fn(i64) -> SolverResult<Vec<i64>>>,
            )
        }
        Expression::Lt(_, lhs, rhs) => {
            let (Ok(lhs_lin), Ok(rhs_lin)) =
                (expr_to_linear(lhs.as_ref(), ctx), expr_to_linear(rhs.as_ref(), ctx))
            else {
                return Ok(None);
            };
            (
                lhs_lin,
                rhs_lin,
                Box::new(|offset: i64| Ok(vec![i64::MIN, -offset - 1]))
                    as Box<dyn Fn(i64) -> SolverResult<Vec<i64>>>,
            )
        }
        Expression::Gt(_, lhs, rhs) => {
            let (Ok(lhs_lin), Ok(rhs_lin)) =
                (expr_to_linear(lhs.as_ref(), ctx), expr_to_linear(rhs.as_ref(), ctx))
            else {
                return Ok(None);
            };
            (
                lhs_lin,
                rhs_lin,
                Box::new(|offset: i64| Ok(vec![-offset + 1, i64::MAX]))
                    as Box<dyn Fn(i64) -> SolverResult<Vec<i64>>>,
            )
        }
        Expression::FlatSumLeq(_, vars, total) => {
            let mut lhs_linear = LinearExpr {
                vars: vec![],
                coeffs: vec![],
                offset: 0,
            };
            for var in vars {
                let var_linear =
                    expr_to_linear(&Expression::Atomic(Metadata::default(), var.clone()), ctx)?;
                lhs_linear.vars.extend(var_linear.vars);
                lhs_linear.coeffs.extend(var_linear.coeffs);
                lhs_linear.offset += var_linear.offset;
            }
            let rhs_linear =
                expr_to_linear(&Expression::Atomic(Metadata::default(), total.clone()), ctx)?;
            (
                lhs_linear,
                rhs_linear,
                Box::new(|offset: i64| Ok(vec![i64::MIN, -offset]))
                    as Box<dyn Fn(i64) -> SolverResult<Vec<i64>>>,
            )
        }
        Expression::FlatWeightedSumLeq(_, coeffs, vars, total) => {
            let mut lhs_linear = LinearExpr {
                vars: vec![],
                coeffs: vec![],
                offset: 0,
            };
            for (coeff_lit, var) in coeffs.iter().zip(vars) {
                let Literal::Int(coeff_val) = coeff_lit else {
                    return Err(SolverError::ModelInvalid(
                        "Weighted sum coefficient is not an integer".into(),
                    ));
                };
                let var_linear =
                    expr_to_linear(&Expression::Atomic(Metadata::default(), var.clone()), ctx)?;
                lhs_linear.vars.extend(var_linear.vars.clone());
                lhs_linear
                    .coeffs
                    .extend(var_linear.coeffs.iter().map(|c| c * *coeff_val as i64));
                lhs_linear.offset += var_linear.offset * *coeff_val as i64;
            }
            let rhs_linear = expr_to_linear(
                &Expression::Atomic(Metadata::default(), total.as_ref().clone()),
                ctx,
            )?;
            (
                lhs_linear,
                rhs_linear,
                Box::new(|offset: i64| Ok(vec![i64::MIN, -offset]))
                    as Box<dyn Fn(i64) -> SolverResult<Vec<i64>>>,
            )
        }
        Expression::FlatSumGeq(_, vars, total) => {
            let mut lhs_linear = LinearExpr {
                vars: vec![],
                coeffs: vec![],
                offset: 0,
            };
            for var in vars {
                let var_linear =
                    expr_to_linear(&Expression::Atomic(Metadata::default(), var.clone()), ctx)?;
                lhs_linear.vars.extend(var_linear.vars);
                lhs_linear.coeffs.extend(var_linear.coeffs);
                lhs_linear.offset += var_linear.offset;
            }
            let rhs_linear =
                expr_to_linear(&Expression::Atomic(Metadata::default(), total.clone()), ctx)?;
            (
                lhs_linear,
                rhs_linear,
                Box::new(|offset: i64| Ok(vec![-offset, i64::MAX]))
                    as Box<dyn Fn(i64) -> SolverResult<Vec<i64>>>,
            )
        }
        Expression::FlatWeightedSumGeq(_, coeffs, vars, total) => {
            let mut lhs_linear = LinearExpr {
                vars: vec![],
                coeffs: vec![],
                offset: 0,
            };
            for (coeff_lit, var) in coeffs.iter().zip(vars) {
                let Literal::Int(coeff_val) = coeff_lit else {
                    return Err(SolverError::ModelInvalid(
                        "Weighted sum coefficient is not an integer".into(),
                    ));
                };
                let var_linear =
                    expr_to_linear(&Expression::Atomic(Metadata::default(), var.clone()), ctx)?;
                lhs_linear.vars.extend(var_linear.vars.clone());
                lhs_linear
                    .coeffs
                    .extend(var_linear.coeffs.iter().map(|c| c * *coeff_val as i64));
                lhs_linear.offset += var_linear.offset * *coeff_val as i64;
            }
            let rhs_linear = expr_to_linear(
                &Expression::Atomic(Metadata::default(), total.as_ref().clone()),
                ctx,
            )?;
            (
                lhs_linear,
                rhs_linear,
                Box::new(|offset: i64| Ok(vec![-offset, i64::MAX]))
                    as Box<dyn Fn(i64) -> SolverResult<Vec<i64>>>,
            )
        }
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
                return Ok(LinearExpr {
                    vars: vec![],
                    coeffs: vec![],
                    offset: val as i64,
                });
            }
            if let Some(Literal::Bool(val)) = reference.resolve_constant() {
                return Ok(LinearExpr {
                    vars: vec![],
                    coeffs: vec![],
                    offset: i64::from(val),
                });
            }

            let name = reference.name();
            let var_index = resolve_var_index(&name, ctx)?;

            Ok(LinearExpr {
                vars: vec![var_index],
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
        Expression::Sum(_, inner) => match inner.as_ref() {
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
                Ok(simplify_linear_expr(LinearExpr {
                    vars,
                    coeffs,
                    offset,
                }))
            }
            _ => Err(SolverError::ModelFeatureNotSupported(format!(
                "Unsupported sum argument in linear constraint: {:?}",
                inner
            ))),
        },
        Expression::Product(_, inner) => match inner.as_ref() {
            Expression::AbstractLiteral(_, AbstractLiteral::Matrix(elems, _)) => {
                let mut scalar_mult: i64 = 1;
                let mut linear_opt: Option<LinearExpr> = None;

                for elem in elems {
                    if let Ok(lin) = expr_to_linear(elem, ctx) {
                        if lin.vars.is_empty() {
                            scalar_mult *= lin.offset;
                        } else if linear_opt.is_none() {
                            linear_opt = Some(lin);
                        } else {
                            return Err(SolverError::ModelFeatureNotSupported(format!(
                                "Non-linear Product in linear constraint: {:?}",
                                inner
                            )));
                        }
                    } else {
                        return Err(SolverError::ModelFeatureNotSupported(format!(
                            "Unsupported Product element in linear constraint: {:?}",
                            elem
                        )));
                    }
                }

                if let Some(lin) = linear_opt {
                    Ok(simplify_linear_expr(LinearExpr {
                        vars: lin.vars,
                        coeffs: lin.coeffs.into_iter().map(|c| c * scalar_mult).collect(),
                        offset: lin.offset * scalar_mult,
                    }))
                } else {
                    Ok(LinearExpr {
                        vars: vec![],
                        coeffs: vec![],
                        offset: scalar_mult,
                    })
                }
            }
            _ => Err(SolverError::ModelFeatureNotSupported(format!(
                "Unsupported Product argument in linear constraint: {:?}",
                inner
            ))),
        },
        Expression::ToInt(_, inner) => expr_to_linear(inner, ctx),
        _ => {
            if let Some(lit) = eval_element_id_constant(expr) {
                if let Literal::Int(val) = lit {
                    return Ok(LinearExpr {
                        vars: vec![],
                        coeffs: vec![],
                        offset: val as i64,
                    });
                }
                if let Literal::Bool(val) = lit {
                    return Ok(LinearExpr {
                        vars: vec![],
                        coeffs: vec![],
                        offset: if val { 1 } else { 0 },
                    });
                }
            }
            Err(SolverError::ModelFeatureNotSupported(format!(
                "Unsupported expression in linear constraint: {expr:?}"
            )))
        }
    }
}

fn simplify_linear_expr(lin: LinearExpr) -> LinearExpr {
    use std::collections::BTreeMap;
    let mut map: BTreeMap<i32, i64> = BTreeMap::new();
    for (v, c) in lin.vars.into_iter().zip(lin.coeffs.into_iter()) {
        *map.entry(v).or_insert(0) += c;
    }
    let mut vars = Vec::new();
    let mut coeffs = Vec::new();
    for (v, c) in map {
        if c != 0 {
            vars.push(v);
            coeffs.push(c);
        }
    }
    LinearExpr {
        vars,
        coeffs,
        offset: lin.offset,
    }
}

fn eval_element_id_constant(expr: &Expression) -> Option<Literal> {
    match expr {
        Expression::ElementId(_, matrix, idx_expr) => {
            let idx_lit = eval_element_id_constant(idx_expr)?;
            let Literal::Int(idx_val) = idx_lit else { return None; };
            if let Expression::Atomic(_, Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Matrix(elems, _)))) = matrix.as_ref() {
                let p = idx_val as usize;
                if p >= 1 && p <= elems.len() {
                    return Some(elems[p - 1].clone());
                }
            }
        }
        Expression::SafeIndex(_, matrix, indices) if indices.len() == 1 => {
            let idx_lit = eval_element_id_constant(&indices[0])?;
            let Literal::Int(idx_val) = idx_lit else { return None; };
            if let Expression::Atomic(_, Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Matrix(elems, _)))) = matrix.as_ref() {
                let p = idx_val as usize;
                if p >= 1 && p <= elems.len() {
                    return Some(elems[p - 1].clone());
                }
            }
        }
        _ => {}
    }
    eval_constant(expr)
}

/// Helper to build a Protobuf linear constraint that enforces exactly one specific value.
fn exact_linear_constraint(linear_expr: LinearExpr, value: i64) -> ConstraintProto {
    ConstraintProto {
        name: String::new(),
        enforcement_literal: vec![],
        constraint: Some(constraint_proto::Constraint::Linear(
            LinearConstraintProto {
                vars: linear_expr.vars,
                coeffs: linear_expr.coeffs,
                domain: vec![value - linear_expr.offset, value - linear_expr.offset],
            },
        )),
    }
}

fn get_or_create_var_for_linear(linear: LinearExpr, cp_model: &mut CpModelProto) -> i32 {
    if linear.vars.len() == 1 && linear.coeffs == vec![1] && linear.offset == 0 {
        linear.vars[0]
    } else if linear.vars.is_empty() {
        let var_index = cp_model.variables.len() as i32;
        let mut var_proto = IntegerVariableProto::default();
        var_proto.domain = vec![linear.offset, linear.offset];
        cp_model.variables.push(var_proto);
        var_index
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
            constraint: Some(constraint_proto::Constraint::Linear(
                LinearConstraintProto {
                    vars,
                    coeffs,
                    domain: vec![linear.offset, linear.offset],
                },
            )),
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
        let candidates = [q_min * b_min, q_min * b_max, q_max * b_min, q_max * b_max];
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
                    constraint: Some(constraint_proto::Constraint::Linear(
                        LinearConstraintProto {
                            vars: target_expr.vars.clone(),
                            coeffs: target_expr.coeffs.clone(),
                            domain: vec![0 - target_expr.offset, val - 1 - target_expr.offset],
                        },
                    )),
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
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars,
                        coeffs,
                        domain: vec![-offset, -offset],
                    },
                )),
            });
            return Ok(exact_linear_constraint(
                LinearExpr {
                    vars: vec![],
                    coeffs: vec![],
                    offset: 0,
                },
                0,
            ));
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
                    constraint: Some(constraint_proto::Constraint::Linear(
                        LinearConstraintProto {
                            vars: target_expr.vars.clone(),
                            coeffs: target_expr.coeffs.clone(),
                            domain: vec![val + 1 - target_expr.offset, 0 - target_expr.offset],
                        },
                    )),
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
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars,
                        coeffs,
                        domain: vec![-offset, -offset],
                    },
                )),
            });
            return Ok(exact_linear_constraint(
                LinearExpr {
                    vars: vec![],
                    coeffs: vec![],
                    offset: 0,
                },
                0,
            ));
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
            constraint: Some(constraint_proto::Constraint::Linear(
                LinearConstraintProto {
                    vars: vec![b_var],
                    coeffs: vec![1],
                    domain: vec![1, i64::MAX],
                },
            )),
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
            constraint: Some(constraint_proto::Constraint::Linear(
                LinearConstraintProto {
                    vars: vec![b_pos, b_var],
                    coeffs: vec![1, -1],
                    domain: vec![0, 0],
                },
            )),
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
            constraint: Some(constraint_proto::Constraint::Linear(
                LinearConstraintProto {
                    vars,
                    coeffs,
                    domain: vec![-target_expr.offset, -target_expr.offset],
                },
            )),
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
                    LinearExpressionProto {
                        vars: vec![q_pos],
                        coeffs: vec![1],
                        offset: 0,
                    },
                    LinearExpressionProto {
                        vars: vec![b_pos],
                        coeffs: vec![1],
                        offset: 0,
                    },
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
            constraint: Some(constraint_proto::Constraint::Linear(
                LinearConstraintProto {
                    vars,
                    coeffs,
                    domain: vec![-a_expr.offset, -a_expr.offset],
                },
            )),
        });

        // Enforce remainder bounds under is_pos:
        // 0 <= r_pos <= b_pos - 1 => r_pos >= 0 and r_pos - b_pos <= -1
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![is_pos],
            constraint: Some(constraint_proto::Constraint::Linear(
                LinearConstraintProto {
                    vars: vec![r_pos, b_pos],
                    coeffs: vec![1, -1],
                    domain: vec![i64::MIN, -1],
                },
            )),
        });

        // Enforce q_pos == 0 if !is_pos
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![-is_pos - 1],
            constraint: Some(constraint_proto::Constraint::Linear(
                LinearConstraintProto {
                    vars: vec![q_pos],
                    coeffs: vec![1],
                    domain: vec![0, 0],
                },
            )),
        });

        // Enforce r_pos == 0 if !is_pos
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![-is_pos - 1],
            constraint: Some(constraint_proto::Constraint::Linear(
                LinearConstraintProto {
                    vars: vec![r_pos],
                    coeffs: vec![1],
                    domain: vec![0, 0],
                },
            )),
        });

        // Enforce b_pos == 0 if !is_pos
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![-is_pos - 1],
            constraint: Some(constraint_proto::Constraint::Linear(
                LinearConstraintProto {
                    vars: vec![b_pos],
                    coeffs: vec![1],
                    domain: vec![0, 0],
                },
            )),
        });
    }

    if b_min < 0 {
        let is_neg = create_bool_var(cp_model, "div_is_neg");
        partition_vars.push(is_neg);

        // Enforce b <= -1 if is_neg
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![is_neg],
            constraint: Some(constraint_proto::Constraint::Linear(
                LinearConstraintProto {
                    vars: vec![b_var],
                    coeffs: vec![1],
                    domain: vec![i64::MIN, -1],
                },
            )),
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
            constraint: Some(constraint_proto::Constraint::Linear(
                LinearConstraintProto {
                    vars: vec![b_neg, b_var],
                    coeffs: vec![1, -1],
                    domain: vec![0, 0],
                },
            )),
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
            constraint: Some(constraint_proto::Constraint::Linear(
                LinearConstraintProto {
                    vars,
                    coeffs,
                    domain: vec![-target_expr.offset, -target_expr.offset],
                },
            )),
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
                    LinearExpressionProto {
                        vars: vec![q_neg],
                        coeffs: vec![1],
                        offset: 0,
                    },
                    LinearExpressionProto {
                        vars: vec![b_neg],
                        coeffs: vec![1],
                        offset: 0,
                    },
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
            constraint: Some(constraint_proto::Constraint::Linear(
                LinearConstraintProto {
                    vars,
                    coeffs,
                    domain: vec![-a_expr.offset, -a_expr.offset],
                },
            )),
        });

        // Enforce remainder bounds under is_neg:
        // b_neg + 1 <= r_neg <= 0 => r_neg <= 0 and r_neg - b_neg >= 1
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![is_neg],
            constraint: Some(constraint_proto::Constraint::Linear(
                LinearConstraintProto {
                    vars: vec![r_neg, b_neg],
                    coeffs: vec![1, -1],
                    domain: vec![1, i64::MAX],
                },
            )),
        });

        // Enforce q_neg == 0 if !is_neg
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![-is_neg - 1],
            constraint: Some(constraint_proto::Constraint::Linear(
                LinearConstraintProto {
                    vars: vec![q_neg],
                    coeffs: vec![1],
                    domain: vec![0, 0],
                },
            )),
        });

        // Enforce r_neg == 0 if !is_neg
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![-is_neg - 1],
            constraint: Some(constraint_proto::Constraint::Linear(
                LinearConstraintProto {
                    vars: vec![r_neg],
                    coeffs: vec![1],
                    domain: vec![0, 0],
                },
            )),
        });

        // Enforce b_neg == 0 if !is_neg
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![-is_neg - 1],
            constraint: Some(constraint_proto::Constraint::Linear(
                LinearConstraintProto {
                    vars: vec![b_neg],
                    coeffs: vec![1],
                    domain: vec![0, 0],
                },
            )),
        });
    }

    if has_zero {
        let is_zero = create_bool_var(cp_model, "div_is_zero");
        partition_vars.push(is_zero);

        // Enforce b == 0 if is_zero
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![is_zero],
            constraint: Some(constraint_proto::Constraint::Linear(
                LinearConstraintProto {
                    vars: vec![b_var],
                    coeffs: vec![1],
                    domain: vec![0, 0],
                },
            )),
        });

        // Enforce target == 0 if is_zero
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![is_zero],
            constraint: Some(constraint_proto::Constraint::Linear(
                LinearConstraintProto {
                    vars: target_expr.vars.clone(),
                    coeffs: target_expr.coeffs.clone(),
                    domain: vec![0 - target_expr.offset, 0 - target_expr.offset],
                },
            )),
        });
    }

    // Enforce exactly one partition variable is active
    let num_partition_vars = partition_vars.len();
    cp_model.constraints.push(ConstraintProto {
        name: String::new(),
        enforcement_literal: vec![],
        constraint: Some(constraint_proto::Constraint::Linear(
            LinearConstraintProto {
                vars: partition_vars,
                coeffs: vec![1; num_partition_vars],
                domain: vec![1, 1],
            },
        )),
    });

    // Return a dummy constraint that is always true
    Ok(exact_linear_constraint(
        LinearExpr {
            vars: vec![],
            coeffs: vec![],
            offset: 0,
        },
        0,
    ))
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

fn resolve_var_index(name: &Name, ctx: &TranslationContext) -> SolverResult<i32> {
    if let Some(&idx) = ctx.var_mapping.borrow().get(name) {
        return Ok(idx);
    }
    let elem_vars = get_matrix_element_vars(name, ctx);
    if elem_vars.len() == 1 {
        return Ok(elem_vars[0]);
    }
    let name_str = name.to_string();
    for (m_name, &idx) in ctx.var_mapping.borrow().iter() {
        if m_name.to_string() == name_str {
            return Ok(idx);
        }
    }
    Err(SolverError::ModelInvalid(format!("Unknown variable in constraint: {}", name)))
}

fn get_literal_strict(expr: &Expression, ctx: &TranslationContext) -> SolverResult<i32> {
    match expr {
        Expression::Atomic(_, Atom::Reference(reference)) => {
            let name = reference.name();
            resolve_var_index(&name, ctx)
        }
        Expression::Atomic(_, Atom::Literal(Literal::Bool(_))) => {
            Err(SolverError::ModelFeatureNotSupported(
                "Constant boolean literal inside logical constraint not supported yet".to_string(),
            ))
        }
        Expression::Not(_, inner) => {
            let inner_lit = get_literal_strict(inner.as_ref(), ctx)?;
            Ok(-inner_lit - 1)
        }
        _ => Err(SolverError::ModelFeatureNotSupported(format!(
            "Logical constraint children must be atomic or negation of atomic, got {:?}",
            expr
        ))),
    }
}

fn create_bool_var(model: &mut CpModelProto, name: &str) -> i32 {
    let idx = model.variables.len() as i32;
    model.variables.push(IntegerVariableProto {
        name: format!("{}_{}", name, idx),
        domain: vec![0, 1],
    });
    idx
}

fn get_or_create_literal(
    expr: &Expression,
    cp_model: &mut CpModelProto,
    ctx: &TranslationContext,
) -> SolverResult<i32> {
    if let Ok(lit) = get_literal_strict(expr, ctx) {
        return Ok(lit);
    }

    if let Expression::Atomic(_, Atom::Literal(Literal::Bool(value))) = expr {
        let b = create_bool_var(cp_model, "const_bool");
        let target_val = if *value { 1 } else { 0 };
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![],
            constraint: Some(constraint_proto::Constraint::Linear(
                LinearConstraintProto {
                    vars: vec![b],
                    coeffs: vec![1],
                    domain: vec![target_val, target_val],
                },
            )),
        });
        return Ok(b);
    }

    if let Expression::Not(_, inner) = expr {
        let inner_lit = get_or_create_literal(inner.as_ref(), cp_model, ctx)?;
        return Ok(-inner_lit - 1);
    }

    let b = create_bool_var(cp_model, "reified_expr");
    translate_reified_constraint(b, expr, cp_model, ctx)?;
    Ok(b)
}

fn equate_literal_and_var(ref_var: i32, lit: i32, cp_model: &mut CpModelProto) {
    if lit >= 0 {
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![],
            constraint: Some(constraint_proto::Constraint::Linear(
                LinearConstraintProto {
                    vars: vec![ref_var, lit],
                    coeffs: vec![1, -1],
                    domain: vec![0, 0],
                },
            )),
        });
    } else {
        let var = -lit - 1;
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![],
            constraint: Some(constraint_proto::Constraint::Linear(
                LinearConstraintProto {
                    vars: vec![ref_var, var],
                    coeffs: vec![1, 1],
                    domain: vec![1, 1],
                },
            )),
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
        return Err(SolverError::ModelInvalid(
            "Table first argument is not a matrix".into(),
        ));
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
                return Err(SolverError::ModelFeatureNotSupported(
                    "Complex expression in Table constraint".into(),
                ));
            }
        }
    }

    let Some(Literal::AbstractLiteral(AbstractLiteral::Matrix(rows, _))) = eval_constant(rows_expr)
    else {
        return Err(SolverError::ModelInvalid(
            "Table second argument is not a constant matrix".into(),
        ));
    };

    let mut values = Vec::new();
    let mut matched_any_row = false;
    for row in rows {
        let Literal::AbstractLiteral(AbstractLiteral::Matrix(row_elems, _)) = row else {
            return Err(SolverError::ModelInvalid(
                "Table row is not a constant matrix".into(),
            ));
        };
        if row_elems.len() != tuple_elems.len() {
            return Err(SolverError::ModelInvalid(
                "Table row width does not match tuple width".into(),
            ));
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
                    return Err(SolverError::ModelInvalid(
                        "Table row contains non-integer/bool literal".into(),
                    ));
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
            return Ok(exact_linear_constraint(
                LinearExpr {
                    vars: vec![],
                    coeffs: vec![],
                    offset: 0,
                },
                target_val,
            ));
        } else {
            let target_val = if negated { 0 } else { 1 };
            return Ok(exact_linear_constraint(
                LinearExpr {
                    vars: vec![],
                    coeffs: vec![],
                    offset: 0,
                },
                target_val,
            ));
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

fn translate_lex_comparison(
    op: &str,
    elems_l: Vec<LinearExpr>,
    elems_r: Vec<LinearExpr>,
    cp_model: &mut CpModelProto,
    ctx: &TranslationContext,
) -> SolverResult<ConstraintProto> {
    let n = elems_l.len();
    let m = elems_r.len();
    let min_len = n.min(m);

    if min_len == 0 {
        let is_true = match op {
            "<" => n < m,
            "<=" => n <= m,
            ">" => n > m,
            ">=" => n >= m,
            "=" => n == m,
            "!=" => n != m,
            _ => unreachable!(),
        };
        let target_val = if is_true { 0 } else { 1 };
        return Ok(exact_linear_constraint(
            LinearExpr {
                vars: vec![],
                coeffs: vec![],
                offset: 0,
            },
            target_val,
        ));
    }

    if op == "=" && n != m {
        return Ok(exact_linear_constraint(
            LinearExpr {
                vars: vec![],
                coeffs: vec![],
                offset: 0,
            },
            1,
        ));
    }
    if op == "!=" && n != m {
        return Ok(exact_linear_constraint(
            LinearExpr {
                vars: vec![],
                coeffs: vec![],
                offset: 0,
            },
            0,
        ));
    }

    const ORTOOLS_MIN: i64 = -9_223_372_036_854_775_800;
    const ORTOOLS_MAX: i64 = 9_223_372_036_854_775_800;

    let mut max_val: i64 = 1000;
    for expr in elems_l.iter().chain(elems_r.iter()) {
        for &c in &expr.coeffs {
            max_val = max_val.max(c.abs() * 1000);
        }
        max_val = max_val.max(expr.offset.abs() * 1000);
    }
    let base = max_val * 2 + 1;

    let mut can_use_base = true;
    let mut multiplier: i128 = 1;
    for _ in 0..min_len {
        if multiplier > (i64::MAX / 4) as i128 {
            can_use_base = false;
            break;
        }
        multiplier = multiplier.saturating_mul(base as i128);
    }

    if can_use_base {
        let mut combined_diff = LinearExpr {
            vars: vec![],
            coeffs: vec![],
            offset: 0,
        };

        let mut mult: i64 = 1;
        for i in (0..min_len).rev() {
            let diff = subtract_linear_exprs(elems_l[i].clone(), elems_r[i].clone());
            for (v, c) in diff.vars.into_iter().zip(diff.coeffs.into_iter()) {
                combined_diff.vars.push(v);
                combined_diff.coeffs.push(c.saturating_mul(mult));
            }
            combined_diff.offset += diff.offset.saturating_mul(mult);
            mult = mult.saturating_mul(base);
        }

        let domain = match op {
            "<" => {
                if n < m {
                    vec![ORTOOLS_MIN, -combined_diff.offset]
                } else {
                    vec![ORTOOLS_MIN, -1 - combined_diff.offset]
                }
            }
            "<=" => {
                if n <= m {
                    vec![ORTOOLS_MIN, -combined_diff.offset]
                } else {
                    vec![ORTOOLS_MIN, -1 - combined_diff.offset]
                }
            }
            ">" => {
                if n > m {
                    vec![-combined_diff.offset, ORTOOLS_MAX]
                } else {
                    vec![1 - combined_diff.offset, ORTOOLS_MAX]
                }
            }
            ">=" => {
                if n >= m {
                    vec![-combined_diff.offset, ORTOOLS_MAX]
                } else {
                    vec![1 - combined_diff.offset, ORTOOLS_MAX]
                }
            }
            "=" => vec![-combined_diff.offset, -combined_diff.offset],
            "!=" => vec![
                ORTOOLS_MIN,
                -1 - combined_diff.offset,
                1 - combined_diff.offset,
                ORTOOLS_MAX,
            ],
            _ => unreachable!(),
        };

        return Ok(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![],
            constraint: Some(constraint_proto::Constraint::Linear(
                LinearConstraintProto {
                    vars: combined_diff.vars,
                    coeffs: combined_diff.coeffs,
                    domain,
                },
            )),
        });
    }

    Err(SolverError::OpNotSupported("Matrix lex comparison too large for base encoding".into()))
}

fn bind_reified_constraint(
    ref_var: i32,
    proto: ConstraintProto,
    cp_model: &mut CpModelProto,
) -> SolverResult<ConstraintProto> {
    if let Some(constraint_proto::Constraint::BoolOr(bool_or)) = proto.constraint {
        let b = cp_model.variables.len() as i32;
        cp_model.variables.push(IntegerVariableProto {
            name: format!("__reified_lex_{b}"),
            domain: vec![0, 1],
        });

        for &lit in &bool_or.literals {
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::BoolOr(BoolArgumentProto {
                    literals: vec![-lit - 1, b],
                })),
            });
        }
        let mut not_b_or_lits = bool_or.literals.clone();
        not_b_or_lits.push(-b - 1);
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![],
            constraint: Some(constraint_proto::Constraint::BoolOr(BoolArgumentProto {
                literals: not_b_or_lits,
            })),
        });

        equate_literal_and_var(ref_var, b, cp_model);
        return Ok(exact_linear_constraint(
            LinearExpr {
                vars: vec![],
                coeffs: vec![],
                offset: 0,
            },
            0,
        ));
    }
    if let Some(constraint_proto::Constraint::Linear(lin)) = proto.constraint {
        let comp_domain = complement_domain_intervals(&lin.domain);
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![ref_var],
            constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                vars: lin.vars.clone(),
                coeffs: lin.coeffs.clone(),
                domain: lin.domain,
            })),
        });
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![-ref_var - 1],
            constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
                vars: lin.vars,
                coeffs: lin.coeffs,
                domain: comp_domain,
            })),
        });
        return Ok(exact_linear_constraint(
            LinearExpr {
                vars: vec![],
                coeffs: vec![],
                offset: 0,
            },
            0,
        ));
    }
    Err(SolverError::ModelFeatureNotSupported(
        "Unsupported reified constraint binding".into(),
    ))
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
            constraint: Some(constraint_proto::Constraint::Linear(
                LinearConstraintProto {
                    vars: vec![ref_var],
                    coeffs: vec![1],
                    domain: vec![target_val, target_val],
                },
            )),
        });
        return Ok(exact_linear_constraint(
            LinearExpr {
                vars: vec![],
                coeffs: vec![],
                offset: 0,
            },
            0,
        ));
    }

    // Check for logical operators: And, Or
    match inner_expr {
        Expression::And(_, inner) => {
            let Expression::AbstractLiteral(_, AbstractLiteral::Matrix(elems, _)) = inner.as_ref()
            else {
                return Err(SolverError::ModelFeatureNotSupported(format!(
                    "Unsupported And argument in reification: {:?}",
                    inner
                )));
            };
            let mut literals = Vec::new();
            for elem in elems {
                literals.push(get_or_create_literal(elem, cp_model, ctx)?);
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

            return Ok(exact_linear_constraint(
                LinearExpr {
                    vars: vec![],
                    coeffs: vec![],
                    offset: 0,
                },
                0,
            ));
        }
        Expression::Or(_, inner) => {
            let Expression::AbstractLiteral(_, AbstractLiteral::Matrix(elems, _)) = inner.as_ref()
            else {
                return Err(SolverError::ModelFeatureNotSupported(format!(
                    "Unsupported Or argument in reification: {:?}",
                    inner
                )));
            };
            let mut literals = Vec::new();
            for elem in elems {
                literals.push(get_or_create_literal(elem, cp_model, ctx)?);
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

            return Ok(exact_linear_constraint(
                LinearExpr {
                    vars: vec![],
                    coeffs: vec![],
                    offset: 0,
                },
                0,
            ));
        }


        Expression::Lt(_, lhs, rhs) | Expression::LexLt(_, lhs, rhs) => {
            if let (Some(elems_l), Some(elems_r)) = (
                expr_to_linear_list(lhs.as_ref(), ctx),
                expr_to_linear_list(rhs.as_ref(), ctx),
            ) {
                if elems_l.len() > 1 || elems_r.len() > 1 || elems_l.len() != elems_r.len() {
                    let lex_proto = translate_lex_comparison("<", elems_l, elems_r, cp_model, ctx)?;
                    return bind_reified_constraint(ref_var, lex_proto, cp_model);
                }
            }
        }
        Expression::Leq(_, lhs, rhs) | Expression::LexLeq(_, lhs, rhs) => {
            if let (Some(elems_l), Some(elems_r)) = (
                expr_to_linear_list(lhs.as_ref(), ctx),
                expr_to_linear_list(rhs.as_ref(), ctx),
            ) {
                if elems_l.len() > 1 || elems_r.len() > 1 || elems_l.len() != elems_r.len() {
                    let lex_proto = translate_lex_comparison("<=", elems_l, elems_r, cp_model, ctx)?;
                    return bind_reified_constraint(ref_var, lex_proto, cp_model);
                }
            }
        }
        Expression::Gt(_, lhs, rhs) | Expression::LexGt(_, lhs, rhs) => {
            if let (Some(elems_l), Some(elems_r)) = (
                expr_to_linear_list(lhs.as_ref(), ctx),
                expr_to_linear_list(rhs.as_ref(), ctx),
            ) {
                if elems_l.len() > 1 || elems_r.len() > 1 || elems_l.len() != elems_r.len() {
                    let lex_proto = translate_lex_comparison(">", elems_l, elems_r, cp_model, ctx)?;
                    return bind_reified_constraint(ref_var, lex_proto, cp_model);
                }
            }
        }
        Expression::Geq(_, lhs, rhs) | Expression::LexGeq(_, lhs, rhs) => {
            if let (Some(elems_l), Some(elems_r)) = (
                expr_to_linear_list(lhs.as_ref(), ctx),
                expr_to_linear_list(rhs.as_ref(), ctx),
            ) {
                if elems_l.len() > 1 || elems_r.len() > 1 || elems_l.len() != elems_r.len() {
                    let lex_proto = translate_lex_comparison(">=", elems_l, elems_r, cp_model, ctx)?;
                    return bind_reified_constraint(ref_var, lex_proto, cp_model);
                }
            }
        }
        Expression::Eq(meta, lhs, rhs) => {
            if let (Some(elems_l), Some(elems_r)) = (
                expr_to_linear_list(lhs.as_ref(), ctx),
                expr_to_linear_list(rhs.as_ref(), ctx),
            ) {
                if elems_l.len() != elems_r.len() {
                    let lex_proto = translate_lex_comparison("=", elems_l, elems_r, cp_model, ctx)?;
                    return bind_reified_constraint(ref_var, lex_proto, cp_model);
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
                        constraint: Some(constraint_proto::Constraint::Linear(
                            LinearConstraintProto {
                                vars: diff.vars.clone(),
                                coeffs: diff.coeffs.clone(),
                                domain: vec![0, 0],
                            },
                        )),
                    });
                    cp_model.constraints.push(ConstraintProto {
                        name: String::new(),
                        enforcement_literal: vec![-aux_idx - 1],
                        constraint: Some(constraint_proto::Constraint::Linear(
                            LinearConstraintProto {
                                vars: diff.vars,
                                coeffs: diff.coeffs,
                                domain: vec![i64::MIN, -1, 1, i64::MAX],
                            },
                        )),
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
                let mut or_literals = aux_vars.iter().map(|&xi| -xi - 1).collect::<Vec<_>>();
                or_literals.push(ref_var);
                cp_model.constraints.push(ConstraintProto {
                    name: String::new(),
                    enforcement_literal: vec![],
                    constraint: Some(constraint_proto::Constraint::BoolOr(BoolArgumentProto {
                        literals: or_literals,
                    })),
                });
                return Ok(exact_linear_constraint(
                    LinearExpr {
                        vars: vec![],
                        coeffs: vec![],
                        offset: 0,
                    },
                    0,
                ));
            }
        }
        Expression::AtMost(_, vars, counts, values) => {
            translate_cardinality_constraint(
                vars.as_ref(),
                counts.as_ref(),
                values.as_ref(),
                "AtMost",
                Some(ref_var),
                cp_model,
                ctx,
            )?;
            return Ok(exact_linear_constraint(
                LinearExpr {
                    vars: vec![],
                    coeffs: vec![],
                    offset: 0,
                },
                0,
            ));
        }
        Expression::AtLeast(_, vars, counts, values) => {
            translate_cardinality_constraint(
                vars.as_ref(),
                counts.as_ref(),
                values.as_ref(),
                "AtLeast",
                Some(ref_var),
                cp_model,
                ctx,
            )?;
            return Ok(exact_linear_constraint(
                LinearExpr {
                    vars: vec![],
                    coeffs: vec![],
                    offset: 0,
                },
                0,
            ));
        }
        Expression::Gcc(_, vars, values, counts) => {
            translate_cardinality_constraint(
                vars.as_ref(),
                counts.as_ref(),
                values.as_ref(),
                "Gcc",
                Some(ref_var),
                cp_model,
                ctx,
            )?;
            return Ok(exact_linear_constraint(
                LinearExpr {
                    vars: vec![],
                    coeffs: vec![],
                    offset: 0,
                },
                0,
            ));
        }
        Expression::GccWeak(_, vars, values, counts) => {
            translate_cardinality_constraint(
                vars.as_ref(),
                counts.as_ref(),
                values.as_ref(),
                "GccWeak",
                Some(ref_var),
                cp_model,
                ctx,
            )?;
            return Ok(exact_linear_constraint(
                LinearExpr {
                    vars: vec![],
                    coeffs: vec![],
                    offset: 0,
                },
                0,
            ));
        }

        Expression::Imply(_, lhs, rhs) => {
            let lhs_lit = get_or_create_literal(lhs.as_ref(), cp_model, ctx)?;
            let rhs_lit = get_or_create_literal(rhs.as_ref(), cp_model, ctx)?;

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
            return Ok(exact_linear_constraint(
                LinearExpr {
                    vars: vec![],
                    coeffs: vec![],
                    offset: 0,
                },
                0,
            ));
        }
        Expression::Neq(meta, lhs, rhs) => {
            if let (Some(elems_l), Some(elems_r)) = (
                expr_to_linear_list(lhs.as_ref(), ctx),
                expr_to_linear_list(rhs.as_ref(), ctx),
            ) {
                if elems_l.len() != elems_r.len() {
                    let lex_proto = translate_lex_comparison("!=", elems_l, elems_r, cp_model, ctx)?;
                    return bind_reified_constraint(ref_var, lex_proto, cp_model);
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
                        constraint: Some(constraint_proto::Constraint::Linear(
                            LinearConstraintProto {
                                vars: diff.vars.clone(),
                                coeffs: diff.coeffs.clone(),
                                domain: vec![i64::MIN, -1, 1, i64::MAX],
                            },
                        )),
                    });
                    cp_model.constraints.push(ConstraintProto {
                        name: String::new(),
                        enforcement_literal: vec![-aux_idx - 1],
                        constraint: Some(constraint_proto::Constraint::Linear(
                            LinearConstraintProto {
                                vars: diff.vars,
                                coeffs: diff.coeffs,
                                domain: vec![0, 0],
                            },
                        )),
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

                return Ok(exact_linear_constraint(
                    LinearExpr {
                        vars: vec![],
                        coeffs: vec![],
                        offset: 0,
                    },
                    0,
                ));
            }
        }
        Expression::MinionDivEqUndefZero(_, a, b, target)
        | Expression::MinionModuloEqUndefZero(_, a, b, target) => {
            let is_div = matches!(inner_expr, Expression::MinionDivEqUndefZero(..));
            let a_expr = expr_to_linear(
                &Expression::Atomic(Metadata::default(), a.as_ref().clone()),
                ctx,
            )?;
            let b_expr = expr_to_linear(
                &Expression::Atomic(Metadata::default(), b.as_ref().clone()),
                ctx,
            )?;
            let target_expr = expr_to_linear(
                &Expression::Atomic(Metadata::default(), target.as_ref().clone()),
                ctx,
            )?;

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

            let constraint =
                translate_div_mod_undef_zero(is_div, &a_expr, &b_expr, &target_aux_expr, cp_model)?;
            cp_model.constraints.push(constraint);

            let diff = subtract_linear_exprs(target_aux_expr, target_expr);
            let domain_true = vec![-diff.offset, -diff.offset];
            let domain_false = vec![i64::MIN, -diff.offset - 1, -diff.offset + 1, i64::MAX];

            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![ref_var],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: diff.vars.clone(),
                        coeffs: diff.coeffs.clone(),
                        domain: domain_true,
                    },
                )),
            });
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![-ref_var - 1],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: diff.vars,
                        coeffs: diff.coeffs,
                        domain: domain_false,
                    },
                )),
            });

            return Ok(exact_linear_constraint(
                LinearExpr {
                    vars: vec![],
                    coeffs: vec![],
                    offset: 0,
                },
                0,
            ));
        }

        Expression::In(_, lhs, rhs) => {
            let lhs_linear = expr_to_linear(lhs.as_ref(), ctx)?;
            let vals = extract_set_values(rhs.as_ref()).ok_or_else(|| {
                SolverError::ModelFeatureNotSupported(format!("Unsupported In set: {:?}", rhs))
            })?;
            let domain_intervals = values_to_flat_domain(&vals);
            let shifted_domain = domain_intervals
                .into_iter()
                .map(|v| v - lhs_linear.offset)
                .collect::<Vec<_>>();

            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![ref_var],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: lhs_linear.vars.clone(),
                        coeffs: lhs_linear.coeffs.clone(),
                        domain: shifted_domain.clone(),
                    },
                )),
            });

            let comp_intervals = complement_domain_intervals(&shifted_domain);
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![-ref_var - 1],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: lhs_linear.vars,
                        coeffs: lhs_linear.coeffs,
                        domain: comp_intervals,
                    },
                )),
            });

            return Ok(exact_linear_constraint(
                LinearExpr {
                    vars: vec![],
                    coeffs: vec![],
                    offset: 0,
                },
                0,
            ));
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
            constraint: Some(constraint_proto::Constraint::Linear(
                LinearConstraintProto {
                    vars: diff.vars.clone(),
                    coeffs: diff.coeffs.clone(),
                    domain: domain_true,
                },
            )),
        });

        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![-ref_var - 1],
            constraint: Some(constraint_proto::Constraint::Linear(
                LinearConstraintProto {
                    vars: diff.vars,
                    coeffs: diff.coeffs,
                    domain: domain_false,
                },
            )),
        });

        return Ok(exact_linear_constraint(
            LinearExpr {
                vars: vec![],
                coeffs: vec![],
                offset: 0,
            },
            0,
        ));
    }

    // Rest of the match inner_expr
    match inner_expr {
        Expression::MinionElementOne(_, array, index, target) => {
            use super::proto::ElementConstraintProto;
            let index_linear = expr_to_linear(
                &Expression::Atomic(Metadata::default(), index.as_ref().clone()),
                ctx,
            )?;
            let target_linear = expr_to_linear(
                &Expression::Atomic(Metadata::default(), target.as_ref().clone()),
                ctx,
            )?;
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
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: vec![index_1_var],
                        coeffs: vec![1],
                        domain: bounds_domain.clone(),
                    },
                )),
            });
            let bounds_comp = complement_domain_intervals(&bounds_domain);
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![-in_bounds_var - 1],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: vec![index_1_var],
                        coeffs: vec![1],
                        domain: bounds_comp,
                    },
                )),
            });

            let index_0_var = cp_model.variables.len() as i32;
            let mut index_0_proto = IntegerVariableProto::default();
            index_0_proto.domain = vec![0, (array.len() - 1) as i64];
            cp_model.variables.push(index_0_proto);

            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![in_bounds_var],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: vec![index_1_var, index_0_var],
                        coeffs: vec![1, -1],
                        domain: vec![1, 1],
                    },
                )),
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
                constraint: Some(constraint_proto::Constraint::Element(
                    ElementConstraintProto {
                        index: index_0_var,
                        target: element_val_var,
                        vars: element_vars,
                        linear_index: None,
                        linear_target: None,
                        exprs: vec![],
                    },
                )),
            });

            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![ref_var, in_bounds_var],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: vec![element_val_var, target_var],
                        coeffs: vec![1, -1],
                        domain: vec![0, 0],
                    },
                )),
            });

            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![-ref_var - 1, in_bounds_var],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: vec![element_val_var, target_var],
                        coeffs: vec![1, -1],
                        domain: vec![i64::MIN, -1, 1, i64::MAX],
                    },
                )),
            });

            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![-in_bounds_var - 1],
                constraint: Some(constraint_proto::Constraint::BoolOr(BoolArgumentProto {
                    literals: vec![-ref_var - 1],
                })),
            });

            Ok(exact_linear_constraint(
                LinearExpr {
                    vars: vec![],
                    coeffs: vec![],
                    offset: 0,
                },
                0,
            ))
        }
        Expression::InDomain(_, var_expr, domain) => {
            let var_linear = expr_to_linear(var_expr.as_ref(), ctx)?;
            let var_idx = get_or_create_var_for_linear(var_linear, cp_model);
            let resolved_domain = domain.resolve();
            let domain = resolved_domain.as_deref().map_err(|_| {
                SolverError::ModelInvalid("InDomain without resolvable domain".into())
            })?;
            let domain_intervals = extract_domain_intervals(domain)?;

            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![ref_var],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: vec![var_idx],
                        coeffs: vec![1],
                        domain: domain_intervals.clone(),
                    },
                )),
            });

            let comp_intervals = complement_domain_intervals(&domain_intervals);
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![-ref_var - 1],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: vec![var_idx],
                        coeffs: vec![1],
                        domain: comp_intervals,
                    },
                )),
            });

            Ok(exact_linear_constraint(
                LinearExpr {
                    vars: vec![],
                    coeffs: vec![],
                    offset: 0,
                },
                0,
            ))
        }
        Expression::FlatAllDiff(_, vars) => {
            let mut exprs = Vec::new();
            for var in vars {
                let var_expr =
                    expr_to_linear(&Expression::Atomic(Metadata::default(), var.clone()), ctx)?;
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
                        constraint: Some(constraint_proto::Constraint::Linear(
                            LinearConstraintProto {
                                vars: diff_vars.clone(),
                                coeffs: diff_coeffs.clone(),
                                domain: vec![-diff_offset, -diff_offset],
                            },
                        )),
                    });

                    // 2. ref_var => exprs[i] != exprs[j]
                    cp_model.constraints.push(ConstraintProto {
                        name: String::new(),
                        enforcement_literal: vec![ref_var],
                        constraint: Some(constraint_proto::Constraint::Linear(
                            LinearConstraintProto {
                                vars: diff_vars,
                                coeffs: diff_coeffs,
                                // x_i - x_j != 0 translates to domain [MIN, -1] U [1, MAX]
                                domain: vec![
                                    i64::MIN,
                                    -diff_offset - 1,
                                    -diff_offset + 1,
                                    i64::MAX,
                                ],
                            },
                        )),
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

            Ok(exact_linear_constraint(
                LinearExpr {
                    vars: vec![],
                    coeffs: vec![],
                    offset: 0,
                },
                0,
            ))
        }
        Expression::MinionPow(_, a, b, target) => {
            let a_expr = Expression::Atomic(Metadata::default(), a.as_ref().clone());
            let b_expr = Expression::Atomic(Metadata::default(), b.as_ref().clone());
            let target_expr = Expression::Atomic(Metadata::default(), target.as_ref().clone());

            let mut pos_constraint = translate_minion_pow_constraint(
                &a_expr,
                &b_expr,
                &target_expr,
                false,
                cp_model,
                ctx,
            )?;
            pos_constraint.enforcement_literal.push(ref_var);
            cp_model.constraints.push(pos_constraint);

            let mut neg_constraint = translate_minion_pow_constraint(
                &a_expr,
                &b_expr,
                &target_expr,
                true,
                cp_model,
                ctx,
            )?;
            neg_constraint.enforcement_literal.push(-ref_var - 1);
            cp_model.constraints.push(neg_constraint);

            Ok(exact_linear_constraint(
                LinearExpr {
                    vars: vec![],
                    coeffs: vec![],
                    offset: 0,
                },
                0,
            ))
        }
        Expression::FlatAbsEq(_, a, b) => {
            let a_expr = expr_to_linear(
                &Expression::Atomic(Metadata::default(), a.as_ref().clone()),
                ctx,
            )?;
            let b_expr = expr_to_linear(
                &Expression::Atomic(Metadata::default(), b.as_ref().clone()),
                ctx,
            )?;

            let abs_b_var = cp_model.variables.len() as i32;
            cp_model.variables.push(IntegerVariableProto {
                name: format!("__abs_aux_{abs_b_var}"),
                domain: vec![0, 1_000_000_000],
            });

            let abs_b_expr = LinearExpr {
                vars: vec![abs_b_var],
                coeffs: vec![1],
                offset: 0,
            };

            let diff_abs_b = subtract_linear_exprs(abs_b_expr.clone(), b_expr.clone());
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: diff_abs_b.vars.clone(),
                        coeffs: diff_abs_b.coeffs.clone(),
                        domain: vec![-diff_abs_b.offset, i64::MAX],
                    },
                )),
            });

            let mut minus_b = b_expr.clone();
            for c in &mut minus_b.coeffs {
                *c = -*c;
            }
            minus_b.offset = -minus_b.offset;
            let diff_abs_minus_b = subtract_linear_exprs(abs_b_expr.clone(), minus_b);
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: diff_abs_minus_b.vars.clone(),
                        coeffs: diff_abs_minus_b.coeffs.clone(),
                        domain: vec![-diff_abs_minus_b.offset, i64::MAX],
                    },
                )),
            });

            let is_pos_var = cp_model.variables.len() as i32;
            cp_model.variables.push(IntegerVariableProto {
                name: format!("__abs_pos_{is_pos_var}"),
                domain: vec![0, 1],
            });

            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![is_pos_var],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: b_expr.vars.clone(),
                        coeffs: b_expr.coeffs.clone(),
                        domain: vec![-b_expr.offset, i64::MAX],
                    },
                )),
            });

            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![-is_pos_var - 1],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: b_expr.vars.clone(),
                        coeffs: b_expr.coeffs.clone(),
                        domain: vec![i64::MIN, -b_expr.offset],
                    },
                )),
            });

            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![is_pos_var],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: diff_abs_b.vars.clone(),
                        coeffs: diff_abs_b.coeffs.clone(),
                        domain: vec![i64::MIN, -diff_abs_b.offset],
                    },
                )),
            });

            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![-is_pos_var - 1],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: diff_abs_minus_b.vars.clone(),
                        coeffs: diff_abs_minus_b.coeffs.clone(),
                        domain: vec![i64::MIN, -diff_abs_minus_b.offset],
                    },
                )),
            });

            let diff_a_abs = subtract_linear_exprs(a_expr, abs_b_expr);
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![ref_var],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: diff_a_abs.vars.clone(),
                        coeffs: diff_a_abs.coeffs.clone(),
                        domain: vec![-diff_a_abs.offset, -diff_a_abs.offset],
                    },
                )),
            });

            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![-ref_var - 1],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: diff_a_abs.vars,
                        coeffs: diff_a_abs.coeffs,
                        domain: vec![
                            i64::MIN,
                            -diff_a_abs.offset - 1,
                            -diff_a_abs.offset + 1,
                            i64::MAX,
                        ],
                    },
                )),
            });

            Ok(exact_linear_constraint(
                LinearExpr {
                    vars: vec![],
                    coeffs: vec![],
                    offset: 0,
                },
                0,
            ))
        }
        Expression::ElementId(_, matrix, value) => {
            translate_element_id_aux(ref_var, matrix.as_ref(), value.as_ref(), cp_model, ctx)
        }
        Expression::SafeIndex(_, matrix, indices) if indices.len() == 1 => {
            translate_element_id_aux(ref_var, matrix.as_ref(), &indices[0], cp_model, ctx)
        }
        Expression::ToInt(_, inner) => {
            translate_reified_constraint(ref_var, inner.as_ref(), cp_model, ctx)
        }
        Expression::Table(_, tuple, allowed_rows) => {
            translate_table_reified(ref_var, tuple.as_ref(), allowed_rows.as_ref(), cp_model, ctx)
        }
        _ => {
            if let Ok(linear_expr) = expr_to_linear(inner_expr, ctx) {
                let ref_linear = LinearExpr {
                    vars: vec![ref_var],
                    coeffs: vec![1],
                    offset: 0,
                };
                let diff = subtract_linear_exprs(ref_linear, linear_expr);
                cp_model.constraints.push(exact_linear_constraint(diff, 0));
                return Ok(exact_linear_constraint(
                    LinearExpr {
                        vars: vec![],
                        coeffs: vec![],
                        offset: 0,
                    },
                    0,
                ));
            }

            if let Ok(lit) = get_literal_strict(inner_expr, ctx) {
                equate_literal_and_var(ref_var, lit, cp_model);
                return Ok(exact_linear_constraint(
                    LinearExpr {
                        vars: vec![],
                        coeffs: vec![],
                        offset: 0,
                    },
                    0,
                ));
            }

            Err(SolverError::ModelFeatureNotSupported(format!(
                "Unsupported expression inside AuxDeclaration: {:?}",
                inner_expr
            )))
        }
    }
}

fn get_constant_int_vector(expr: &Expression) -> Option<Vec<i64>> {
    match expr {
        Expression::Atomic(_, Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Matrix(elems, _)))) => {
            let mut vec = Vec::new();
            for elem in elems {
                if let Literal::Int(val) = elem {
                    vec.push(*val as i64);
                } else {
                    return None;
                }
            }
            Some(vec)
        }
        Expression::AbstractLiteral(_, AbstractLiteral::Matrix(elems, _)) => {
            let mut vec = Vec::new();
            for elem in elems {
                if let Some(Literal::Int(val)) = eval_constant(elem) {
                    vec.push(val as i64);
                } else {
                    return None;
                }
            }
            Some(vec)
        }
        _ => None,
    }
}

fn compose_matrix_with_indices(matrix: &Expression, indices: &[i64]) -> Option<Expression> {
    match matrix {
        Expression::Atomic(_, Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Matrix(elems, domain)))) => {
            let mut min_idx: i64 = 1;
            if let Ok(intervals) = extract_domain_intervals(domain) {
                if let Some(&start) = intervals.first() {
                    min_idx = start;
                }
            }
            let mut composed_elems = Vec::new();
            for &idx in indices {
                let pos = (idx - min_idx) as usize;
                if pos < elems.len() {
                    composed_elems.push(Expression::Atomic(Metadata::default(), Atom::Literal(elems[pos].clone())));
                } else {
                    return None;
                }
            }
            let domain_ptr: crate::ast::DomainPtr = domain.clone().into();
            Some(Expression::AbstractLiteral(
                Metadata::default(),
                AbstractLiteral::Matrix(composed_elems, domain_ptr),
            ))
        }
        Expression::AbstractLiteral(_, AbstractLiteral::Matrix(elems, domain)) => {
            let mut composed_elems = Vec::new();
            for &idx in indices {
                let pos = (idx - 1) as usize;
                if pos < elems.len() {
                    composed_elems.push(elems[pos].clone());
                } else {
                    return None;
                }
            }
            Some(Expression::AbstractLiteral(
                Metadata::default(),
                AbstractLiteral::Matrix(composed_elems, domain.clone()),
            ))
        }
        _ => None,
    }
}

fn translate_element_id_aux(
    ref_var: i32,
    matrix: &Expression,
    value: &Expression,
    cp_model: &mut CpModelProto,
    ctx: &TranslationContext,
) -> SolverResult<ConstraintProto> {
    use super::proto::ElementConstraintProto;

    // Nested Element Collapsing: X[A[k]] where A is a constant index matrix
    if let Expression::ElementId(_, inner_matrix, inner_k) = value {
        if let Some(a_consts) = get_constant_int_vector(inner_matrix.as_ref()) {
            if let Some(composed_matrix) = compose_matrix_with_indices(matrix, &a_consts) {
                return translate_element_id_aux(ref_var, &composed_matrix, inner_k.as_ref(), cp_model, ctx);
            }
        }
    }
    if let Expression::SafeIndex(_, inner_matrix, inner_indices) = value {
        if inner_indices.len() == 1 {
            if let Some(a_consts) = get_constant_int_vector(inner_matrix.as_ref()) {
                if let Some(composed_matrix) = compose_matrix_with_indices(matrix, &a_consts) {
                    return translate_element_id_aux(ref_var, &composed_matrix, &inner_indices[0], cp_model, ctx);
                }
            }
        }
    }

    let index_linear = expr_to_linear(value, ctx)?;
    let index_1_var = get_or_create_var_for_linear(index_linear, cp_model);
    let target_var = ref_var;

    let element_linears = expr_to_linear_list(matrix, ctx)
        .ok_or_else(|| SolverError::ModelFeatureNotSupported("ElementId matrix argument".into()))?;

    let mut pos_and_vars = Vec::new();
    let mut max_pos: usize = 0;

    for (idx, lin) in element_linears.into_iter().enumerate() {
        let elem_var = get_or_create_var_for_linear(lin.clone(), cp_model);
        let mut pos = idx + 1;

        if lin.vars.len() == 1 {
            let var_idx = lin.vars[0];
            let var_map = ctx.var_mapping.borrow();
            for (name, &mapped_idx) in var_map.iter() {
                if mapped_idx == var_idx {
                    if let Name::Represented(box_tuple) = name {
                        let (_, repr_name, suffix) = box_tuple.as_ref();
                        if repr_name.as_str() == "matrix_to_atom" {
                            if let Ok(p) = suffix.as_str().parse::<usize>() {
                                pos = p;
                            }
                        }
                    }
                }
            }
        }

        max_pos = std::cmp::max(max_pos, pos);
        pos_and_vars.push((pos, elem_var));
    }

    let index_domain_max = cp_model.variables[index_1_var as usize]
        .domain
        .last()
        .copied()
        .unwrap_or(0);
    if index_domain_max > 0 {
        max_pos = std::cmp::max(max_pos, index_domain_max as usize);
    }

    let mut padded_vars = Vec::with_capacity(max_pos);
    for p in 1..=max_pos {
        let var = if let Some(&(_, elem_var)) = pos_and_vars.iter().find(|&&(pos, _)| pos == p) {
            elem_var
        } else {
            get_or_create_var_for_linear(
                LinearExpr {
                    vars: vec![],
                    coeffs: vec![],
                    offset: p as i64,
                },
                cp_model,
            )
        };
        padded_vars.push(var);
    }

    let index_0_var = cp_model.variables.len() as i32;
    let mut index_0_proto = IntegerVariableProto::default();
    index_0_proto.domain = vec![0, (padded_vars.len() - 1) as i64];
    cp_model.variables.push(index_0_proto);

    let mut min_idx: i64 = 1;
    if let Expression::Atomic(_, Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Matrix(_, domain)))) = matrix {
        if let Ok(intervals) = extract_domain_intervals(domain) {
            if let Some(&start) = intervals.first() {
                min_idx = start;
            }
        }
    }

    cp_model.constraints.push(ConstraintProto {
        name: String::new(),
        enforcement_literal: vec![],
        constraint: Some(constraint_proto::Constraint::Linear(
            LinearConstraintProto {
                vars: vec![index_1_var, index_0_var],
                coeffs: vec![1, -1],
                domain: vec![min_idx, min_idx],
            },
        )),
    });

    Ok(ConstraintProto {
        name: String::new(),
        enforcement_literal: vec![],
        constraint: Some(constraint_proto::Constraint::Element(
            ElementConstraintProto {
                index: index_0_var,
                target: target_var,
                vars: padded_vars,
                linear_index: None,
                linear_target: None,
                exprs: vec![],
            },
        )),
    })
}

fn translate_table_reified(
    ref_var: i32,
    tuple_expr: &Expression,
    rows_expr: &Expression,
    cp_model: &mut CpModelProto,
    ctx: &TranslationContext,
) -> SolverResult<ConstraintProto> {
    let tuple_linears = expr_to_linear_list(tuple_expr, ctx)
        .ok_or_else(|| SolverError::ModelFeatureNotSupported("Complex expression in Table constraint tuple".into()))?;

    let Some(Literal::AbstractLiteral(AbstractLiteral::Matrix(rows, _))) = eval_constant(rows_expr)
    else {
        return Err(SolverError::ModelInvalid(
            "Table second argument is not a constant matrix".into(),
        ));
    };

    let mut row_match_vars = Vec::new();

    for row in rows {
        let Literal::AbstractLiteral(AbstractLiteral::Matrix(row_elems, _)) = row else {
            return Err(SolverError::ModelInvalid(
                "Table row is not a constant matrix".into(),
            ));
        };

        if row_elems.len() != tuple_linears.len() {
            return Err(SolverError::ModelInvalid(
                "Table row width does not match tuple width".into(),
            ));
        }

        let mut row_vals = Vec::new();
        for elem in row_elems {
            match elem {
                Literal::Int(val) => row_vals.push(val as i64),
                Literal::Bool(val) => row_vals.push(if val { 1 } else { 0 }),
                _ => return Err(SolverError::ModelInvalid("Table row non-int/bool".into())),
            }
        }

        let row_var = cp_model.variables.len() as i32;
        cp_model.variables.push(IntegerVariableProto {
            name: format!("__table_row_match_{row_var}"),
            domain: vec![0, 1],
        });
        row_match_vars.push(row_var);

        // 1. row_var => each element == row_val
        for (lin, &val) in tuple_linears.iter().zip(row_vals.iter()) {
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![row_var],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: lin.vars.clone(),
                        coeffs: lin.coeffs.clone(),
                        domain: vec![val - lin.offset, val - lin.offset],
                    },
                )),
            });
        }

        // 2. tuple == row_vals => row_var
        let mut elem_match_vars = Vec::new();
        for (lin, &val) in tuple_linears.iter().zip(row_vals.iter()) {
            let b = cp_model.variables.len() as i32;
            cp_model.variables.push(IntegerVariableProto {
                name: format!("__elem_eq_{b}"),
                domain: vec![0, 1],
            });
            elem_match_vars.push(b);

            // b => lin == val
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![b],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: lin.vars.clone(),
                        coeffs: lin.coeffs.clone(),
                        domain: vec![val - lin.offset, val - lin.offset],
                    },
                )),
            });

            // !b => lin != val
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![-b - 1],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: lin.vars.clone(),
                        coeffs: lin.coeffs.clone(),
                        domain: vec![
                            i64::MIN,
                            val - lin.offset - 1,
                            val - lin.offset + 1,
                            i64::MAX,
                        ],
                    },
                )),
            });
        }

        // And(elem_match_vars) => row_var (i.e. not b1 \/ not b2 \/ ... \/ row_var)
        let mut not_elems_or_row = elem_match_vars.iter().map(|&b| -b - 1).collect::<Vec<_>>();
        not_elems_or_row.push(row_var);
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![],
            constraint: Some(constraint_proto::Constraint::BoolOr(BoolArgumentProto {
                literals: not_elems_or_row,
            })),
        });
    }

    // Now ref_var <=> Or(row_match_vars)
    for &row_var in &row_match_vars {
        cp_model.constraints.push(ConstraintProto {
            name: String::new(),
            enforcement_literal: vec![],
            constraint: Some(constraint_proto::Constraint::BoolOr(BoolArgumentProto {
                literals: vec![-row_var - 1, ref_var],
            })),
        });
    }

    let mut not_ref_or_rows = row_match_vars.clone();
    not_ref_or_rows.push(-ref_var - 1);
    cp_model.constraints.push(ConstraintProto {
        name: String::new(),
        enforcement_literal: vec![],
        constraint: Some(constraint_proto::Constraint::BoolOr(BoolArgumentProto {
            literals: not_ref_or_rows,
        })),
    });

    Ok(exact_linear_constraint(
        LinearExpr {
            vars: vec![],
            coeffs: vec![],
            offset: 0,
        },
        0,
    ))
}

fn translate_aux_declaration(
    reference: &crate::ast::Reference,
    inner_expr: &Expression,
    cp_model: &mut CpModelProto,
    ctx: &TranslationContext,
) -> SolverResult<ConstraintProto> {
    let mut ref_vars = get_matrix_element_vars(&reference.name(), ctx);

    let inner_linears_opt = expr_to_linear_list(inner_expr, ctx).or_else(|| {
        if let Expression::AbstractLiteral(_, AbstractLiteral::Matrix(elems, _)) = inner_expr {
            let mut list = Vec::new();
            for elem in elems {
                if let Ok(lin) = expr_to_linear(elem, ctx) {
                    list.push(lin);
                } else if let Ok(lit) = get_or_create_literal(elem, cp_model, ctx) {
                    list.push(LinearExpr {
                        vars: vec![lit],
                        coeffs: vec![1],
                        offset: 0,
                    });
                } else {
                    return None;
                }
            }
            Some(list)
        } else {
            None
        }
    });

    if ref_vars.is_empty() {
        if let Some(inner_linears) = inner_linears_opt {
            if inner_linears.len() > 1 {
                for (i, inner_lin) in inner_linears.into_iter().enumerate() {
                    let var_idx = get_or_create_var_for_linear(inner_lin, cp_model);
                    let suffix = format!("{}", i + 1);
                    let elem_name = match &*reference.name() {
                        Name::WithRepresentation(box_name, reprs) => {
                            Name::Represented(Box::new((
                                box_name.as_ref().clone(),
                                reprs.first().cloned().unwrap_or_else(|| "matrix_to_atom".into()),
                                suffix.into(),
                            )))
                        }
                        Name::Represented(box_tuple) => {
                            let (r_var, r_name, _) = box_tuple.as_ref();
                            Name::Represented(Box::new((r_var.clone(), r_name.clone(), suffix.into())))
                        }
                        name => Name::Represented(Box::new((name.clone(), "matrix_to_atom".into(), suffix.into()))),
                    };
                    ctx.var_mapping.borrow_mut().insert(elem_name, var_idx);
                }
                return Ok(exact_linear_constraint(
                    LinearExpr {
                        vars: vec![],
                        coeffs: vec![],
                        offset: 0,
                    },
                    0,
                ));
            } else if inner_linears.len() == 1 {
                let var_idx = get_or_create_var_for_linear(inner_linears.into_iter().next().unwrap(), cp_model);
                ctx.var_mapping.borrow_mut().insert(reference.name().clone(), var_idx);
                return Ok(exact_linear_constraint(
                    LinearExpr {
                        vars: vec![],
                        coeffs: vec![],
                        offset: 0,
                    },
                    0,
                ));
            }
        } else if let Expression::ToInt(_, inner) = inner_expr {
            let inner_linear = expr_to_linear(inner, ctx).or_else(|_| {
                let lit = get_or_create_literal(inner, cp_model, ctx)?;
                Ok(LinearExpr {
                    vars: vec![lit],
                    coeffs: vec![1],
                    offset: 0,
                })
            })?;
            let var_idx = get_or_create_var_for_linear(inner_linear, cp_model);
            ctx.var_mapping.borrow_mut().insert(reference.name().clone(), var_idx);
            return Ok(exact_linear_constraint(
                LinearExpr {
                    vars: vec![],
                    coeffs: vec![],
                    offset: 0,
                },
                0,
            ));
        } else if let Ok(inner_linear) = expr_to_linear(inner_expr, ctx) {
            let var_idx = get_or_create_var_for_linear(inner_linear, cp_model);
            ctx.var_mapping.borrow_mut().insert(reference.name().clone(), var_idx);
            return Ok(exact_linear_constraint(
                LinearExpr {
                    vars: vec![],
                    coeffs: vec![],
                    offset: 0,
                },
                0,
            ));
        } else if let Expression::Eq(_, lhs, rhs) = inner_expr {
            let elem_opt = match lhs.as_ref() {
                Expression::ElementId(_, m, v) => Some((m.as_ref(), v.as_ref())),
                Expression::SafeIndex(_, m, idxs) if idxs.len() == 1 => Some((m.as_ref(), &idxs[0])),
                _ => None,
            }.or_else(|| match rhs.as_ref() {
                Expression::ElementId(_, m, v) => Some((m.as_ref(), v.as_ref())),
                Expression::SafeIndex(_, m, idxs) if idxs.len() == 1 => Some((m.as_ref(), &idxs[0])),
                _ => None,
            });
            if let Some((matrix, value)) = elem_opt {
                let var_idx = get_or_create_literal(
                    &Expression::Atomic(Metadata::default(), Atom::Reference(reference.clone())),
                    cp_model,
                    ctx,
                )?;
                ctx.var_mapping.borrow_mut().insert(reference.name().clone(), var_idx);
                return translate_element_id_aux(var_idx, matrix, value, cp_model, ctx);
            }
        } else if let Expression::FlatProductEq(..) = inner_expr {
            return translate_constraint(inner_expr, cp_model, ctx);
        }

        let ref_var = get_or_create_literal(
            &Expression::Atomic(Metadata::default(), Atom::Reference(reference.clone())),
            cp_model,
            ctx,
        )?;
        return translate_reified_constraint(ref_var, inner_expr, cp_model, ctx);
    } else {
        let inner_linears = inner_linears_opt.or_else(|| {
            let mut list = Vec::new();
            for i in 0..ref_vars.len() {
                let index_expr = Expression::ElementId(
                    Metadata::default(),
                    inner_expr.clone().into(),
                    Expression::Atomic(Metadata::default(), Atom::Literal(Literal::Int((i + 1) as i32))).into(),
                );
                if let Ok(lin) = expr_to_linear(&index_expr, ctx) {
                    list.push(lin);
                } else if let Ok(lit) = get_or_create_literal(&index_expr, cp_model, ctx) {
                    list.push(LinearExpr {
                        vars: vec![lit],
                        coeffs: vec![1],
                        offset: 0,
                    });
                } else {
                    return None;
                }
            }
            Some(list)
        });

        if let Some(inner_linears) = inner_linears {
            let count = ref_vars.len().min(inner_linears.len());
            if count > 0 {
                for (ref_v, inner_lin) in ref_vars.into_iter().take(count).zip(inner_linears.into_iter().take(count)) {
                    let ref_linear = LinearExpr {
                        vars: vec![ref_v],
                        coeffs: vec![1],
                        offset: 0,
                    };
                    let diff = subtract_linear_exprs(ref_linear, inner_lin);
                    cp_model.constraints.push(exact_linear_constraint(diff, 0));
                }
                return Ok(exact_linear_constraint(
                    LinearExpr {
                        vars: vec![],
                        coeffs: vec![],
                        offset: 0,
                    },
                    0,
                ));
            }
        }
    }

    let ref_var = if ref_vars.len() == 1 {
        ref_vars[0]
    } else {
        get_literal_strict(
            &Expression::Atomic(Metadata::default(), Atom::Reference(reference.clone())),
            ctx,
        )?
    };

    if let Expression::ElementId(_, matrix, value) = inner_expr {
        return translate_element_id_aux(ref_var, matrix.as_ref(), value.as_ref(), cp_model, ctx);
    }
    if let Expression::SafeIndex(_, matrix, indices) = inner_expr {
        if indices.len() == 1 {
            return translate_element_id_aux(ref_var, matrix.as_ref(), &indices[0], cp_model, ctx);
        }
    }
    if let Expression::Eq(_, lhs, rhs) = inner_expr {
        let lhs_elem = match lhs.as_ref() {
            Expression::ElementId(_, m, v) => Some((m.as_ref(), v.as_ref())),
            Expression::SafeIndex(_, m, idxs) if idxs.len() == 1 => Some((m.as_ref(), &idxs[0])),
            _ => None,
        };
        if let Some((matrix, value)) = lhs_elem {
            return translate_element_id_aux(ref_var, matrix, value, cp_model, ctx);
        }
        let rhs_elem = match rhs.as_ref() {
            Expression::ElementId(_, m, v) => Some((m.as_ref(), v.as_ref())),
            Expression::SafeIndex(_, m, idxs) if idxs.len() == 1 => Some((m.as_ref(), &idxs[0])),
            _ => None,
        };
        if let Some((matrix, value)) = rhs_elem {
            return translate_element_id_aux(ref_var, matrix, value, cp_model, ctx);
        }
    }
    if let Expression::FlatProductEq(..) = inner_expr {
        return translate_constraint(inner_expr, cp_model, ctx);
    }
    if let Expression::Table(_, tuple, allowed_rows) = inner_expr {
        return translate_table_reified(ref_var, tuple.as_ref(), allowed_rows.as_ref(), cp_model, ctx);
    }
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
        return Err(SolverError::ModelFeatureNotSupported(
            "Complex base expression in Pow not supported".into(),
        ));
    };

    let b_vals = if b_expr.vars.is_empty() {
        vec![b_expr.offset]
    } else if b_expr.vars.len() == 1 && b_expr.coeffs == vec![1] && b_expr.offset == 0 {
        let var = b_expr.vars[0];
        get_domain_values(&cp_model.variables[var as usize].domain)
    } else {
        return Err(SolverError::ModelFeatureNotSupported(
            "Complex exponent expression in Pow not supported".into(),
        ));
    };

    let target_domain = if target_expr.vars.is_empty() {
        vec![target_expr.offset, target_expr.offset]
    } else if target_expr.vars.len() == 1
        && target_expr.coeffs == vec![1]
        && target_expr.offset == 0
    {
        let var = target_expr.vars[0];
        cp_model.variables[var as usize].domain.clone()
    } else {
        return Err(SolverError::ModelFeatureNotSupported(
            "Complex target expression in Pow not supported".into(),
        ));
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
        return Ok(exact_linear_constraint(
            LinearExpr {
                vars: vec![],
                coeffs: vec![],
                offset: 0,
            },
            target_val,
        ));
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
    let (target_lit, expr) = if let Ok(lit) = get_or_create_literal(lhs, cp_model, ctx) {
        (lit, rhs)
    } else if let Ok(lit) = get_or_create_literal(rhs, cp_model, ctx) {
        (lit, lhs)
    } else {
        return Err(SolverError::ModelFeatureNotSupported(
            "Complex Iff constraints not supported".into(),
        ));
    };

    match expr {
        Expression::And(_, inner) => {
            let Expression::AbstractLiteral(_, AbstractLiteral::Matrix(elems, _)) = inner.as_ref()
            else {
                return Err(SolverError::ModelFeatureNotSupported(
                    "Unsupported And in Iff".into(),
                ));
            };
            let mut literals = Vec::new();
            for elem in elems {
                literals.push(get_or_create_literal(elem, cp_model, ctx)?);
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
            let Expression::AbstractLiteral(_, AbstractLiteral::Matrix(elems, _)) = inner.as_ref()
            else {
                return Err(SolverError::ModelFeatureNotSupported(
                    "Unsupported Or in Iff".into(),
                ));
            };
            let mut literals = Vec::new();
            for elem in elems {
                literals.push(get_or_create_literal(elem, cp_model, ctx)?);
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
            let other_lit = get_or_create_literal(expr, cp_model, ctx)?;
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

fn translate_cardinality_constraint(
    vars: &Expression,
    counts: &Expression,
    values: &Expression,
    c_type: &str, // "AtMost", "AtLeast", "Gcc", "GccWeak"
    enforcement_lit: Option<i32>,
    cp_model: &mut CpModelProto,
    ctx: &TranslationContext,
) -> SolverResult<()> {
    let vars_lin = expr_to_linear_list(vars, ctx)
        .ok_or_else(|| SolverError::ModelFeatureNotSupported(format!("{} variables", c_type)))?;
    let counts_lin = expr_to_linear_list(counts, ctx)
        .ok_or_else(|| SolverError::ModelFeatureNotSupported(format!("{} counts", c_type)))?;
    let values_lin = expr_to_linear_list(values, ctx)
        .ok_or_else(|| SolverError::ModelFeatureNotSupported(format!("{} values", c_type)))?;

    if counts_lin.len() != values_lin.len() {
        return Err(SolverError::ModelInvalid(format!(
            "{}: counts and values length mismatch",
            c_type
        )));
    }

    for (count_lin, value_lin) in counts_lin.into_iter().zip(values_lin.into_iter()) {
        let mut indicator_vars = Vec::new();
        for var_lin in &vars_lin {
            let diff = subtract_linear_exprs(var_lin.clone(), value_lin.clone());

            // create indicator variable b <=> diff == 0
            let b = cp_model.variables.len() as i32;
            cp_model.variables.push(IntegerVariableProto {
                name: format!("card_ind_{}", b),
                domain: vec![0, 1],
            });
            let not_b = -b - 1;

            // b => diff == 0
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![b],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: diff.vars.clone(),
                        coeffs: diff.coeffs.clone(),
                        domain: vec![-diff.offset, -diff.offset],
                    },
                )),
            });

            // !b => diff != 0
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![not_b],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: diff.vars,
                        coeffs: diff.coeffs,
                        domain: vec![i64::MIN, -1 - diff.offset, 1 - diff.offset, i64::MAX],
                    },
                )),
            });

            indicator_vars.push(b);
        }

        let mut sum_lin = LinearExpr {
            vars: indicator_vars,
            coeffs: vec![1; vars_lin.len()],
            offset: 0,
        };
        sum_lin = subtract_linear_exprs(sum_lin, count_lin);

        let exact_linear_with_enforcement =
            |expr: LinearExpr, min: i64, max: i64, enf: i32| -> ConstraintProto {
                ConstraintProto {
                    name: String::new(),
                    enforcement_literal: vec![enf],
                    constraint: Some(constraint_proto::Constraint::Linear(
                        LinearConstraintProto {
                            vars: expr.vars,
                            coeffs: expr.coeffs,
                            domain: vec![
                                if min == i64::MIN {
                                    i64::MIN
                                } else {
                                    min - expr.offset
                                },
                                if max == i64::MAX {
                                    i64::MAX
                                } else {
                                    max - expr.offset
                                },
                            ],
                        },
                    )),
                }
            };

        if let Some(e_lit) = enforcement_lit {
            let not_e_lit = -e_lit - 1;
            match c_type {
                "AtMost" => {
                    cp_model.constraints.push(exact_linear_with_enforcement(
                        sum_lin.clone(),
                        i64::MIN,
                        0,
                        e_lit,
                    ));
                    cp_model.constraints.push(exact_linear_with_enforcement(
                        sum_lin,
                        1,
                        i64::MAX,
                        not_e_lit,
                    ));
                }
                "AtLeast" => {
                    cp_model.constraints.push(exact_linear_with_enforcement(
                        sum_lin.clone(),
                        0,
                        i64::MAX,
                        e_lit,
                    ));
                    cp_model.constraints.push(exact_linear_with_enforcement(
                        sum_lin,
                        i64::MIN,
                        -1,
                        not_e_lit,
                    ));
                }
                "Gcc" | "GccWeak" => {
                    cp_model.constraints.push(exact_linear_with_enforcement(
                        sum_lin.clone(),
                        0,
                        0,
                        e_lit,
                    ));
                    cp_model.constraints.push(exact_linear_with_enforcement(
                        sum_lin.clone(),
                        i64::MIN,
                        -1,
                        not_e_lit,
                    ));
                    cp_model.constraints.push(exact_linear_with_enforcement(
                        sum_lin,
                        1,
                        i64::MAX,
                        not_e_lit,
                    ));
                }
                _ => unreachable!(),
            }
        } else {
            let mut cons = exact_linear_constraint(sum_lin.clone(), 0); // Placeholder
            match c_type {
                "AtMost" => {
                    if let Some(constraint_proto::Constraint::Linear(lin)) = &mut cons.constraint {
                        lin.domain = vec![i64::MIN, -sum_lin.offset];
                    }
                }
                "AtLeast" => {
                    if let Some(constraint_proto::Constraint::Linear(lin)) = &mut cons.constraint {
                        lin.domain = vec![-sum_lin.offset, i64::MAX];
                    }
                }
                "Gcc" | "GccWeak" => {
                    if let Some(constraint_proto::Constraint::Linear(lin)) = &mut cons.constraint {
                        lin.domain = vec![-sum_lin.offset, -sum_lin.offset];
                    }
                }
                _ => unreachable!(),
            }
            cp_model.constraints.push(cons);
        }
    }
    Ok(())
}

/// Main dispatcher: takes a Conjure constraint, extracts LHS and RHS, linearizes them, and builds a Protobuf constraint.
fn translate_constraint(
    expr: &Expression,
    cp_model: &mut CpModelProto,
    ctx: &TranslationContext,
) -> SolverResult<ConstraintProto> {
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
            let enforcement_lit = get_or_create_literal(lhs.as_ref(), cp_model, ctx)?;
            let mut constraint = translate_constraint(rhs.as_ref(), cp_model, ctx)?;
            constraint.enforcement_literal.push(enforcement_lit);
            return Ok(constraint);
        }
        Expression::AtMost(_, vars, counts, values) => {
            translate_cardinality_constraint(
                vars.as_ref(),
                counts.as_ref(),
                values.as_ref(),
                "AtMost",
                None,
                cp_model,
                ctx,
            )?;
            return Ok(ConstraintProto {
                constraint: Some(constraint_proto::Constraint::BoolAnd(BoolArgumentProto {
                    literals: vec![],
                })),
                name: String::new(),
                enforcement_literal: vec![],
            });
        }
        Expression::AtLeast(_, vars, counts, values) => {
            translate_cardinality_constraint(
                vars.as_ref(),
                counts.as_ref(),
                values.as_ref(),
                "AtLeast",
                None,
                cp_model,
                ctx,
            )?;
            return Ok(ConstraintProto {
                constraint: Some(constraint_proto::Constraint::BoolAnd(BoolArgumentProto {
                    literals: vec![],
                })),
                name: String::new(),
                enforcement_literal: vec![],
            });
        }
        Expression::Gcc(_, vars, values, counts) => {
            translate_cardinality_constraint(
                vars.as_ref(),
                counts.as_ref(),
                values.as_ref(),
                "Gcc",
                None,
                cp_model,
                ctx,
            )?;
            return Ok(ConstraintProto {
                constraint: Some(constraint_proto::Constraint::BoolAnd(BoolArgumentProto {
                    literals: vec![],
                })),
                name: String::new(),
                enforcement_literal: vec![],
            });
        }
        Expression::GccWeak(_, vars, values, counts) => {
            translate_cardinality_constraint(
                vars.as_ref(),
                counts.as_ref(),
                values.as_ref(),
                "GccWeak",
                None,
                cp_model,
                ctx,
            )?;
            return Ok(ConstraintProto {
                constraint: Some(constraint_proto::Constraint::BoolAnd(BoolArgumentProto {
                    literals: vec![],
                })),
                name: String::new(),
                enforcement_literal: vec![],
            });
        }
        _ => {}
    }



    // 1. Matrix Equality/Inequality, Neq, and In constraints
    match expr {
        Expression::Lt(_, lhs, rhs) | Expression::LexLt(_, lhs, rhs) => {
            if let (Some(elems_l), Some(elems_r)) = (
                expr_to_linear_list(lhs.as_ref(), ctx),
                expr_to_linear_list(rhs.as_ref(), ctx),
            ) {
                if elems_l.len() > 1 || elems_r.len() > 1 || elems_l.len() != elems_r.len() {
                    return translate_lex_comparison("<", elems_l, elems_r, cp_model, ctx);
                }
            }
        }
        Expression::Leq(_, lhs, rhs) | Expression::LexLeq(_, lhs, rhs) => {
            if let (Some(elems_l), Some(elems_r)) = (
                expr_to_linear_list(lhs.as_ref(), ctx),
                expr_to_linear_list(rhs.as_ref(), ctx),
            ) {
                if elems_l.len() > 1 || elems_r.len() > 1 || elems_l.len() != elems_r.len() {
                    return translate_lex_comparison("<=", elems_l, elems_r, cp_model, ctx);
                }
            }
        }
        Expression::Gt(_, lhs, rhs) | Expression::LexGt(_, lhs, rhs) => {
            if let (Some(elems_l), Some(elems_r)) = (
                expr_to_linear_list(lhs.as_ref(), ctx),
                expr_to_linear_list(rhs.as_ref(), ctx),
            ) {
                if elems_l.len() > 1 || elems_r.len() > 1 || elems_l.len() != elems_r.len() {
                    return translate_lex_comparison(">", elems_l, elems_r, cp_model, ctx);
                }
            }
        }
        Expression::Geq(_, lhs, rhs) | Expression::LexGeq(_, lhs, rhs) => {
            if let (Some(elems_l), Some(elems_r)) = (
                expr_to_linear_list(lhs.as_ref(), ctx),
                expr_to_linear_list(rhs.as_ref(), ctx),
            ) {
                if elems_l.len() > 1 || elems_r.len() > 1 || elems_l.len() != elems_r.len() {
                    return translate_lex_comparison(">=", elems_l, elems_r, cp_model, ctx);
                }
            }
        }
        Expression::Eq(meta, lhs, rhs) => {
            if let (Some(elems_l), Some(elems_r)) = (
                expr_to_linear_list(lhs.as_ref(), ctx),
                expr_to_linear_list(rhs.as_ref(), ctx),
            ) {
                if elems_l.len() != elems_r.len() {
                    return translate_lex_comparison("=", elems_l, elems_r, cp_model, ctx);
                }
                for (el, er) in elems_l.into_iter().zip(elems_r) {
                    let diff = subtract_linear_exprs(el, er);
                    cp_model.constraints.push(exact_linear_constraint(diff, 0));
                }
                return Ok(exact_linear_constraint(
                    LinearExpr {
                        vars: vec![],
                        coeffs: vec![],
                        offset: 0,
                    },
                    0,
                ));
            }
        }
        Expression::Neq(meta, lhs, rhs) => {
            if let (Some(elems_l), Some(elems_r)) = (
                expr_to_linear_list(lhs.as_ref(), ctx),
                expr_to_linear_list(rhs.as_ref(), ctx),
            ) {
                if elems_l.len() != elems_r.len() {
                    return translate_lex_comparison("!=", elems_l, elems_r, cp_model, ctx);
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
                        constraint: Some(constraint_proto::Constraint::Linear(
                            LinearConstraintProto {
                                vars: diff.vars.clone(),
                                coeffs: diff.coeffs.clone(),
                                domain: domain_true,
                            },
                        )),
                    });

                    cp_model.constraints.push(ConstraintProto {
                        name: String::new(),
                        enforcement_literal: vec![-aux_idx - 1],
                        constraint: Some(constraint_proto::Constraint::Linear(
                            LinearConstraintProto {
                                vars: diff.vars,
                                coeffs: diff.coeffs,
                                domain: domain_false,
                            },
                        )),
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
                constraint: Some(constraint_proto::Constraint::AllDiff(
                    AllDifferentConstraintProto {
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
                    },
                )),
            });
        }
        Expression::In(_, lhs, rhs) => {
            let lhs_linear = expr_to_linear(lhs.as_ref(), ctx)?;
            let vals = extract_set_values(rhs.as_ref()).ok_or_else(|| {
                SolverError::ModelFeatureNotSupported(format!("Unsupported In set: {:?}", rhs))
            })?;
            let domain = values_to_flat_domain(&vals);
            let shifted_domain = domain
                .into_iter()
                .map(|v| v - lhs_linear.offset)
                .collect::<Vec<_>>();

            return Ok(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: lhs_linear.vars,
                        coeffs: lhs_linear.coeffs,
                        domain: shifted_domain,
                    },
                )),
            });
        }
        _ => {}
    }

    if let Expression::Eq(_, lhs, rhs) = expr {
        if let Ok(ref_var) = get_literal_strict(lhs.as_ref(), ctx) {
            if matches!(rhs.as_ref(), Expression::FlatAllDiff(_, _)) {
                return translate_reified_constraint(ref_var, rhs.as_ref(), cp_model, ctx);
            }
        } else if let Ok(ref_var) = get_literal_strict(rhs.as_ref(), ctx) {
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
            constraint: Some(constraint_proto::Constraint::Linear(
                LinearConstraintProto {
                    vars: linear_expr.vars,
                    coeffs: linear_expr.coeffs,
                    domain,
                },
            )),
        });
    }

    match expr {
        Expression::FlatAbsEq(_, a, b) => {
            // a = |b|
            let a_expr = expr_to_linear(
                &Expression::Atomic(Metadata::default(), a.as_ref().clone()),
                ctx,
            )?;
            let b_expr = expr_to_linear(
                &Expression::Atomic(Metadata::default(), b.as_ref().clone()),
                ctx,
            )?;

            // 1. a >= b  (a - b >= 0)
            let diff_ab = subtract_linear_exprs(a_expr.clone(), b_expr.clone());
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: diff_ab.vars.clone(),
                        coeffs: diff_ab.coeffs.clone(),
                        domain: vec![-diff_ab.offset, i64::MAX],
                    },
                )),
            });

            // 2. a >= -b  (a + b >= 0)
            let mut minus_b = b_expr.clone();
            for c in &mut minus_b.coeffs {
                *c = -*c;
            }
            minus_b.offset = -minus_b.offset;
            let diff_a_minus_b = subtract_linear_exprs(a_expr.clone(), minus_b);
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: diff_a_minus_b.vars.clone(),
                        coeffs: diff_a_minus_b.coeffs.clone(),
                        domain: vec![-diff_a_minus_b.offset, i64::MAX],
                    },
                )),
            });

            // 3. Create boolean indicator variable for b >= 0
            let is_pos_var = cp_model.variables.len() as i32;
            cp_model.variables.push(IntegerVariableProto {
                name: format!("__abs_pos_{is_pos_var}"),
                domain: vec![0, 1],
            });

            // is_pos => b >= 0
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![is_pos_var],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: b_expr.vars.clone(),
                        coeffs: b_expr.coeffs.clone(),
                        domain: vec![-b_expr.offset, i64::MAX],
                    },
                )),
            });

            // !is_pos => b <= 0
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![-is_pos_var - 1],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: b_expr.vars.clone(),
                        coeffs: b_expr.coeffs.clone(),
                        domain: vec![i64::MIN, -b_expr.offset],
                    },
                )),
            });

            // is_pos => a <= b  (a - b <= 0)
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![is_pos_var],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: diff_ab.vars.clone(),
                        coeffs: diff_ab.coeffs.clone(),
                        domain: vec![i64::MIN, -diff_ab.offset],
                    },
                )),
            });

            // !is_pos => a <= -b  (a + b <= 0)
            cp_model.constraints.push(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![-is_pos_var - 1],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: diff_a_minus_b.vars.clone(),
                        coeffs: diff_a_minus_b.coeffs.clone(),
                        domain: vec![i64::MIN, -diff_a_minus_b.offset],
                    },
                )),
            });

            return Ok(exact_linear_constraint(
                LinearExpr {
                    vars: vec![],
                    coeffs: vec![],
                    offset: 0,
                },
                0,
            ));
        }
        Expression::FlatAllDiff(_, vars) => {
            use super::proto::AllDifferentConstraintProto;
            let mut exprs = Vec::new();
            for var in vars {
                let var_expr =
                    expr_to_linear(&Expression::Atomic(Metadata::default(), var.clone()), ctx)?;
                exprs.push(super::proto::LinearExpressionProto {
                    vars: var_expr.vars,
                    coeffs: var_expr.coeffs,
                    offset: var_expr.offset,
                });
            }
            return Ok(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::AllDiff(
                    AllDifferentConstraintProto { exprs },
                )),
            });
        }
        Expression::Table(_, tuple, allowed_rows) => {
            return translate_table_constraint(false, tuple.as_ref(), allowed_rows.as_ref(), ctx);
        }
        Expression::NegativeTable(_, tuple, forbidden_rows) => {
            return translate_table_constraint(true, tuple.as_ref(), forbidden_rows.as_ref(), ctx);
        }
        Expression::MinionPow(_, a, b, target) => {
            let a_expr = Expression::Atomic(Metadata::default(), a.as_ref().clone());
            let b_expr = Expression::Atomic(Metadata::default(), b.as_ref().clone());
            let target_expr = Expression::Atomic(Metadata::default(), target.as_ref().clone());
            return translate_minion_pow_constraint(
                &a_expr,
                &b_expr,
                &target_expr,
                false,
                cp_model,
                ctx,
            );
        }
        Expression::MinionDivEqUndefZero(_, a, b, target) => {
            let a_expr = expr_to_linear(
                &Expression::Atomic(Metadata::default(), a.as_ref().clone()),
                ctx,
            )?;
            let b_expr = expr_to_linear(
                &Expression::Atomic(Metadata::default(), b.as_ref().clone()),
                ctx,
            )?;
            let target_expr = expr_to_linear(
                &Expression::Atomic(Metadata::default(), target.as_ref().clone()),
                ctx,
            )?;
            let constraint =
                translate_div_mod_undef_zero(true, &a_expr, &b_expr, &target_expr, cp_model)?;
            return Ok(constraint);
        }
        Expression::MinionModuloEqUndefZero(_, a, b, target) => {
            let a_expr = expr_to_linear(
                &Expression::Atomic(Metadata::default(), a.as_ref().clone()),
                ctx,
            )?;
            let b_expr = expr_to_linear(
                &Expression::Atomic(Metadata::default(), b.as_ref().clone()),
                ctx,
            )?;
            let target_expr = expr_to_linear(
                &Expression::Atomic(Metadata::default(), target.as_ref().clone()),
                ctx,
            )?;
            let constraint =
                translate_div_mod_undef_zero(false, &a_expr, &b_expr, &target_expr, cp_model)?;
            return Ok(constraint);
        }
        Expression::FlatProductEq(_, a, b, target) => {
            use super::proto::{LinearArgumentProto, LinearExpressionProto};
            let a_expr = expr_to_linear(
                &Expression::Atomic(Metadata::default(), a.as_ref().clone()),
                ctx,
            )?;
            let b_expr = expr_to_linear(
                &Expression::Atomic(Metadata::default(), b.as_ref().clone()),
                ctx,
            )?;
            let target_expr = expr_to_linear(
                &Expression::Atomic(Metadata::default(), target.as_ref().clone()),
                ctx,
            )?;
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
        }
        Expression::Or(_, inner) => {
            let Expression::AbstractLiteral(_, AbstractLiteral::Matrix(elems, _)) = inner.as_ref()
            else {
                return Err(SolverError::ModelFeatureNotSupported(format!(
                    "Unsupported Or argument in constraint: {:?}",
                    inner
                )));
            };
            let mut literals = Vec::new();
            for elem in elems {
                literals.push(get_or_create_literal(elem, cp_model, ctx)?);
            }
            return Ok(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::BoolOr(BoolArgumentProto {
                    literals,
                })),
            });
        }
        Expression::And(_, inner) => {
            let Expression::AbstractLiteral(_, AbstractLiteral::Matrix(elems, _)) = inner.as_ref()
            else {
                return Err(SolverError::ModelFeatureNotSupported(format!(
                    "Unsupported And argument in constraint: {:?}",
                    inner
                )));
            };
            let mut literals = Vec::new();
            for elem in elems {
                literals.push(get_or_create_literal(elem, cp_model, ctx)?);
            }
            return Ok(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::BoolAnd(BoolArgumentProto {
                    literals,
                })),
            });
        }
        Expression::MinionElementOne(_, array, index, target) => {
            use super::proto::ElementConstraintProto;
            let index_linear = expr_to_linear(
                &Expression::Atomic(Metadata::default(), index.as_ref().clone()),
                ctx,
            )?;
            let target_linear = expr_to_linear(
                &Expression::Atomic(Metadata::default(), target.as_ref().clone()),
                ctx,
            )?;

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
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: vec![index_1_var, index_0_var],
                        coeffs: vec![1, -1],
                        domain: vec![1, 1],
                    },
                )),
            });

            return Ok(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::Element(
                    ElementConstraintProto {
                        index: index_0_var,
                        target: target_var,
                        vars: element_vars,
                        linear_index: None,
                        linear_target: None,
                        exprs: vec![],
                    },
                )),
            });
        }
        Expression::InDomain(_, var_expr, domain) => {
            let var_linear = expr_to_linear(var_expr.as_ref(), ctx)?;
            let var_idx = get_or_create_var_for_linear(var_linear, cp_model);
            let resolved_domain = domain.resolve();
            let domain = resolved_domain.as_deref().map_err(|_| {
                SolverError::ModelInvalid("InDomain without resolvable domain".into())
            })?;
            let domain_intervals = extract_domain_intervals(domain)?;
            return Ok(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::Linear(
                    LinearConstraintProto {
                        vars: vec![var_idx],
                        coeffs: vec![1],
                        domain: domain_intervals,
                    },
                )),
            });
        }
        Expression::Iff(_, lhs, rhs) => {
            return translate_iff_constraint(lhs.as_ref(), rhs.as_ref(), cp_model, ctx);
        }
        Expression::Atomic(_, Atom::Literal(Literal::Bool(val))) => {
            if *val {
                return Ok(exact_linear_constraint(
                    LinearExpr {
                        vars: vec![],
                        coeffs: vec![],
                        offset: 0,
                    },
                    0,
                ));
            } else {
                return Ok(ConstraintProto {
                    name: String::new(),
                    enforcement_literal: vec![],
                    constraint: Some(constraint_proto::Constraint::Linear(
                        LinearConstraintProto {
                            vars: vec![],
                            coeffs: vec![],
                            domain: vec![1, 0], // Unsatisfiable domain
                        },
                    )),
                });
            }
        }
        _ => {
            return Err(SolverError::ModelFeatureNotSupported(format!(
                "Unsupported top-level constraint: {expr:?}"
            )));
        }
    }
}

#[derive(Clone)]
pub(super) struct SolutionVar {
    pub name: Name,
    pub var_index: usize,
    pub is_bool: bool,
}

/// Entry point for translation: iterates over all variables and constraints to build the final CpModelProto.
pub(super) fn model_to_cp_sat(model: Model) -> SolverResult<(CpModelProto, Vec<SolutionVar>)> {
    let mut cp_model = CpModelProto::default();
    let ctx = TranslationContext {
        var_mapping: RefCell::new(HashMap::new()),
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
            let domain = resolved_domain.as_deref().map_err(|_| {
                SolverError::ModelInvalid(format!("Variable {} without resolvable domain", name))
            })?;

            var_proto.domain = extract_domain_intervals(domain)?;

            let var_index = cp_model.variables.len() as i32;
            cp_model.variables.push(var_proto);

            ctx.var_mapping.borrow_mut().insert(name.clone(), var_index);
        }
    }

    let mut decision_var_indices = Vec::new();
    if let Some(ref search_order) = model.search_order {
        for name in search_order {
            if let Some(&var_index) = ctx.var_mapping.borrow().get(name) {
                decision_var_indices.push(var_index);
            } else {
                let elem_vars = get_matrix_element_vars(name, &ctx);
                decision_var_indices.extend(elem_vars);
            }
        }
    } else {
        for (name, _) in model.symbols().iter_local() {
            if !matches!(name, Name::Machine(_)) {
                if let Some(&var_index) = ctx.var_mapping.borrow().get(name) {
                    decision_var_indices.push(var_index);
                } else {
                    let elem_vars = get_matrix_element_vars(name, &ctx);
                    decision_var_indices.extend(elem_vars);
                }
            }
        }
    }

    for constraint in model.constraints() {
        let constraint_proto = translate_constraint(constraint, &mut cp_model, &ctx)?;
        cp_model.constraints.push(constraint_proto);
    }

    if let Some(objective) = &model.objective {
        let linear = expr_to_linear(&objective.expression, &ctx)?;
        let mut cp_objective = CpObjectiveProto::default();
        cp_objective.vars = linear.vars;

        match objective.direction {
            OptimiseDirection::Minimising => {
                cp_objective.coeffs = linear.coeffs;
                cp_objective.offset = linear.offset as f64;
                cp_objective.scaling_factor = 1.0;
            }
            OptimiseDirection::Maximising => {
                cp_objective.coeffs = linear.coeffs.into_iter().map(|coeff| -coeff).collect();
                cp_objective.offset = -(linear.offset as f64);
                cp_objective.scaling_factor = -1.0;
            }
        }

        cp_model.objective = Some(cp_objective);
    }

    if !decision_var_indices.is_empty() {
        cp_model.search_strategy.push(DecisionStrategyProto {
            variables: decision_var_indices,
            exprs: vec![],
            variable_selection_strategy: super::proto::decision_strategy_proto::VariableSelectionStrategy::ChooseFirst as i32,
            domain_reduction_strategy: super::proto::decision_strategy_proto::DomainReductionStrategy::SelectMinValue as i32,
        });
    }

    let mut solution_vars = Vec::new();
    let var_map = ctx.var_mapping.borrow();
    for (name, &var_index) in var_map.iter() {
        let is_bool = if (var_index as usize) < cp_model.variables.len() {
            let domain = &cp_model.variables[var_index as usize].domain;
            domain == &[0, 1]
        } else {
            false
        };
        solution_vars.push(SolutionVar {
            name: name.clone(),
            var_index: var_index as usize,
            is_bool,
        });
    }

    Ok((cp_model, solution_vars))
}

pub(super) fn response_to_solution(
    response: &CpSolverResponse,
    solution_vars: &[SolutionVar],
) -> SolverResult<HashMap<Name, Literal>> {
    let mut solution = HashMap::with_capacity(solution_vars.len());
    for var in solution_vars {
        if var.var_index < response.solution.len() {
            let value = response.solution[var.var_index];
            let literal = if var.is_bool {
                Literal::Bool(value != 0)
            } else {
                Literal::Int(value as i32)
            };
            solution.insert(var.name.clone(), literal);
        }
    }
    Ok(solution)
}

fn translate_minion_pow_constraint(
    a: &Expression,
    b: &Expression,
    target: &Expression,
    negated: bool,
    cp_model: &CpModelProto,
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
        return Err(SolverError::ModelFeatureNotSupported(
            "Complex base expression in Pow not supported".into(),
        ));
    };

    let b_vals = if b_expr.vars.is_empty() {
        vec![b_expr.offset]
    } else if b_expr.vars.len() == 1 && b_expr.coeffs == vec![1] && b_expr.offset == 0 {
        let var = b_expr.vars[0];
        get_domain_values(&cp_model.variables[var as usize].domain)
    } else {
        return Err(SolverError::ModelFeatureNotSupported(
            "Complex exponent expression in Pow not supported".into(),
        ));
    };

    let target_domain = if target_expr.vars.is_empty() {
        vec![target_expr.offset, target_expr.offset]
    } else if target_expr.vars.len() == 1
        && target_expr.coeffs == vec![1]
        && target_expr.offset == 0
    {
        let var = target_expr.vars[0];
        cp_model.variables[var as usize].domain.clone()
    } else {
        return Err(SolverError::ModelFeatureNotSupported(
            "Complex target expression in Pow not supported".into(),
        ));
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
        let mut target_val = if matched_any { 0 } else { 1 };
        if negated {
            target_val = 1 - target_val;
        }
        return Ok(exact_linear_constraint(
            LinearExpr {
                vars: vec![],
                coeffs: vec![],
                offset: 0,
            },
            target_val,
        ));
    }

    let table_constraint = super::proto::TableConstraintProto {
        vars,
        values,
        exprs: vec![],
        negated,
    };
    Ok(ConstraintProto {
        name: String::new(),
        enforcement_literal: vec![],
        constraint: Some(constraint_proto::Constraint::Table(table_constraint)),
    })
}
