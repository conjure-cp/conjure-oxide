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
                constraint::cardinality_one::{BitwiseEncoder, LadderEncoder, PairwiseEncoder},
                constraint::linear::{LinAggregator, LinExp, LinVariant, Linear},
            };
            let terms = distinct_occurrence_literals(self.instance, &inputs)
                .into_iter()
                .map(|literal| (pind_lit(literal), 1))
                .collect::<Vec<_>>();
            let mut sink = PindakaasSink {
                instance: self.instance,
                guard,
            };
            let variant = LinAggregator::default().aggregate(
                &mut sink,
                &Linear::new(
                    LinExp::from_terms(&terms),
                    pindakaas::constraint::linear::Comparator::LessEq,
                    1,
                ),
            );
            let result = match variant {
                Ok(LinVariant::CardinalityOne(cardinality)) => match algorithm {
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
                Ok(LinVariant::Trivial) => Ok(()),
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
