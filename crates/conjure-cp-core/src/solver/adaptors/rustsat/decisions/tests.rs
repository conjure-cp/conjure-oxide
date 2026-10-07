use super::*;
use crate::ast::{DeclarationPtr, Domain, Metadata, Moo, Reference};
use crate::solver::adaptors::rustsat::adaptor::SatSolver;
use rustsat::{
    instances::{BasicVarManager, Cnf},
    solvers::{Solve, SolveIncremental, SolverResult},
};

#[test]
fn rebuilt_boolean_projections_share_literals_across_batches() {
    use crate::ast::sat_decision::{
        EncodingSelection, IntegerRelation, PbEncoding, SelectionProvenance,
    };
    use rustsat::instances::ManageVars;

    for algorithm in PbEncoding::ALL {
        let inputs: Vec<_> = ["a", "b"]
            .into_iter()
            .map(|name| DeclarationPtr::new_find(Name::User(name.into()), Domain::bool()))
            .collect();
        let expressions: Vec<Expression> = inputs
            .iter()
            .map(|decl| Reference::new(decl.clone()).into())
            .collect();
        let mut cache = EncodingCache::default();
        let mut map = HashMap::new();
        let mut solver = SatSolver::default();
        let mut next_free = 0;
        let mut previous_projection = None;
        let mut outputs = Vec::new();
        let mut named_projections = Vec::new();
        for batch in 0..3 {
            let mut instance: SatInstance = SatInstance::new();
            instance
                .var_manager_mut()
                .increase_next_free(rustsat::types::Var::new(next_free));
            let declarations: Vec<_> = (0..4)
                .map(|index| {
                    DeclarationPtr::new_find(
                        Name::User(format!("batch{batch}_{index}").into()),
                        Domain::bool(),
                    )
                })
                .collect();
            let refs: Vec<Expression> = declarations
                .iter()
                .map(|decl| Reference::new(decl.clone()).into())
                .collect();
            let mut children = expressions.clone();
            if batch == 1 {
                children.reverse();
            }
            let conjunction = Expression::And(
                Metadata::new(),
                Moo::new(crate::into_matrix_expr!(children)),
            );
            let decisions = [
                SatEncodingDecision::Boolean {
                    output: refs[0].clone(),
                    expression: conjunction,
                },
                SatEncodingDecision::Boolean {
                    output: refs[1].clone(),
                    expression: Expression::Not(Metadata::new(), Moo::new(refs[0].clone())),
                },
                SatEncodingDecision::Boolean {
                    output: refs[2].clone(),
                    expression: Expression::Not(Metadata::new(), Moo::new(refs[1].clone())),
                },
                SatEncodingDecision::IntegerRelation {
                    output: refs[3].clone(),
                    terms: vec![(3, refs[2].clone()), (2, expressions[0].clone())],
                    groups: vec![],
                    relation: IntegerRelation::LessEqual,
                    bound: if batch == 1 { 0 } else { 3 },
                    encoding: Some(EncodingSelection {
                        algorithm,
                        provenance: SelectionProvenance::ExplicitConfiguration,
                    }),
                },
            ];
            compile_decisions_with_cache(&decisions, &mut instance, &mut map, &mut cache).unwrap();
            let compiler = Compiler {
                instance: &mut instance,
                variables: &mut map,
                counters: Some(&mut cache),
            };
            let projection =
                compiler.resolve_alias(Term::Literal(compiler.variables[&declarations[2].name()]));
            if let Some(previous) = previous_projection {
                assert!(projection == previous);
            }
            previous_projection = Some(projection);
            assert_eq!(
                cache.gates.len(),
                1,
                "{algorithm}: decoder gate should be shared"
            );
            named_projections.push(
                declarations[..3]
                    .iter()
                    .map(|decl| map[&decl.name()])
                    .collect::<Vec<_>>(),
            );
            outputs.push((map[&declarations[3].name()], if batch == 1 { 0 } else { 3 }));
            let (cnf, manager): (Cnf, BasicVarManager) = instance.into_cnf();
            if batch == 2 {
                assert_eq!(
                    manager.n_used(),
                    next_free + 4,
                    "{algorithm}: repeated projections and bounds need only new named outputs"
                );
            }
            next_free = manager.n_used();
            solver.add_cnf(cnf).unwrap();
        }
        for bits in 0..4 {
            let a = bits & 1 != 0;
            let b = bits & 2 != 0;
            let value = 3 * i64::from(a && b) + 2 * i64::from(a);
            let mut assumptions: Vec<_> = inputs
                .iter()
                .zip([a, b])
                .map(|(decl, truth)| {
                    let lit = map[&decl.name()];
                    if truth { lit } else { !lit }
                })
                .collect();
            for projections in &named_projections {
                assumptions.extend(
                    projections
                        .iter()
                        .zip([a && b, !(a && b), a && b])
                        .map(|(&lit, truth)| if truth { lit } else { !lit }),
                );
            }
            assumptions.extend(
                outputs
                    .iter()
                    .map(|&(lit, bound)| if value <= bound { lit } else { !lit }),
            );
            assert_eq!(
                solver.solve_assumps(&assumptions).unwrap(),
                SolverResult::Sat
            );
            for index in 2..assumptions.len() {
                assumptions[index] = !assumptions[index];
                assert_eq!(
                    solver.solve_assumps(&assumptions).unwrap(),
                    SolverResult::Unsat,
                    "{algorithm}: batch projection/output {index}, inputs {bits}"
                );
                assumptions[index] = !assumptions[index];
            }
        }
    }
}

#[test]
fn boolean_aliases_retain_existing_uses_and_multiple_definitions() {
    let vars: Vec<_> = ["a", "out"]
        .into_iter()
        .map(|name| DeclarationPtr::new_find(Name::User(name.into()), Domain::bool()))
        .collect();
    let refs: Vec<Expression> = vars
        .iter()
        .map(|decl| Reference::new(decl.clone()).into())
        .collect();
    let mut instance = SatInstance::new();
    let mut map = HashMap::new();
    let decisions = [
        SatEncodingDecision::Assert(refs[1].clone()),
        SatEncodingDecision::Boolean {
            output: refs[1].clone(),
            expression: Expression::Not(Metadata::new(), Moo::new(refs[0].clone())),
        },
        SatEncodingDecision::Boolean {
            output: refs[1].clone(),
            expression: true.into(),
        },
    ];
    compile_decisions(&decisions, &mut instance, &mut map).unwrap();
    let (cnf, _): (Cnf, BasicVarManager) = instance.into_cnf();
    let mut solver = SatSolver::default();
    solver.add_cnf(cnf).unwrap();
    for a in [false, true] {
        let lit = map[&vars[0].name()];
        assert_eq!(
            solver.solve_assumps(&[if a { lit } else { !lit }]).unwrap() == SolverResult::Sat,
            !a
        );
    }
}

#[test]
fn semantic_gates_preserve_all_input_and_output_assignments() {
    let variables: Vec<_> = ["a", "b", "out"]
        .into_iter()
        .map(|name| DeclarationPtr::new_find(Name::User(name.into()), Domain::bool()))
        .collect();
    let expressions: Vec<Expression> = variables
        .iter()
        .map(|decl| Reference::new(decl.clone()).into())
        .collect();
    let a = expressions[0].clone();
    let b = expressions[1].clone();
    let matrix = || Moo::new(crate::into_matrix_expr!(vec![a.clone(), b.clone()]));
    let gates = [
        Expression::And(Metadata::new(), matrix()),
        Expression::Or(Metadata::new(), matrix()),
        Expression::Not(Metadata::new(), Moo::new(a.clone())),
        Expression::Iff(Metadata::new(), Moo::new(a.clone()), Moo::new(b.clone())),
        Expression::Imply(Metadata::new(), Moo::new(a), Moo::new(b)),
    ];
    for (gate, expression) in gates.into_iter().enumerate() {
        let mut instance = SatInstance::new();
        let mut map = HashMap::new();
        for variable in &variables {
            map.insert(variable.name().clone(), instance.new_lit());
        }
        compile_decisions(
            &[SatEncodingDecision::Boolean {
                output: expressions[2].clone(),
                expression,
            }],
            &mut instance,
            &mut map,
        )
        .unwrap();
        let (cnf, _): (Cnf, BasicVarManager) = instance.into_cnf();
        let mut solver = SatSolver::default();
        solver.add_cnf(cnf).unwrap();
        for bits in 0..8 {
            let a = bits & 1 != 0;
            let b = bits & 2 != 0;
            let out = bits & 4 != 0;
            let expected = [a && b, a || b, !a, a == b, !a || b][gate];
            let assumptions: Vec<_> = variables
                .iter()
                .zip([a, b, out])
                .map(|(variable, value)| {
                    let lit = map[&variable.name()];
                    if value { lit } else { !lit }
                })
                .collect();
            assert_eq!(
                solver.solve_assumps(&assumptions).unwrap() == SolverResult::Sat,
                out == expected,
                "gate {gate}, assignment {bits}"
            );
        }
    }
}

#[test]
fn assertions_preserve_every_assignment_in_both_polarities() {
    use crate::ast::{Literal, eval_constant};
    use uniplate::Uniplate;
    let vars: Vec<_> = ["a", "b", "c"]
        .into_iter()
        .map(|name| DeclarationPtr::new_find(Name::user(name), Domain::bool()))
        .collect();
    let refs: Vec<Expression> = vars
        .iter()
        .map(|v| Reference::new(v.clone()).into())
        .collect();
    let and = |inputs| Expression::And(Metadata::new(), Moo::new(crate::into_matrix_expr!(inputs)));
    let or = |inputs| Expression::Or(Metadata::new(), Moo::new(crate::into_matrix_expr!(inputs)));
    let not = |input| Expression::Not(Metadata::new(), Moo::new(input));
    let [a, b, c] = refs.as_slice() else {
        unreachable!()
    };
    let expressions = [
        Expression::Imply(
            Metadata::new(),
            Moo::new(and(vec![a.clone(), b.clone()])),
            Moo::new(c.clone()),
        ),
        or(vec![a.clone(), and(vec![b.clone(), c.clone()])]),
        not(and(vec![or(vec![a.clone(), b.clone()]), c.clone()])),
        Expression::Iff(
            Metadata::new(),
            Moo::new(a.clone()),
            Moo::new(or(vec![b.clone(), c.clone()])),
        ),
        Expression::Eq(
            Metadata::new(),
            Moo::new(a.clone()),
            Moo::new(and(vec![b.clone(), c.clone()])),
        ),
        Expression::Neq(
            Metadata::new(),
            Moo::new(or(vec![a.clone(), b.clone()])),
            Moo::new(c.clone()),
        ),
        and(vec![]),
        or(vec![]),
        and(vec![a.clone(), not(a.clone())]),
        or(vec![a.clone(), not(a.clone())]),
        and(vec![a.clone(), a.clone(), true.into()]),
        or(vec![a.clone(), a.clone(), false.into()]),
        not(not(a.clone())),
        or(vec![
            a.clone(),
            and(vec![b.clone(), or(vec![a.clone(), c.clone()])]),
        ]),
    ];
    for expression in expressions {
        for truth in [false, true] {
            let mut instance: SatInstance = SatInstance::new();
            let mut map = HashMap::new();
            for var in &vars {
                map.insert(var.name().clone(), instance.new_lit());
            }
            let assertion = if truth {
                expression.clone()
            } else {
                not(expression.clone())
            };
            compile_decisions(
                &[SatEncodingDecision::Assert(assertion)],
                &mut instance,
                &mut map,
            )
            .unwrap();
            let (cnf, _): (Cnf, BasicVarManager) = instance.into_cnf();
            let mut solver = SatSolver::default();
            solver.add_cnf(cnf).unwrap();
            for bits in 0..8 {
                let grounded = expression.clone().transform(&|input| match input {
                    Expression::Atomic(_, Atom::Reference(reference)) => {
                        let index = vars
                            .iter()
                            .position(|var| *var.name() == *reference.name())
                            .unwrap();
                        (bits & (1 << index) != 0).into()
                    }
                    other => other,
                });
                let expected = eval_constant(&grounded) == Some(Literal::Bool(truth));
                let assumptions: Vec<_> = vars
                    .iter()
                    .enumerate()
                    .map(|(i, var)| {
                        let lit = map[&var.name()];
                        if bits & (1 << i) != 0 { lit } else { !lit }
                    })
                    .collect();
                assert_eq!(
                    solver.solve_assumps(&assumptions).unwrap() == SolverResult::Sat,
                    expected,
                    "{expression}, truth={truth}, bits={bits}"
                );
            }
        }
    }
}

#[test]
fn asserted_transitivity_is_one_library_clause_without_gate_variables() {
    use rustsat::instances::ManageVars;
    let vars: Vec<_> = ["a", "b", "c"]
        .into_iter()
        .map(|name| DeclarationPtr::new_find(Name::user(name), Domain::bool()))
        .collect();
    let refs: Vec<Expression> = vars
        .iter()
        .map(|v| Reference::new(v.clone()).into())
        .collect();
    let expression = Expression::Imply(
        Metadata::new(),
        Moo::new(Expression::And(
            Metadata::new(),
            Moo::new(crate::into_matrix_expr!(refs[..2].to_vec())),
        )),
        Moo::new(refs[2].clone()),
    );
    let mut instance: SatInstance = SatInstance::new();
    compile_decisions(
        &[SatEncodingDecision::Assert(expression)],
        &mut instance,
        &mut HashMap::new(),
    )
    .unwrap();
    let (cnf, manager): (Cnf, BasicVarManager) = instance.into_cnf();
    assert_eq!(manager.n_used(), 3);
    assert_eq!(cnf.len(), 1);
    assert_eq!(cnf[0].len(), 3);
}

#[test]
fn assertion_witness_upgrades_to_equivalence_across_solver_batches() {
    use rustsat::instances::ManageVars;
    for and in [false, true] {
        let vars: Vec<_> = ["a", "b", "c", "out"]
            .into_iter()
            .map(|name| DeclarationPtr::new_find(Name::user(name), Domain::bool()))
            .collect();
        let refs: Vec<Expression> = vars
            .iter()
            .map(|v| Reference::new(v.clone()).into())
            .collect();
        let predicate = if and {
            Expression::And(
                Metadata::new(),
                Moo::new(crate::into_matrix_expr!(refs[..2].to_vec())),
            )
        } else {
            Expression::Or(
                Metadata::new(),
                Moo::new(crate::into_matrix_expr!(refs[..2].to_vec())),
            )
        };
        let witness = if and {
            predicate.clone()
        } else {
            Expression::And(
                Metadata::new(),
                Moo::new(crate::into_matrix_expr!(vec![
                    refs[0].clone(),
                    predicate.clone()
                ])),
            )
        };
        let assertion = Expression::Or(
            Metadata::new(),
            Moo::new(crate::into_matrix_expr!(vec![refs[2].clone(), witness])),
        );
        let mut instance: SatInstance = SatInstance::new();
        let mut map = HashMap::new();
        let mut cache = EncodingCache::default();
        compile_decisions_with_cache(
            &[SatEncodingDecision::Assert(assertion)],
            &mut instance,
            &mut map,
            &mut cache,
        )
        .unwrap();
        assert_eq!(cache.implied_gates.len(), if and { 1 } else { 2 });
        assert!(cache.gates.is_empty());
        let (cnf, manager): (Cnf, BasicVarManager) = instance.into_cnf();
        assert_eq!(manager.n_used(), if and { 4 } else { 5 });
        assert_eq!(cnf.len(), if and { 3 } else { 4 });
        let mut solver = SatSolver::default();
        solver.add_cnf(cnf).unwrap();
        let mut instance: SatInstance = SatInstance::new();
        instance
            .var_manager_mut()
            .increase_next_free(rustsat::types::Var::new(manager.n_used()));
        compile_decisions_with_cache(
            &[SatEncodingDecision::Boolean {
                output: refs[3].clone(),
                expression: predicate,
            }],
            &mut instance,
            &mut map,
            &mut cache,
        )
        .unwrap();
        assert_eq!(cache.implied_gates.len(), if and { 0 } else { 1 });
        assert_eq!(cache.gates.len(), 1);
        let (cnf, manager): (Cnf, BasicVarManager) = instance.into_cnf();
        assert_eq!(manager.n_used(), if and { 5 } else { 6 });
        solver.add_cnf(cnf).unwrap();
        for bits in 0..16 {
            let values: Vec<_> = (0..4).map(|i| bits & (1 << i) != 0).collect();
            let predicate = if and {
                values[0] && values[1]
            } else {
                values[0] || values[1]
            };
            let witness = if and {
                predicate
            } else {
                values[0] && predicate
            };
            let expected = (values[2] || witness) && values[3] == predicate;
            let assumptions: Vec<_> = vars
                .iter()
                .zip(values)
                .map(|(var, value)| {
                    let literal = map[&var.name()];
                    if value { literal } else { !literal }
                })
                .collect();
            assert_eq!(
                solver.solve_assumps(&assumptions).unwrap() == SolverResult::Sat,
                expected,
                "assignment {bits}"
            );
        }
    }
}
