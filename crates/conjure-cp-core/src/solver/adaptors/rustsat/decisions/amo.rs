//! At-most-one constraints through the library AMO encoders.
use super::*;

impl Compiler<'_> {
    pub(super) fn asserted_amo(
        &mut self,
        algorithm: crate::ast::sat_decision::AmoEncoding,
        terms: Vec<Term>,
    ) -> Result<(), SolverError> {
        self.asserted_amo_guarded(algorithm, terms, None)
    }

    pub(super) fn asserted_amo_guarded(
        &mut self,
        algorithm: crate::ast::sat_decision::AmoEncoding,
        terms: Vec<Term>,
        guard: Option<Lit>,
    ) -> Result<(), SolverError> {
        let true_count = terms
            .iter()
            .filter(|term| matches!(term, Term::Constant(true)))
            .count();
        let literals: Vec<_> = terms
            .into_iter()
            .filter_map(|term| match term {
                Term::Literal(literal) => Some(literal),
                _ => None,
            })
            .collect();
        if true_count >= 2 {
            self.assert_guarded(Term::Constant(false), guard);
        } else if true_count == 1 {
            for literal in literals {
                self.assert_guarded(Term::Literal(!literal), guard);
            }
        } else {
            self.amo_guarded(algorithm, literals, guard)?;
        }
        Ok(())
    }

    pub(super) fn amo_guarded(
        &mut self,
        algorithm: crate::ast::sat_decision::AmoEncoding,
        inputs: Vec<Lit>,
        guard: Option<Lit>,
    ) -> Result<(), SolverError> {
        use crate::ast::sat_decision::AmoEncoding;
        use rustsat::encodings::am1::{self, Encode};
        if inputs.len() <= 1 {
            return Ok(());
        }
        if matches!(
            algorithm,
            AmoEncoding::PindakaasPairwise
                | AmoEncoding::PindakaasLadder
                | AmoEncoding::PindakaasBitwise
        ) {
            use pindakaas::{
                Encoder,
                bool_linear::{BoolLinAggregator, BoolLinExp, BoolLinVariant, BoolLinear},
                cardinality_one::{BitwiseEncoder, LadderEncoder, PairwiseEncoder},
            };
            // The public AMO input requires distinct variables, even for opposite polarities.
            let mut seen = std::collections::HashSet::new();
            let terms = inputs
                .into_iter()
                .map(|literal| {
                    let literal = if seen.insert(literal.var()) {
                        literal
                    } else {
                        let alias = self.instance.new_lit();
                        self.instance
                            .add_clause(atomics::lit_impl_lit(alias, literal));
                        self.instance
                            .add_clause(atomics::lit_impl_lit(literal, alias));
                        alias
                    };
                    (pind_lit(literal), 1)
                })
                .collect::<Vec<_>>();
            let mut sink = PindakaasSink {
                instance: self.instance,
                guard,
            };
            let variant = BoolLinAggregator::default().aggregate(
                &mut sink,
                &BoolLinear::new(
                    BoolLinExp::from_terms(&terms),
                    pindakaas::bool_linear::Comparator::LessEq,
                    1,
                ),
            );
            let result = match variant {
                Ok(BoolLinVariant::CardinalityOne(cardinality)) => match algorithm {
                    AmoEncoding::PindakaasPairwise => {
                        PairwiseEncoder::default().encode(&mut sink, &cardinality)
                    }
                    AmoEncoding::PindakaasLadder => {
                        LadderEncoder::default().encode(&mut sink, &cardinality)
                    }
                    AmoEncoding::PindakaasBitwise => {
                        BitwiseEncoder::default().encode(&mut sink, &cardinality)
                    }
                    _ => unreachable!(),
                },
                Ok(BoolLinVariant::Trivial) => Ok(()),
                Ok(_) => {
                    return Err(SolverError::ModelInvalid(
                        "AMO normalisation unexpectedly produced a different constraint family"
                            .into(),
                    ));
                }
                Err(error) => Err(error),
            };
            if result.is_err() {
                self.assert_guarded(Term::Constant(false), guard);
            }
            return Ok(());
        }
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
            AmoEncoding::PindakaasPairwise
            | AmoEncoding::PindakaasLadder
            | AmoEncoding::PindakaasBitwise => unreachable!(),
        };
        result.map_err(|error| SolverError::Runtime(format!("AMO encoder failed: {error}")))?;
        for mut clause in cnf {
            if let Some(guard) = guard {
                clause.add(!guard);
            }
            self.instance.add_clause(clause);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::sat_decision::{AmoEncoding, EncodingSelection, SelectionProvenance};
    use crate::ast::{DeclarationPtr, Domain, Metadata, Moo, Reference};
    use crate::solver::adaptors::rustsat::adaptor::SatSolver;
    use rustsat::{
        instances::{BasicVarManager, Cnf},
        solvers::{Solve, SolveIncremental, SolverResult},
    };

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
        let mut solver = SatSolver::default();
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
            let not_x = Expression::Not(Metadata::new(), Moo::new(x.clone()));
            check(
                vec![x.clone(), not_x.clone()],
                std::slice::from_ref(&variable),
                algorithm,
                |_| true,
            );
            check(
                vec![x.clone(), not_x, x.clone()],
                std::slice::from_ref(&variable),
                algorithm,
                |bits| bits == 0,
            );
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
