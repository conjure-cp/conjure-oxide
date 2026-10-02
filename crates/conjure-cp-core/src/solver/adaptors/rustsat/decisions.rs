//! Generate solver clauses directly from semantic Boolean encoding decisions.
use crate::{
    ast::{AbstractLiteral, Atom, Expression, Literal, Name, SatEncodingDecision},
    solver::SolverError,
};
use rustsat::{
    encodings::atomics,
    instances::{Cnf, SatInstance},
    types::{Clause, Lit},
};
use std::collections::HashMap;

#[derive(Clone, Copy)]
enum Term {
    Constant(bool),
    Literal(Lit),
}
impl Term {
    fn negated(self) -> Self {
        match self {
            Self::Constant(v) => Self::Constant(!v),
            Self::Literal(l) => Self::Literal(!l),
        }
    }
}

/// Compile semantic decisions using the instance's shared variable allocator.
pub fn compile_decisions(
    decisions: &[SatEncodingDecision],
    instance: &mut SatInstance,
    variables: &mut HashMap<Name, Lit>,
) -> Result<(), SolverError> {
    let mut compiler = Compiler {
        instance,
        variables,
    };
    for decision in decisions {
        match decision {
            SatEncodingDecision::AtMostOne { inputs, encoding } => {
                let algorithm = encoding
                    .as_ref()
                    .ok_or_else(|| {
                        SolverError::ModelInvalid("Unresolved AMO encoding decision".into())
                    })?
                    .algorithm;
                let terms = inputs
                    .iter()
                    .map(|input| compiler.encode(input))
                    .collect::<Result<Vec<_>, _>>()?;
                let true_count = terms
                    .iter()
                    .filter(|term| matches!(term, Term::Constant(true)))
                    .count();
                let literals = terms
                    .into_iter()
                    .filter_map(|term| match term {
                        Term::Literal(lit) => Some(lit),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if true_count >= 2 {
                    compiler.assert(Term::Constant(false));
                } else if true_count == 1 {
                    for lit in literals {
                        compiler.assert(Term::Literal(!lit));
                    }
                } else {
                    compiler.amo(algorithm, literals)?;
                }
            }
            SatEncodingDecision::Assert(expression) => {
                let term = compiler.encode(expression)?;
                compiler.assert(term);
            }
            SatEncodingDecision::Boolean { output, expression } => {
                let output = compiler.encode(output)?;
                let value = compiler.encode(expression)?;
                match (output, value) {
                    (Term::Literal(output), Term::Literal(value)) => {
                        compiler
                            .instance
                            .add_clause(atomics::lit_impl_lit(output, value));
                        compiler
                            .instance
                            .add_clause(atomics::lit_impl_lit(value, output));
                    }
                    (Term::Constant(value), term) | (term, Term::Constant(value)) => {
                        compiler.assert(if value { term } else { term.negated() })
                    }
                }
            }
        }
    }
    Ok(())
}
struct Compiler<'a> {
    instance: &'a mut SatInstance,
    variables: &'a mut HashMap<Name, Lit>,
}
impl Compiler<'_> {
    fn amo(
        &mut self,
        algorithm: crate::ast::sat_decision::AmoEncoding,
        inputs: Vec<Lit>,
    ) -> Result<(), SolverError> {
        use crate::ast::sat_decision::AmoEncoding;
        use rustsat::encodings::am1::{self, Encode};
        let mut cnf = Cnf::new();
        let manager = self.instance.var_manager_mut();
        let result = match algorithm {
            AmoEncoding::Pairwise => am1::Pairwise::from(inputs).encode(&mut cnf, manager),
            AmoEncoding::Ladder => am1::Ladder::from(inputs).encode(&mut cnf, manager),
            AmoEncoding::Bitwise => am1::Bitwise::from(inputs).encode(&mut cnf, manager),
            AmoEncoding::Commander => am1::Commander::<4>::from(inputs).encode(&mut cnf, manager),
            AmoEncoding::Bimander => am1::Bimander::<4>::from(inputs).encode(&mut cnf, manager),
            AmoEncoding::TwoProduct => {
                am1::TwoProduct::<am1::Pairwise>::from(inputs).encode(&mut cnf, manager)
            }
        };
        result.map_err(|error| SolverError::Runtime(format!("AMO encoder failed: {error}")))?;
        for clause in cnf {
            self.instance.add_clause(clause);
        }
        Ok(())
    }
    fn assert(&mut self, term: Term) {
        match term {
            Term::Constant(true) => (),
            Term::Constant(false) => self.instance.add_clause(Clause::new()),
            Term::Literal(lit) => self.instance.add_clause([lit].into_iter().collect()),
        }
    }
    fn combine(&mut self, and: bool, terms: Vec<Term>) -> Term {
        let mut literals = Vec::new();
        for term in terms {
            match term {
                Term::Constant(value) if value != and => return Term::Constant(value),
                Term::Constant(_) => (),
                Term::Literal(lit) => literals.push(lit),
            }
        }
        match literals.as_slice() {
            [] => Term::Constant(and),
            [lit] => Term::Literal(*lit),
            _ => {
                let output = self.instance.new_lit();
                if and {
                    for clause in atomics::lit_impl_cube(output, &literals) {
                        self.instance.add_clause(clause);
                    }
                    self.instance
                        .add_clause(atomics::cube_impl_lit(&literals, output));
                } else {
                    for clause in atomics::clause_impl_lit(&literals, output) {
                        self.instance.add_clause(clause);
                    }
                    self.instance
                        .add_clause(atomics::lit_impl_clause(output, &literals));
                }
                Term::Literal(output)
            }
        }
    }
    fn encode(&mut self, expression: &Expression) -> Result<Term, SolverError> {
        Ok(match expression {
            Expression::Atomic(_, Atom::Literal(Literal::Bool(value))) => Term::Constant(*value),
            Expression::Atomic(_, Atom::Reference(reference)) => {
                if !reference.domain().is_some_and(|domain| domain.is_bool()) {
                    return Err(SolverError::ModelInvalid(format!(
                        "Non-Boolean SAT reference: {reference}"
                    )));
                }
                Term::Literal(
                    *self
                        .variables
                        .entry(reference.name().clone())
                        .or_insert_with(|| self.instance.new_lit()),
                )
            }
            Expression::Not(_, inner) => self.encode(inner)?.negated(),
            Expression::And(_, children) | Expression::Or(_, children) => {
                let Expression::AbstractLiteral(_, AbstractLiteral::Matrix(children, _)) =
                    children.as_ref()
                else {
                    return Err(SolverError::ModelInvalid(
                        "SAT Boolean operands must be an explicit matrix".into(),
                    ));
                };
                let terms = children
                    .iter()
                    .map(|child| self.encode(child))
                    .collect::<Result<Vec<_>, _>>()?;
                self.combine(matches!(expression, Expression::And(..)), terms)
            }
            Expression::Imply(_, left, right) => {
                let left = self.encode(left)?.negated();
                let right = self.encode(right)?;
                self.combine(false, vec![left, right])
            }
            Expression::Iff(_, left, right) => {
                let left = self.encode(left)?;
                let right = self.encode(right)?;
                let forward = self.combine(false, vec![left.negated(), right]);
                let backward = self.combine(false, vec![right.negated(), left]);
                self.combine(true, vec![forward, backward])
            }
            _ => {
                return Err(SolverError::ModelInvalid(format!(
                    "Unsupported semantic SAT operation: {expression}"
                )));
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{DeclarationPtr, Domain, Metadata, Moo, Reference};
    use rustsat::{
        instances::{BasicVarManager, Cnf},
        solvers::{Solve, SolveIncremental, SolverResult},
    };
    use rustsat_cadical::CaDiCaL;

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
}

#[cfg(test)]
mod amo_tests {
    use super::*;
    use crate::ast::sat_decision::{AmoEncoding, EncodingSelection, SelectionProvenance};
    use crate::ast::{DeclarationPtr, Domain, Reference};
    use rustsat::{
        instances::{BasicVarManager, Cnf},
        solvers::{Solve, SolveIncremental, SolverResult},
    };
    use rustsat_cadical::CaDiCaL;

    fn check(
        inputs: Vec<Expression>,
        variables: &[DeclarationPtr],
        algorithm: AmoEncoding,
        expected: impl Fn(usize) -> bool,
    ) {
        let mut instance = SatInstance::new();
        let mut map = HashMap::new();
        for variable in variables {
            map.insert(variable.name().clone(), instance.new_lit());
        }
        compile_decisions(
            &[SatEncodingDecision::AtMostOne {
                inputs,
                encoding: Some(EncodingSelection {
                    algorithm,
                    provenance: SelectionProvenance::ExplicitConfiguration,
                }),
            }],
            &mut instance,
            &mut map,
        )
        .unwrap();
        let (cnf, _): (Cnf, BasicVarManager) = instance.into_cnf();
        let mut solver = CaDiCaL::default();
        solver.add_cnf(cnf).unwrap();
        for bits in 0..(1usize << variables.len()) {
            let assumptions: Vec<_> = variables
                .iter()
                .enumerate()
                .map(|(index, variable)| {
                    let literal = map[&variable.name()];
                    if bits & (1 << index) != 0 {
                        literal
                    } else {
                        !literal
                    }
                })
                .collect();
            assert_eq!(
                solver.solve_assumps(&assumptions).unwrap() == SolverResult::Sat,
                expected(bits),
                "{algorithm}, assignment {bits}"
            );
        }
    }
    #[test]
    fn every_amo_encoder_preserves_projection_and_boundary_sizes() {
        for algorithm in AmoEncoding::ALL {
            for size in [0, 1, 2, 3, 4, 5, 8, 9] {
                let variables: Vec<_> = (0..size)
                    .map(|index| {
                        DeclarationPtr::new_find(
                            Name::User(format!("x{index}").into()),
                            Domain::bool(),
                        )
                    })
                    .collect();
                let inputs = variables
                    .iter()
                    .map(|variable| Reference::new(variable.clone()).into())
                    .collect();
                check(inputs, &variables, algorithm, |bits| bits.count_ones() <= 1);
            }
        }
    }
    #[test]
    fn amo_constants_and_repeated_inputs_keep_their_multiplicity() {
        let variable = DeclarationPtr::new_find(Name::User("x".into()), Domain::bool());
        let x: Expression = Reference::new(variable.clone()).into();
        for algorithm in AmoEncoding::ALL {
            check(
                vec![x.clone(), x.clone()],
                std::slice::from_ref(&variable),
                algorithm,
                |bits| bits == 0,
            );
            check(
                vec![true.into(), x.clone(), false.into()],
                std::slice::from_ref(&variable),
                algorithm,
                |bits| bits == 0,
            );
            check(
                vec![true.into(), true.into(), x.clone()],
                std::slice::from_ref(&variable),
                algorithm,
                |_| false,
            );
        }
    }
    #[test]
    fn unresolved_decisions_are_rejected_at_the_solver_boundary() {
        assert!(
            compile_decisions(
                &[SatEncodingDecision::AtMostOne {
                    inputs: vec![],
                    encoding: None
                }],
                &mut SatInstance::new(),
                &mut HashMap::new()
            )
            .is_err()
        );
    }
}
