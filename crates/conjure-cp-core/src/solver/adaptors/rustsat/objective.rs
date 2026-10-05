//! Retain native bound encoders while an objective improves.
use std::collections::HashMap;

use rustsat::{
    encodings::pb::{
        BinaryAdder, BoundUpper, BoundUpperIncremental, DynamicPolyWatchdog, GeneralizedTotalizer,
    },
    instances::{Cnf, SatInstance},
    types::{Assignment, Clause, Lit, TernaryVal},
};

use super::decisions::{compile_decisions, compile_objective_terms};
use crate::{
    ast::{
        Name, SatEncodingDecision,
        sat_decision::{EncodingSelection, IntegerRelation, PbEncoding, SatIntegerView},
    },
    solver::SolverError,
};

// Each instance owns a fixed input set. Tightening extends its existing bound encoding.
enum UpperEncoder {
    Totalizer(GeneralizedTotalizer),
    Adder(BinaryAdder),
    Watchdog(DynamicPolyWatchdog),
    OneShot,
}

/// Solve-time objective state, separate from the semantic model decision.
pub(super) struct CompiledObjective {
    minimise: bool,
    constant: i128,
    positive: Vec<(Lit, usize)>,
    value: SatIntegerView,
    selection: EncodingSelection<PbEncoding>,
    encoder: UpperEncoder,
}

impl CompiledObjective {
    /// Compile the actual-value view and construct a lazy native upper-bound encoder.
    pub(super) fn new(
        decision: &SatEncodingDecision,
        instance: &mut SatInstance,
        variables: &mut HashMap<Name, Lit>,
    ) -> Result<Self, SolverError> {
        let SatEncodingDecision::Objective {
            minimise,
            value,
            encoding,
        } = decision
        else {
            return Err(SolverError::ModelInvalid(
                "Expected an objective decision".into(),
            ));
        };
        let selection = encoding.clone().ok_or_else(|| {
            SolverError::ModelInvalid("Unresolved objective PB encoding decision".into())
        })?;
        let (constant, positive) = compile_objective_terms(value, *minimise, instance, variables)?;
        let encoder = match selection.algorithm {
            PbEncoding::RustsatGeneralizedTotalizer => {
                UpperEncoder::Totalizer(positive.iter().copied().collect())
            }
            PbEncoding::RustsatBinaryAdder => {
                UpperEncoder::Adder(positive.iter().copied().collect())
            }
            PbEncoding::RustsatDynamicPolyWatchdog => {
                UpperEncoder::Watchdog(positive.iter().copied().collect())
            }
            PbEncoding::PindakaasBdd | PbEncoding::PindakaasSwc => UpperEncoder::OneShot,
        };
        Ok(Self {
            minimise: *minimise,
            constant,
            positive,
            value: value.clone(),
            selection,
            encoder,
        })
    }

    /// Add a strictly improving bound, reusing native state and the shared allocation frontier.
    pub(super) fn tighten(
        &mut self,
        solution: &Assignment,
        instance: &mut SatInstance,
        variables: &mut HashMap<Name, Lit>,
    ) -> Result<Vec<Lit>, SolverError> {
        let current: usize = self
            .positive
            .iter()
            .filter(|(literal, _)| solution.lit_value(*literal) == TernaryVal::True)
            .map(|(_, weight)| *weight)
            .sum();
        let Some(bound) = current.checked_sub(1) else {
            instance.add_clause(Clause::new());
            return Ok(vec![]);
        };
        let mut cnf = Cnf::new();
        macro_rules! tighten {
            ($encoder:expr) => {{
                $encoder
                    .encode_ub_change(bound..=bound, &mut cnf, instance.var_manager_mut())
                    .map_err(|error| {
                        SolverError::Runtime(format!(
                            "Incremental objective encoding failed: {error}"
                        ))
                    })?;
                $encoder.enforce_ub(bound).map_err(|error| {
                    SolverError::Runtime(format!("Incremental objective bound failed: {error}"))
                })?
            }};
        }
        let units = match &mut self.encoder {
            UpperEncoder::Totalizer(encoder) => tighten!(encoder),
            UpperEncoder::Adder(encoder) => tighten!(encoder),
            UpperEncoder::Watchdog(encoder) => tighten!(encoder),
            UpperEncoder::OneShot => {
                let cost = self.constant + current as i128;
                let actual = if self.minimise { cost } else { -cost };
                let bound = i64::try_from(actual).map_err(|_| {
                    SolverError::ModelInvalid("Objective value exceeds the library range".into())
                })?;
                compile_decisions(
                    &[SatEncodingDecision::IntegerRelation {
                        output: true.into(),
                        terms: self.value.terms.clone(),
                        groups: self.value.groups.clone(),
                        relation: if self.minimise {
                            IntegerRelation::Less
                        } else {
                            IntegerRelation::Greater
                        },
                        bound: bound.checked_sub(self.value.constant).ok_or_else(|| {
                            SolverError::ModelInvalid(
                                "Objective bound exceeds the library range".into(),
                            )
                        })?,
                        encoding: Some(self.selection.clone()),
                    }],
                    instance,
                    variables,
                )?;
                return Ok(vec![]);
            }
        };
        for clause in cnf {
            instance.add_clause(clause);
        }
        // Enforcement literals are transient: DPW control bits can change between bounds.
        Ok(units)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{
        DeclarationPtr, Domain, Expression, Metadata, Moo, Reference,
        sat_decision::SelectionProvenance,
    };
    use crate::solver::adaptors::rustsat::adaptor::SatSolver;
    use rustsat::{
        instances::{BasicVarManager, ManageVars},
        solvers::{Solve, SolveIncremental, SolverResult},
    };

    #[test]
    fn tightening_preserves_signed_objective_projections_and_reuses_native_state() {
        for minimise in [false, true] {
            for algorithm in PbEncoding::ALL {
                for constant in [i64::MIN + 20, -7, i64::MAX - 20] {
                    let refs: Vec<_> = ["a", "b", "c"]
                        .into_iter()
                        .map(|name| {
                            Expression::from(Reference::new(DeclarationPtr::new_find(
                                Name::user(name),
                                Domain::bool(),
                            )))
                        })
                        .collect();
                    let value = SatIntegerView {
                        constant,
                        terms: vec![
                            (3, refs[0].clone()),
                            (-2, refs[1].clone()),
                            (1, refs[0].clone()),
                            (
                                2,
                                Expression::Not(Metadata::new(), Moo::new(refs[2].clone())),
                            ),
                            (1, true.into()),
                            (9, false.into()),
                        ],
                        groups: vec![],
                        choices: None,
                    };
                    let decision = SatEncodingDecision::Objective {
                        minimise,
                        value,
                        encoding: Some(EncodingSelection {
                            algorithm,
                            provenance: SelectionProvenance::ExplicitConfiguration,
                        }),
                    };
                    let mut instance = SatInstance::new();
                    let mut variables = HashMap::new();
                    let mut objective =
                        CompiledObjective::new(&decision, &mut instance, &mut variables).unwrap();
                    let lits: Vec<_> = ["a", "b", "c"]
                        .map(|name| variables[&Name::user(name)])
                        .into();
                    let (_, mut manager): (Cnf, BasicVarManager) = instance.into_cnf();
                    let raw = |assignment: usize| {
                        i128::from(constant) + 4 * i128::from(assignment & 1 != 0)
                            - 2 * i128::from(assignment & 2 != 0)
                            + 2 * i128::from(assignment & 4 == 0)
                            + 1
                    };
                    let mut candidates: Vec<_> = (0..8).collect();
                    candidates.sort_by_key(|&assignment| {
                        if minimise {
                            -raw(assignment)
                        } else {
                            raw(assignment)
                        }
                    });
                    candidates.dedup_by_key(|assignment| raw(*assignment));
                    let mut solver = SatSolver::default();
                    for assignment in candidates {
                        let mut completed = Assignment::default();
                        for (bit, literal) in lits.iter().enumerate() {
                            completed.assign_var(
                                literal.var(),
                                TernaryVal::from(assignment & (1 << bit) != 0),
                            );
                        }
                        let mut delta: SatInstance = SatInstance::new();
                        delta
                            .var_manager_mut()
                            .increase_next_free(rustsat::types::Var::new(manager.n_used()));
                        let enforcement = objective
                            .tighten(&completed, &mut delta, &mut variables)
                            .unwrap();
                        let (cnf, new_manager): (Cnf, BasicVarManager) = delta.into_cnf();
                        manager = new_manager;
                        solver.add_cnf(cnf).unwrap();
                        for candidate in 0..8 {
                            let mut assumptions: Vec<_> = lits
                                .iter()
                                .enumerate()
                                .map(|(bit, literal)| {
                                    if candidate & (1 << bit) != 0 {
                                        *literal
                                    } else {
                                        !*literal
                                    }
                                })
                                .collect();
                            assumptions.extend(enforcement.iter().copied());
                            assert_eq!(
                                solver.solve_assumps(&assumptions).unwrap() == SolverResult::Sat,
                                if minimise {
                                    raw(candidate) < raw(assignment)
                                } else {
                                    raw(candidate) > raw(assignment)
                                },
                                "{algorithm:?} minimise={minimise} constant={constant}, current={assignment}, candidate={candidate}"
                            );
                        }
                        if matches!(objective.encoder, UpperEncoder::OneShot) {
                            continue;
                        }
                        let mut repeated: SatInstance = SatInstance::new();
                        repeated
                            .var_manager_mut()
                            .increase_next_free(rustsat::types::Var::new(manager.n_used()));
                        objective
                            .tighten(&completed, &mut repeated, &mut variables)
                            .unwrap();
                        let (cnf, new_manager): (Cnf, BasicVarManager) = repeated.into_cnf();
                        assert_eq!(
                            new_manager.n_used(),
                            manager.n_used(),
                            "native bound must reuse auxiliaries"
                        );
                        assert!(
                            cnf.iter().all(|clause| clause.is_empty()),
                            "repeating a bound reuses clauses (except a minimum-cost contradiction): {algorithm:?} minimise={minimise}, current={assignment}: {cnf:?}"
                        );
                    }
                }
            }
        }
    }
}
