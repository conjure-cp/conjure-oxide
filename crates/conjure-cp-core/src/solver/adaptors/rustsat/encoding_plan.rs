//! First clause generator for the SAT encoding-plan IR.
//!
//! The selected Boolean Tseitin algorithm is realised directly in RustSAT, without constructing
//! CNF AST expressions. External literals remain private to this adapter. The adaptor accepts explicit terminal decision ASTs. The CLI still builds legacy clauses;
//! integer plans and automatic decision selection are subsequent work.
use crate::ast::encoding_plan::{
    BooleanFormula, EncodingDecision, EncodingPlanKind, RepresentationKind, SemanticDomain,
    SemanticVarId,
};
use anyhow::{Result, anyhow};
use rustsat::encodings::atomics;
use rustsat::instances::{BasicVarManager, Cnf, ManageVars, SatInstance};
use rustsat::solvers::{Solve, SolveIncremental, SolverResult};
use rustsat::types::{Clause, Lit};
use rustsat_cadical::CaDiCaL;
use std::collections::HashMap;

/// Compiled Boolean plans with a private mapping from semantic variables to solver literals.
pub struct CompiledBooleanDecision {
    instance: SatInstance,
    variables: HashMap<SemanticVarId, Lit>,
}
#[derive(Clone, Copy)]
enum Term {
    Constant(bool),
    Literal(Lit),
}
impl Term {
    fn negated(self) -> Self {
        match self {
            Self::Constant(value) => Self::Constant(!value),
            Self::Literal(lit) => Self::Literal(!lit),
        }
    }
}

impl CompiledBooleanDecision {
    pub(super) fn into_parts(self) -> (SatInstance, HashMap<SemanticVarId, Lit>) {
        (self.instance, self.variables)
    }

    pub fn compile(decision: &EncodingDecision) -> Result<Self> {
        decision.validate()?;
        // Check capabilities before allocating literals. No implicit integer representation.
        if decision
            .variables()
            .iter()
            .any(|variable| variable.domain != SemanticDomain::Boolean)
            || decision
                .plans()
                .iter()
                .flat_map(|plan| &plan.requests)
                .any(|request| request.kind != RepresentationKind::Boolean)
        {
            return Err(anyhow!(
                "Boolean Tseitin generator supports only Boolean variables and requests"
            ));
        }
        let mut compiled = Self {
            instance: SatInstance::new(),
            variables: HashMap::new(),
        };
        for variable in decision.variable_ids() {
            compiled
                .variables
                .insert(variable, compiled.instance.new_lit());
        }
        for plan in decision.plans() {
            let EncodingPlanKind::BooleanTseitin { formula } = &plan.kind;
            let term = compiled.encode(formula, decision)?;
            match term {
                Term::Constant(true) => {}
                Term::Constant(false) => compiled.instance.add_clause(Clause::new()),
                Term::Literal(lit) => compiled.instance.add_clause([lit].into_iter().collect()),
            }
        }
        Ok(compiled)
    }

    fn encode(&mut self, formula: &BooleanFormula, decision: &EncodingDecision) -> Result<Term> {
        match formula {
            BooleanFormula::Constant(value) => Ok(Term::Constant(*value)),
            BooleanFormula::Reference(reference) => {
                let variable = decision.reference(*reference)?.variable;
                Ok(Term::Literal(*self.variables.get(&variable).ok_or_else(
                    || anyhow!("Boolean representation was not allocated"),
                )?))
            }
            BooleanFormula::Not(inner) => Ok(self.encode(inner, decision)?.negated()),
            BooleanFormula::And(children) | BooleanFormula::Or(children) => {
                let and = matches!(formula, BooleanFormula::And(_));
                let mut literals = Vec::new();
                for child in children {
                    match self.encode(child, decision)? {
                        Term::Constant(value) if value != and => return Ok(Term::Constant(value)),
                        Term::Constant(_) => {}
                        Term::Literal(lit) => literals.push(lit),
                    }
                }
                match literals.as_slice() {
                    [] => Ok(Term::Constant(and)),
                    [literal] => Ok(Term::Literal(*literal)),
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
                        Ok(Term::Literal(output))
                    }
                }
            }
        }
    }

    /// Check the projected truth value under semantic Boolean assumptions.
    pub fn satisfiable(&self, values: &[(SemanticVarId, bool)]) -> Result<bool> {
        let mut solver = CaDiCaL::default();
        let (cnf, _): (Cnf, BasicVarManager) = self.instance.clone().into_cnf();
        for clause in &cnf {
            solver.add_clause(clause.clone())?;
        }
        let assumptions = values
            .iter()
            .map(|(variable, value)| {
                self.variables
                    .get(variable)
                    .map(|lit| if *value { *lit } else { !*lit })
                    .ok_or_else(|| anyhow!("unknown semantic variable in Boolean assumptions"))
            })
            .collect::<Result<Vec<_>>>()?;
        match solver.solve_assumps(&assumptions)? {
            SolverResult::Sat => Ok(true),
            SolverResult::Unsat => Ok(false),
            SolverResult::Interrupted => Err(anyhow!("SAT checking was interrupted")),
        }
    }

    pub fn write_dimacs(&self, writer: &mut impl std::io::Write) -> std::io::Result<()> {
        let (cnf, manager): (Cnf, BasicVarManager) = self.instance.clone().into_cnf();
        cnf.write_dimacs(writer, manager.n_used())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::encoding_plan::{
        EncodingPlan, PlanProvenance, ReferenceContext, RepresentationRequest, VariableOrigin,
    };
    use crate::ast::{DeclarationPtr, Domain, Name};

    #[test]
    fn nested_boolean_plans_match_the_semantics_for_every_input_assignment() {
        let mut decision = EncodingDecision::default();
        let mut vars = Vec::new();
        let mut refs = Vec::new();
        for name in ["p", "q", "r"] {
            let declaration = DeclarationPtr::new_find(Name::User(name.into()), Domain::bool());
            let variable = decision.intern(&declaration, VariableOrigin::User).unwrap();
            vars.push(variable);
            refs.push(
                decision
                    .new_reference(variable, ReferenceContext::BooleanOperand)
                    .unwrap(),
            );
        }
        let repeated_p = decision.duplicate_reference(refs[0]).unwrap();
        // (p or !q) and (q or r) and !(p and r); repeated uses must share one semantic variable.
        let formula = BooleanFormula::And(vec![
            BooleanFormula::Or(vec![
                BooleanFormula::Reference(refs[0]),
                BooleanFormula::Not(Box::new(BooleanFormula::Reference(refs[1]))),
            ]),
            BooleanFormula::Or(vec![
                BooleanFormula::Reference(refs[1]),
                BooleanFormula::Reference(refs[2]),
            ]),
            BooleanFormula::Not(Box::new(BooleanFormula::And(vec![
                BooleanFormula::Reference(repeated_p),
                BooleanFormula::Reference(refs[2]),
            ]))),
        ]);
        decision
            .add_plan(EncodingPlan {
                source_constraint: 0,
                kind: EncodingPlanKind::BooleanTseitin { formula },
                requests: refs
                    .iter()
                    .chain([&repeated_p])
                    .map(|reference| RepresentationRequest {
                        reference: *reference,
                        kind: RepresentationKind::Boolean,
                    })
                    .collect(),
                provenance: PlanProvenance::ExplicitConfiguration,
            })
            .unwrap();
        let compiled = CompiledBooleanDecision::compile(&decision).unwrap();
        for bits in 0..8 {
            let p = bits & 1 != 0;
            let q = bits & 2 != 0;
            let r = bits & 4 != 0;
            assert_eq!(
                compiled
                    .satisfiable(&[(vars[0], p), (vars[1], q), (vars[2], r)])
                    .unwrap(),
                (p || !q) && (q || r) && !(p && r)
            );
        }
        let mut first = Vec::new();
        compiled.write_dimacs(&mut first).unwrap();
        let mut second = Vec::new();
        CompiledBooleanDecision::compile(&decision)
            .unwrap()
            .write_dimacs(&mut second)
            .unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn integer_variables_are_rejected_without_implicit_boolean_encoding() {
        let mut decision = EncodingDecision::default();
        decision
            .intern(
                &DeclarationPtr::new_find(
                    Name::User("x".into()),
                    Domain::int(vec![crate::ast::Range::Bounded(0, 1)]),
                ),
                VariableOrigin::User,
            )
            .unwrap();
        let error = CompiledBooleanDecision::compile(&decision)
            .err()
            .expect("an integer domain is not a Boolean domain");
        assert!(error.to_string().contains("only Boolean variables"));
    }

    #[test]
    fn empty_boolean_lists_and_constants_preserve_truth_and_contradiction() {
        for (formula, expected) in [
            (BooleanFormula::And(vec![]), true),
            (BooleanFormula::Or(vec![]), false),
            (BooleanFormula::Constant(false), false),
            (
                BooleanFormula::Not(Box::new(BooleanFormula::Constant(false))),
                true,
            ),
        ] {
            let mut decision = EncodingDecision::default();
            decision
                .add_plan(EncodingPlan {
                    source_constraint: 0,
                    kind: EncodingPlanKind::BooleanTseitin { formula },
                    requests: vec![],
                    provenance: PlanProvenance::ExplicitConfiguration,
                })
                .unwrap();
            assert_eq!(
                CompiledBooleanDecision::compile(&decision)
                    .unwrap()
                    .satisfiable(&[])
                    .unwrap(),
                expected
            );
        }
    }
}
