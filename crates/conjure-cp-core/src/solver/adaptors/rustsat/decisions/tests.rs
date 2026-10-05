use super::*;
use crate::ast::{DeclarationPtr, Domain, Metadata, Moo, Reference};
use rustsat::{
    instances::{BasicVarManager, Cnf},
    solvers::{Solve, SolveIncremental, SolverResult},
};
use rustsat_cadical::CaDiCaL;

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
        let mut solver = CaDiCaL::default();
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
    let mut solver = CaDiCaL::default();
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
        let mut solver = CaDiCaL::default();
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
