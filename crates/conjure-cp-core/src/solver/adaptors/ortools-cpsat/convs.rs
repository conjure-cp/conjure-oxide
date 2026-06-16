use crate::solver::{SolverError, SolverResult};
use super::proto::{
    CpModelProto, IntegerVariableProto, ConstraintProto, LinearConstraintProto,
    constraint_proto, CpSolverResponse, BoolArgumentProto,
};
use std::collections::HashMap;
use crate::Model;
use crate::ast::{AbstractLiteral, Atom, Expression, GroundDomain, HasDomain, Literal, Metadata, Name, Range};

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
            let mut flat_domain = Vec::new();

            for range in ranges {
                match range {
                    Range::Single(v) => {
                        flat_domain.push(*v as i64);
                        flat_domain.push(*v as i64);
                    }
                    Range::Bounded(lb, ub) => {
                        flat_domain.push(*lb as i64);
                        flat_domain.push(*ub as i64);
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

            Ok(flat_domain)
        }
        GroundDomain::Bool => Ok(vec![0, 1]),
        _ => Err(SolverError::ModelFeatureNotSupported(
            "Domain not supported by OR-Tools CP-SAT".into(),
        )),
    }
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
    a: &Expression,
    b: &Expression,
    target: &Expression,
    cp_model: &mut CpModelProto,
    ctx: &TranslationContext,
) -> SolverResult<ConstraintProto> {
    use super::proto::{LinearArgumentProto, LinearExpressionProto};

    let a_expr = expr_to_linear(a, ctx)?;
    let b_expr = expr_to_linear(b, ctx)?;
    let target_expr = expr_to_linear(target, ctx)?;

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
            return Ok(exact_linear_constraint(target_expr, 0));
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

        // Define b_pos variable in [1, b_max]
        let b_pos = cp_model.variables.len() as i32;
        cp_model.variables.push(IntegerVariableProto {
            name: format!("b_pos_{}", b_pos),
            domain: vec![1, b_max],
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

        // Define b_neg variable in [b_min, -1]
        let b_neg = cp_model.variables.len() as i32;
        cp_model.variables.push(IntegerVariableProto {
            name: format!("b_neg_{}", b_neg),
            domain: vec![b_min, -1],
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
            let ref_var = get_literal(&Expression::Atomic(Metadata::default(), Atom::Reference(reference.clone())), ctx)?;
            match inner_expr.as_ref() {
                Expression::Eq(_, lhs, rhs)
                | Expression::Neq(_, lhs, rhs)
                | Expression::Leq(_, lhs, rhs)
                | Expression::Geq(_, lhs, rhs)
                | Expression::Lt(_, lhs, rhs)
                | Expression::Gt(_, lhs, rhs) => {
                    let lhs_linear = expr_to_linear(lhs.as_ref(), ctx)?;
                    let rhs_linear = expr_to_linear(rhs.as_ref(), ctx)?;
                    let diff = subtract_linear_exprs(lhs_linear, rhs_linear);
                    
                    let (domain_true, domain_false) = match inner_expr.as_ref() {
                        Expression::Eq(_, _, _) => (
                            vec![0 - diff.offset, 0 - diff.offset],
                            vec![i64::MIN, -1 - diff.offset, 1 - diff.offset, i64::MAX],
                        ),
                        Expression::Neq(_, _, _) => (
                            vec![i64::MIN, -1 - diff.offset, 1 - diff.offset, i64::MAX],
                            vec![0 - diff.offset, 0 - diff.offset],
                        ),
                        Expression::Leq(_, _, _) => (
                            vec![i64::MIN, 0 - diff.offset],
                            vec![1 - diff.offset, i64::MAX],
                        ),
                        Expression::Geq(_, _, _) => (
                            vec![0 - diff.offset, i64::MAX],
                            vec![i64::MIN, -1 - diff.offset],
                        ),
                        Expression::Lt(_, _, _) => (
                            vec![i64::MIN, -1 - diff.offset],
                            vec![0 - diff.offset, i64::MAX],
                        ),
                        Expression::Gt(_, _, _) => (
                            vec![1 - diff.offset, i64::MAX],
                            vec![i64::MIN, 0 - diff.offset],
                        ),
                        _ => unreachable!(),
                    };
                    
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
                _ => {
                    return Err(SolverError::ModelFeatureNotSupported(format!(
                        "Unsupported expression inside AuxDeclaration: {:?}",
                        inner_expr
                    )));
                }
            }
        }
        _ => {}
    }

    let (lhs_expr, rhs_expr, domain_func): (LinearExpr, LinearExpr, Box<dyn Fn(i64) -> SolverResult<Vec<i64>>>) = match expr {
        Expression::Eq(_, lhs, rhs) => (
            expr_to_linear(lhs.as_ref(), ctx)?,
            expr_to_linear(rhs.as_ref(), ctx)?,
            Box::new(|offset| Ok(vec![-offset, -offset])),
        ),
        Expression::Leq(_, lhs, rhs) => (
            expr_to_linear(lhs.as_ref(), ctx)?,
            expr_to_linear(rhs.as_ref(), ctx)?,
            Box::new(|offset| Ok(vec![i64::MIN, -offset])),
        ),
        Expression::Geq(_, lhs, rhs) => (
            expr_to_linear(lhs.as_ref(), ctx)?,
            expr_to_linear(rhs.as_ref(), ctx)?,
            Box::new(|offset| Ok(vec![-offset, i64::MAX])),
        ),
        Expression::Lt(_, lhs, rhs) => (
            expr_to_linear(lhs.as_ref(), ctx)?,
            expr_to_linear(rhs.as_ref(), ctx)?,
            Box::new(|offset| Ok(vec![i64::MIN, -offset - 1])),
        ),
        Expression::Gt(_, lhs, rhs) => (
            expr_to_linear(lhs.as_ref(), ctx)?,
            expr_to_linear(rhs.as_ref(), ctx)?,
            Box::new(|offset| Ok(vec![-offset + 1, i64::MAX])),
        ),
        Expression::Neq(_, lhs, rhs) => {
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
        },
        Expression::FlatSumLeq(_, vars, total) => {
            let mut lhs_linear = LinearExpr { vars: vec![], coeffs: vec![], offset: 0 };
            for var in vars {
                let var_linear = expr_to_linear(&Expression::Atomic(Metadata::default(), var.clone()), ctx)?;
                lhs_linear.vars.extend(var_linear.vars);
                lhs_linear.coeffs.extend(var_linear.coeffs);
                lhs_linear.offset += var_linear.offset;
            }
            let rhs_linear = expr_to_linear(&Expression::Atomic(Metadata::default(), total.clone()), ctx)?;
            (lhs_linear, rhs_linear, Box::new(|offset| Ok(vec![i64::MIN, -offset])))
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
            (lhs_linear, rhs_linear, Box::new(|offset| Ok(vec![i64::MIN, -offset])))
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
            (lhs_linear, rhs_linear, Box::new(|offset| Ok(vec![-offset, i64::MAX])))
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
            (lhs_linear, rhs_linear, Box::new(|offset| Ok(vec![-offset, i64::MAX])))
        },
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
        Expression::MinionDivEqUndefZero(_, a, b, target) => {
            let a_expr = Expression::Atomic(Metadata::default(), a.as_ref().clone());
            let b_expr = Expression::Atomic(Metadata::default(), b.as_ref().clone());
            let target_expr = Expression::Atomic(Metadata::default(), target.as_ref().clone());
            return translate_div_mod_undef_zero(true, &a_expr, &b_expr, &target_expr, cp_model, ctx);
        },
        Expression::MinionModuloEqUndefZero(_, a, b, target) => {
            let a_expr = Expression::Atomic(Metadata::default(), a.as_ref().clone());
            let b_expr = Expression::Atomic(Metadata::default(), b.as_ref().clone());
            let target_expr = Expression::Atomic(Metadata::default(), target.as_ref().clone());
            return translate_div_mod_undef_zero(false, &a_expr, &b_expr, &target_expr, cp_model, ctx);
        },
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
        _ => {
            return Err(SolverError::ModelFeatureNotSupported(format!(
                "Unsupported top-level constraint: {expr:?}"
            )))
        }
    };

    let linear_expr = subtract_linear_exprs(lhs_expr, rhs_expr);
    let domain = domain_func(linear_expr.offset)?;

    Ok(ConstraintProto {
        name: String::new(),
        enforcement_literal: vec![],
        constraint: Some(constraint_proto::Constraint::Linear(LinearConstraintProto {
            vars: linear_expr.vars,
            coeffs: linear_expr.coeffs,
            domain,
        })),
    })
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

