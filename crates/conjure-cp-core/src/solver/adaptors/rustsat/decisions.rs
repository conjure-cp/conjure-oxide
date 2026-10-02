//! Generate solver clauses directly from semantic Boolean encoding decisions.
use crate::{
    ast::{AbstractLiteral, Atom, Expression, Literal, Name, SatEncodingDecision},
    solver::SolverError,
};
use rustsat::{
    encodings::atomics,
    instances::SatInstance,
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
