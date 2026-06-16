use crate::solver::{SolverError, SolverResult};
use super::proto::{
    CpModelProto, IntegerVariableProto, ConstraintProto, LinearConstraintProto,
    constraint_proto, CpSolverResponse,
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

/// Main dispatcher: takes a Conjure constraint, extracts LHS and RHS, linearizes them, and builds a Protobuf constraint.
fn translate_constraint(expr: &Expression, ctx: &TranslationContext) -> SolverResult<ConstraintProto> {
    match expr {
        // Top-level boolean constraints must evaluate to true.
        Expression::Atomic(_, Atom::Literal(Literal::Bool(_)))
        | Expression::Atomic(_, Atom::Reference(_)) => {
            return Ok(exact_linear_constraint(expr_to_linear(expr, ctx)?, 1));
        }
        Expression::Not(_, inner) => {
            return Ok(exact_linear_constraint(expr_to_linear(inner, ctx)?, 0));
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
            use super::proto::{LinearArgumentProto, LinearExpressionProto};
            let a_expr = expr_to_linear(&Expression::Atomic(Metadata::default(), a.as_ref().clone()), ctx)?;
            let b_expr = expr_to_linear(&Expression::Atomic(Metadata::default(), b.as_ref().clone()), ctx)?;
            let target_expr = expr_to_linear(&Expression::Atomic(Metadata::default(), target.as_ref().clone()), ctx)?;
            return Ok(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::IntDiv(LinearArgumentProto {
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
        Expression::MinionModuloEqUndefZero(_, a, b, target) => {
            use super::proto::{LinearArgumentProto, LinearExpressionProto};
            let a_expr = expr_to_linear(&Expression::Atomic(Metadata::default(), a.as_ref().clone()), ctx)?;
            let b_expr = expr_to_linear(&Expression::Atomic(Metadata::default(), b.as_ref().clone()), ctx)?;
            let target_expr = expr_to_linear(&Expression::Atomic(Metadata::default(), target.as_ref().clone()), ctx)?;
            return Ok(ConstraintProto {
                name: String::new(),
                enforcement_literal: vec![],
                constraint: Some(constraint_proto::Constraint::IntMod(LinearArgumentProto {
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
        cp_model.constraints.push(translate_constraint(constraint, &ctx)?);
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

