use crate::solver::{SolverError, SolverResult};
use super::proto::{
    CpModelProto, IntegerVariableProto, ConstraintProto, LinearConstraintProto,
    constraint_proto, CpSolverResponse,
};
use std::collections::HashMap;
use crate::Model;
use crate::ast::{Atom, Expression, GroundDomain, HasDomain, Literal, Name, Range};

struct TranslationContext {
    var_mapping: HashMap<Name, i32>,
}

struct LinearExpr {
    vars: Vec<i32>,
    coeffs: Vec<i64>,
    offset: i64,
}

/// Convert a Conjure domain in i64 CP-SAT type
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
        _ => Err(SolverError::ModelFeatureNotSupported(format!(
            "Unsupported expression in linear constraint: {expr:?}"
        ))),
    }
}

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

    let (lhs, rhs) = match expr {
        Expression::Eq(_, lhs, rhs)
        | Expression::Leq(_, lhs, rhs)
        | Expression::Geq(_, lhs, rhs)
        | Expression::Lt(_, lhs, rhs)
        | Expression::Gt(_, lhs, rhs) => (lhs.as_ref(), rhs.as_ref()),
        _ => {
            return Err(SolverError::ModelFeatureNotSupported(format!(
                "Unsupported top-level constraint: {expr:?}"
            )))
        }
    };

    let linear_expr = subtract_linear_exprs(expr_to_linear(lhs, ctx)?, expr_to_linear(rhs, ctx)?);
    let domain = comparison_domain(expr, linear_expr.offset)?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{DeclarationPtr, Domain, Metadata, Moo};
    use crate::context::Context;
    use std::sync::{Arc, RwLock};

    #[test]
    fn extract_domain_intervals_converts_bounded_integer_domains() {
        let domain = GroundDomain::Int(vec![Range::Single(3), Range::Bounded(5, 8)]);

        let result = extract_domain_intervals(&domain).expect("bounded int domain should convert");

        assert_eq!(result, vec![3, 3, 5, 8]);
    }

    #[test]
    fn extract_domain_intervals_converts_boolean_domains() {
        let result =
            extract_domain_intervals(&GroundDomain::Bool).expect("bool domain should convert");

        assert_eq!(result, vec![0, 1]);
    }

    #[test]
    fn extract_domain_intervals_rejects_unbounded_integer_domains() {
        let domain = GroundDomain::Int(vec![Range::UnboundedR(1)]);

        let result = extract_domain_intervals(&domain);

        assert!(matches!(
            result,
            Err(SolverError::ModelFeatureNotSupported(_))
        ));
    }

    #[test]
    fn model_to_cp_sat_translates_find_variables() {
        let mut model = Model::new(Arc::new(RwLock::new(Context::default())));
        model
            .add_symbol(DeclarationPtr::new_find(
                Name::User("x".into()),
                Domain::int(vec![Range::Bounded(1, 5)]),
            ))
            .expect("x should be inserted");
        model
            .add_symbol(DeclarationPtr::new_find(
                Name::User("flag".into()),
                Domain::bool(),
            ))
            .expect("flag should be inserted");

        let (cp_model, _vars) = model_to_cp_sat(model).expect("simple model should convert");

        assert_eq!(cp_model.variables.len(), 2);
        assert_eq!(cp_model.variables[0].name, "x");
        assert_eq!(cp_model.variables[0].domain, vec![1, 5]);
        assert_eq!(cp_model.variables[1].name, "flag");
        assert_eq!(cp_model.variables[1].domain, vec![0, 1]);
    }

    #[test]
    fn model_to_cp_sat_translates_single_integer_find_variable() {
        let mut model = Model::new(Arc::new(RwLock::new(Context::default())));
        model
            .add_symbol(DeclarationPtr::new_find(
                Name::User("x".into()),
                Domain::int(vec![Range::Bounded(1, 5)]),
            ))
            .expect("x should be inserted");

        let (cp_model, _vars) = model_to_cp_sat(model).expect("single integer find should convert");

        assert_eq!(cp_model.variables.len(), 1);
        assert_eq!(cp_model.variables[0].name, "x");
        assert_eq!(cp_model.variables[0].domain, vec![1, 5]);
    }

    #[test]
    fn model_to_cp_sat_translates_simple_equality_constraint() {
        let mut model = Model::new(Arc::new(RwLock::new(Context::default())));
        let x_decl = DeclarationPtr::new_find(
            Name::User("x".into()),
            Domain::int(vec![Range::Bounded(1, 5)]),
        );
        model.add_symbol(x_decl.clone()).expect("x should be inserted");
        model.add_constraint(Expression::Eq(
            Metadata::new(),
            Moo::new(Expression::from(Atom::new_ref(x_decl))),
            Moo::new(Expression::from(3)),
        ));

        let (cp_model, _vars) = model_to_cp_sat(model).expect("simple equality should convert");

        assert_eq!(cp_model.variables.len(), 1);
        assert_eq!(cp_model.constraints.len(), 1);

        let constraint = &cp_model.constraints[0];
        let Some(constraint_proto::Constraint::Linear(linear)) = &constraint.constraint else {
            panic!("expected a linear constraint");
        };

        assert_eq!(linear.vars, vec![0]);
        assert_eq!(linear.coeffs, vec![1]);
        assert_eq!(linear.domain, vec![3, 3]);
    }

    #[test]
    fn model_to_cp_sat_translates_boolean_equality_constraint() {
        let mut model = Model::new(Arc::new(RwLock::new(Context::default())));
        let flag_decl = DeclarationPtr::new_find(Name::User("flag".into()), Domain::bool());
        model
            .add_symbol(flag_decl.clone())
            .expect("flag should be inserted");
        model.add_constraint(Expression::Eq(
            Metadata::new(),
            Moo::new(Expression::from(Atom::new_ref(flag_decl))),
            Moo::new(Expression::from(true)),
        ));

        let (cp_model, _vars) =
            model_to_cp_sat(model).expect("boolean equality should convert");

        let constraint = &cp_model.constraints[0];
        let Some(constraint_proto::Constraint::Linear(linear)) = &constraint.constraint else {
            panic!("expected a linear constraint");
        };

        assert_eq!(linear.vars, vec![0]);
        assert_eq!(linear.coeffs, vec![1]);
        assert_eq!(linear.domain, vec![1, 1]);
    }

    #[test]
    fn model_to_cp_sat_translates_top_level_boolean_reference_constraint() {
        let mut model = Model::new(Arc::new(RwLock::new(Context::default())));
        let flag_decl = DeclarationPtr::new_find(Name::User("flag".into()), Domain::bool());
        model
            .add_symbol(flag_decl.clone())
            .expect("flag should be inserted");
        model.add_constraint(Expression::from(Atom::new_ref(flag_decl)));

        let (cp_model, _vars) =
            model_to_cp_sat(model).expect("top-level boolean reference should convert");

        let constraint = &cp_model.constraints[0];
        let Some(constraint_proto::Constraint::Linear(linear)) = &constraint.constraint else {
            panic!("expected a linear constraint");
        };

        assert_eq!(linear.vars, vec![0]);
        assert_eq!(linear.coeffs, vec![1]);
        assert_eq!(linear.domain, vec![1, 1]);
    }

    #[test]
    fn model_to_cp_sat_translates_top_level_not_boolean_constraint() {
        let mut model = Model::new(Arc::new(RwLock::new(Context::default())));
        let flag_decl = DeclarationPtr::new_find(Name::User("flag".into()), Domain::bool());
        model
            .add_symbol(flag_decl.clone())
            .expect("flag should be inserted");
        model.add_constraint(Expression::Not(
            Metadata::new(),
            Moo::new(Expression::from(Atom::new_ref(flag_decl))),
        ));

        let (cp_model, _vars) =
            model_to_cp_sat(model).expect("top-level not boolean should convert");

        let constraint = &cp_model.constraints[0];
        let Some(constraint_proto::Constraint::Linear(linear)) = &constraint.constraint else {
            panic!("expected a linear constraint");
        };

        assert_eq!(linear.vars, vec![0]);
        assert_eq!(linear.coeffs, vec![1]);
        assert_eq!(linear.domain, vec![0, 0]);
    }
}
