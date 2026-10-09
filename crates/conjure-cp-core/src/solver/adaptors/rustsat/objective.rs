//! Retain native bound encoders while an objective improves.
use std::collections::HashMap;

use rustsat::{
    encodings::pb::{
        BinaryAdder, BoundUpper, BoundUpperIncremental, DynamicPolyWatchdog, GeneralizedTotalizer,
    },
    instances::{Cnf, SatInstance},
    types::{Assignment, Clause, Lit, TernaryVal},
};

use super::decisions::{EncodingCache, compile_decisions_with_cache, compile_objective_terms};
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
        cache: &mut EncodingCache,
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
        let (constant, positive) =
            compile_objective_terms(value, *minimise, instance, variables, cache)?;
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
        cache: &mut EncodingCache,
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
                compile_decisions_with_cache(
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
                    cache,
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
