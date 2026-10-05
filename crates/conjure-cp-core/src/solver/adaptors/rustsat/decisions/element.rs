//! Element constraints over integer views.
use super::*;

impl Compiler<'_> {
    pub(super) fn element(
        &mut self,
        index: &crate::ast::sat_decision::SatIntegerView,
        value: &crate::ast::sat_decision::SatIntegerView,
        entries: &[(i64, crate::ast::sat_decision::SatIntegerView)],
        encoding: &Option<
            crate::ast::sat_decision::EncodingSelection<crate::ast::sat_decision::ElementEncoding>,
        >,
        pb_encoding: &Option<
            crate::ast::sat_decision::EncodingSelection<crate::ast::sat_decision::PbEncoding>,
        >,
    ) -> Result<(), SolverError> {
        use crate::ast::sat_decision::{ElementEncoding, SatIntegerView};
        let algorithm = encoding
            .as_ref()
            .ok_or_else(|| {
                SolverError::ModelInvalid("Unresolved element encoding decision".into())
            })?
            .algorithm;
        let pb = pb_encoding
            .as_ref()
            .ok_or_else(|| {
                SolverError::ModelInvalid("Unresolved element component PB encoding".into())
            })?
            .algorithm;
        let mut labels = std::collections::HashSet::new();
        if entries.iter().any(|(label, _)| !labels.insert(*label)) {
            return Err(SolverError::ModelInvalid(
                "Element index labels must be unique".into(),
            ));
        }
        let mut selectors = Vec::new();
        let mut supports = Vec::new();
        for (label, entry) in entries {
            let label = SatIntegerView {
                constant: *label,
                terms: vec![],
                groups: vec![],
                choices: None,
            };
            let selector = self.view_equality(index, &label, pb)?;
            let equality = self.view_equality(value, entry, pb)?;
            match algorithm {
                ElementEncoding::Implication => {
                    let implication = self.combine(false, vec![selector.negated(), equality]);
                    self.equate(Term::Constant(true), implication);
                }
                ElementEncoding::Support => {
                    selectors.push(selector);
                    supports.push(self.combine(true, vec![selector, equality]));
                }
            }
        }
        if algorithm == ElementEncoding::Support {
            let valid = self.combine(false, selectors);
            supports.push(valid.negated());
            let definition = self.combine(false, supports);
            self.equate(Term::Constant(true), definition);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::sat_decision::{
        ElementEncoding, EncodingSelection, PbEncoding, PbTermGroup, PbTermStructure,
        SatIntegerView, SelectionProvenance,
    };
    use crate::ast::{DeclarationPtr, Domain, Reference};
    use crate::solver::adaptors::rustsat::adaptor::SatSolver;
    use rustsat::{
        instances::{BasicVarManager, Cnf, ManageVars},
        solvers::{Solve, SolveIncremental, SolverResult},
    };

    fn constant(value: i64) -> SatIntegerView {
        SatIntegerView {
            constant: value,
            terms: vec![],
            groups: vec![],
            choices: None,
        }
    }

    #[test]
    fn element_preserves_sparse_values_shared_views_and_free_outside_indices() {
        let variables: Vec<_> = (0..5)
            .map(|i| {
                DeclarationPtr::new_find(Name::User(format!("element_{i}").into()), Domain::bool())
            })
            .collect();
        let bits: Vec<Expression> = variables
            .iter()
            .cloned()
            .map(Reference::new)
            .map(Into::into)
            .collect();
        let inverse = Expression::Not(
            crate::ast::Metadata::new(),
            crate::ast::Moo::new(bits[0].clone()),
        );
        let index = SatIntegerView {
            constant: 0,
            terms: vec![(-2, inverse.clone()), (3, bits[0].clone())],
            groups: vec![PbTermGroup {
                start: 0,
                end: 2,
                structure: PbTermStructure::Choice,
            }],
            choices: Some(vec![(-2, inverse), (3, bits[0].clone())]),
        };
        let value = SatIntegerView {
            constant: -1,
            terms: vec![(2, bits[1].clone()), (3, bits[2].clone())],
            groups: vec![PbTermGroup {
                start: 0,
                end: 2,
                structure: PbTermStructure::Chain,
            }],
            choices: None,
        };
        let entry = SatIntegerView {
            constant: -1,
            terms: vec![(2, bits[3].clone()), (3, bits[4].clone())],
            groups: vec![PbTermGroup {
                start: 0,
                end: 2,
                structure: PbTermStructure::BoundedBinary { lower: 0, upper: 5 },
            }],
            choices: None,
        };
        for algorithm in ElementEncoding::ALL {
            for pb in PbEncoding::ALL {
                for entries in [
                    vec![(-2, entry.clone()), (3, value.clone())],
                    vec![(-2, constant(4))],
                    vec![],
                ] {
                    let mut instance = SatInstance::new();
                    let mut map = HashMap::new();
                    for variable in &variables {
                        map.insert(variable.name().clone(), instance.new_lit());
                    }
                    let decision = SatEncodingDecision::Element {
                        index_view: index.clone(),
                        value: value.clone(),
                        entries: entries.clone(),
                        encoding: Some(EncodingSelection {
                            algorithm,
                            provenance: SelectionProvenance::ExplicitConfiguration,
                        }),
                        pb_encoding: Some(EncodingSelection {
                            algorithm: pb,
                            provenance: SelectionProvenance::ExplicitConfiguration,
                        }),
                    };
                    assert_eq!(
                        serde_json::from_str::<SatEncodingDecision>(
                            &serde_json::to_string(&decision).unwrap()
                        )
                        .unwrap(),
                        decision
                    );
                    let invariants = SatEncodingDecision::Assert(Expression::Imply(
                        crate::ast::Metadata::new(),
                        crate::ast::Moo::new(bits[2].clone()),
                        crate::ast::Moo::new(bits[1].clone()),
                    ));
                    compile_decisions(&[decision, invariants], &mut instance, &mut map).unwrap();
                    let used = instance.var_manager_mut().n_used();
                    assert_eq!(instance.new_lit().var().idx32(), used);
                    let (cnf, _): (Cnf, BasicVarManager) = instance.into_cnf();
                    let mut solver = SatSolver::default();
                    solver.add_cnf(cnf).unwrap();
                    for assignment in 0usize..32 {
                        let set = |i| assignment & (1usize << i) != 0;
                        let index_number = if set(0) { 3 } else { -2 };
                        let numeric = |view: &SatIntegerView| {
                            view.constant
                                + view
                                    .terms
                                    .iter()
                                    .map(|(weight, expression)| {
                                        let position =
                                            bits.iter().position(|bit| bit == expression).unwrap();
                                        weight * i64::from(set(position))
                                    })
                                    .sum::<i64>()
                        };
                        let valid_chain = !set(2) || set(1);
                        let expected = valid_chain
                            && entries
                                .iter()
                                .filter(|(label, _)| *label == index_number)
                                .all(|(_, entry)| numeric(&value) == numeric(entry));
                        let assumptions: Vec<_> = variables
                            .iter()
                            .enumerate()
                            .map(|(i, variable)| {
                                let lit = map[&variable.name()];
                                if set(i) { lit } else { !lit }
                            })
                            .collect();
                        assert_eq!(
                            solver.solve_assumps(&assumptions).unwrap() == SolverResult::Sat,
                            expected,
                            "{algorithm:?}/{pb:?}, assignment {assignment}, entries {entries:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn element_rejects_duplicate_labels_and_unresolved_algorithms() {
        let mut decision = SatEncodingDecision::Element {
            index_view: constant(i64::MIN),
            value: constant(i64::MAX),
            entries: vec![(i64::MIN, constant(i64::MAX))],
            encoding: Some(EncodingSelection {
                algorithm: ElementEncoding::Support,
                provenance: SelectionProvenance::Heuristic,
            }),
            pb_encoding: Some(EncodingSelection {
                algorithm: PbEncoding::RustsatBinaryAdder,
                provenance: SelectionProvenance::Heuristic,
            }),
        };
        let compile =
            |decision| compile_decisions(&[decision], &mut SatInstance::new(), &mut HashMap::new());
        assert!(compile(decision.clone()).is_ok());
        if let SatEncodingDecision::Element { entries, .. } = &mut decision {
            entries.push(entries[0].clone());
        }
        assert!(compile(decision.clone()).is_err());
        if let SatEncodingDecision::Element {
            entries, encoding, ..
        } = &mut decision
        {
            entries.pop();
            *encoding = None;
        }
        assert!(compile(decision.clone()).is_err());
        if let SatEncodingDecision::Element {
            encoding,
            pb_encoding,
            ..
        } = &mut decision
        {
            *encoding = Some(EncodingSelection {
                algorithm: ElementEncoding::Implication,
                provenance: SelectionProvenance::Heuristic,
            });
            *pb_encoding = None;
        }
        assert!(compile(decision).is_err());
    }
}
