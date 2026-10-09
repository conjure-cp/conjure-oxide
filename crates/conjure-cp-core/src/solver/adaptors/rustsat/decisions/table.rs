//! Positive and negative table constraints, including MDD-based tables.
use super::*;

impl Compiler<'_> {
    pub(super) fn general_table(
        &mut self,
        output: Term,
        inputs: &[crate::ast::sat_decision::SatIntegerView],
        rows: &[Vec<crate::ast::sat_decision::SatIntegerView>],
        negative: bool,
        encoding: &Option<
            crate::ast::sat_decision::EncodingSelection<crate::ast::sat_decision::TableEncoding>,
        >,
        pb_encoding: &Option<
            crate::ast::sat_decision::EncodingSelection<crate::ast::sat_decision::PbEncoding>,
        >,
    ) -> Result<(), SolverError> {
        use crate::ast::sat_decision::TableEncoding;
        let algorithm = encoding
            .as_ref()
            .ok_or_else(|| SolverError::ModelInvalid("Unresolved table encoding decision".into()))?
            .algorithm;
        if algorithm == TableEncoding::BinarySupport {
            return Err(SolverError::ModelInvalid(
                "Binary-support tables require constant rows and two columns".into(),
            ));
        }
        let pb = pb_encoding
            .as_ref()
            .ok_or_else(|| {
                SolverError::ModelInvalid("Unresolved table component PB encoding".into())
            })?
            .algorithm;
        let mut cells = vec![std::collections::BTreeMap::new(); inputs.len()];
        let mut ids = vec![HashMap::new(); inputs.len()];
        let mut relation = Vec::new();
        for row in rows {
            if row.len() != inputs.len() {
                return Err(SolverError::ModelInvalid(
                    "Table row width differs from tuple width".into(),
                ));
            }
            let mut encoded = Vec::new();
            for (column, (input, cell)) in inputs.iter().zip(row).enumerate() {
                let equality = self.view_equality(input, cell, pb)?;
                let next = i64::try_from(ids[column].len())
                    .map_err(|_| SolverError::ModelInvalid("Too many table cells".into()))?;
                let id = *ids[column].entry(equality).or_insert(next);
                cells[column].insert(id, equality);
                encoded.push(id);
            }
            relation.push(encoded);
        }
        relation.sort();
        relation.dedup();
        let truth = match algorithm {
            TableEncoding::Tuple => {
                let matches = relation
                    .iter()
                    .map(|row| {
                        self.combine(
                            true,
                            row.iter()
                                .enumerate()
                                .map(|(column, id)| cells[column][id])
                                .collect(),
                        )
                    })
                    .collect();
                self.combine(false, matches)
            }
            TableEncoding::Mdd => self.table_mdd(0, relation, &cells, &mut HashMap::new()),
            TableEncoding::BinarySupport => unreachable!(),
        };
        self.equate(output, if negative { truth.negated() } else { truth });
        Ok(())
    }

    pub(super) fn table(
        &mut self,
        output: Term,
        inputs: &[crate::ast::sat_decision::SatIntegerView],
        rows: &[Vec<i64>],
        negative: bool,
        encoding: &Option<
            crate::ast::sat_decision::EncodingSelection<crate::ast::sat_decision::TableEncoding>,
        >,
        pb_encoding: &Option<
            crate::ast::sat_decision::EncodingSelection<crate::ast::sat_decision::PbEncoding>,
        >,
    ) -> Result<(), SolverError> {
        use crate::ast::sat_decision::{IntegerRelation, TableEncoding};
        let algorithm = encoding
            .as_ref()
            .ok_or_else(|| SolverError::ModelInvalid("Unresolved table encoding decision".into()))?
            .algorithm;
        if algorithm == TableEncoding::BinarySupport && inputs.len() != 2 {
            return Err(SolverError::ModelInvalid(
                "Binary-support tables require two columns".into(),
            ));
        }
        if rows.iter().any(|row| row.len() != inputs.len()) {
            return Err(SolverError::ModelInvalid(
                "Table row width differs from tuple width".into(),
            ));
        }
        if rows.is_empty() || inputs.is_empty() {
            let truth = !rows.is_empty();
            self.equate(output, Term::Constant(truth != negative));
            return Ok(());
        }
        let pb = pb_encoding
            .as_ref()
            .ok_or_else(|| {
                SolverError::ModelInvalid("Unresolved table component PB encoding".into())
            })?
            .algorithm;
        let mut rows = rows.to_vec();
        rows.sort();
        rows.dedup();
        let mut cells = Vec::with_capacity(inputs.len());
        for (column, input) in inputs.iter().enumerate() {
            validate_pb_groups(&input.groups, input.terms.len())?;
            let terms = input
                .terms
                .iter()
                .map(|(weight, expression)| self.encode(expression).map(|term| (*weight, term)))
                .collect::<Result<Vec<_>, _>>()?;
            let values = rows
                .iter()
                .map(|row| row[column])
                .collect::<std::collections::BTreeSet<_>>();
            let mut equalities = std::collections::BTreeMap::new();
            for value in values {
                if terms.is_empty() {
                    equalities.insert(value, Term::Constant(input.constant == value));
                    continue;
                }
                let bound = i64::try_from(i128::from(value) - i128::from(input.constant)).map_err(
                    |_| {
                        SolverError::ModelInvalid(
                            "Table cell difference exceeds the library range".into(),
                        )
                    },
                )?;
                let truth = Term::Literal(self.instance.new_lit());
                if !self.choice_relation(
                    truth,
                    IntegerRelation::Equal,
                    bound,
                    &terms,
                    &input.groups,
                ) {
                    self.integer_relation(
                        pb,
                        truth,
                        IntegerRelation::Equal,
                        bound,
                        &terms,
                        &input.groups,
                    )?;
                }
                equalities.insert(value, truth);
            }
            cells.push(equalities);
        }
        let truth = match algorithm {
            TableEncoding::Tuple => {
                let matches = rows
                    .iter()
                    .map(|row| {
                        self.combine(
                            true,
                            row.iter()
                                .enumerate()
                                .map(|(column, value)| cells[column][value])
                                .collect(),
                        )
                    })
                    .collect();
                self.combine(false, matches)
            }
            TableEncoding::Mdd => self.table_mdd(0, rows, &cells, &mut HashMap::new()),
            TableEncoding::BinarySupport => {
                let mut conditions = Vec::new();
                for column in 0..2 {
                    conditions.push(self.combine(false, cells[column].values().copied().collect()));
                    for (value, equality) in &cells[column] {
                        let supports = rows
                            .iter()
                            .filter(|row| row[column] == *value)
                            .map(|row| cells[1 - column][&row[1 - column]])
                            .collect();
                        let support = self.combine(false, supports);
                        conditions.push(self.combine(false, vec![equality.negated(), support]));
                    }
                }
                self.combine(true, conditions)
            }
        };
        self.equate(output, if negative { truth.negated() } else { truth });
        Ok(())
    }

    /// Identical suffix relations share one layered node and its library gates.
    pub(super) fn table_mdd(
        &mut self,
        column: usize,
        rows: Vec<Vec<i64>>,
        cells: &[std::collections::BTreeMap<i64, Term>],
        memo: &mut TableNodeCache,
    ) -> Term {
        if column == cells.len() {
            return Term::Constant(!rows.is_empty());
        }
        let key = (column, rows.clone());
        if let Some(truth) = memo.get(&key) {
            return *truth;
        }
        let mut branches = std::collections::BTreeMap::<i64, Vec<Vec<i64>>>::new();
        for row in rows {
            branches.entry(row[0]).or_default().push(row[1..].to_vec());
        }
        let mut truths = Vec::new();
        for (value, mut suffixes) in branches {
            suffixes.sort();
            suffixes.dedup();
            let suffix = self.table_mdd(column + 1, suffixes, cells, memo);
            truths.push(self.combine(true, vec![cells[column][&value], suffix]));
        }
        let truth = self.combine(false, truths);
        memo.insert(key, truth);
        truth
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::sat_decision::{
        EncodingSelection, PbEncoding, PbTermGroup, PbTermStructure, SelectionProvenance,
    };
    use crate::ast::{DeclarationPtr, Domain, Metadata, Moo, Reference};
    use crate::solver::adaptors::rustsat::adaptor::SatSolver;
    use rustsat::instances::{BasicVarManager, ManageVars};
    use rustsat::solvers::{Solve, SolveIncremental, SolverResult};

    #[test]
    fn variable_table_rows_and_wildcards_preserve_full_reification() {
        use crate::ast::sat_decision::{SatIntegerView, TableEncoding};
        let variables = (0..4)
            .map(|index| {
                DeclarationPtr::new_find(
                    Name::User(format!("variable_table_{index}").into()),
                    Domain::bool(),
                )
            })
            .collect::<Vec<_>>();
        let bits = variables
            .iter()
            .cloned()
            .map(Reference::new)
            .map(Expression::from)
            .collect::<Vec<_>>();
        let view = |index: usize| SatIntegerView {
            constant: 0,
            terms: vec![(1, bits[index].clone())],
            groups: vec![],
            choices: None,
        };
        let constant = |value| SatIntegerView {
            constant: value,
            terms: vec![],
            groups: vec![],
            choices: None,
        };
        for algorithm in [TableEncoding::Tuple, TableEncoding::Mdd] {
            for pb in PbEncoding::ALL {
                for negative in [false, true] {
                    for wildcard in [false, true] {
                        let rows = if wildcard {
                            vec![vec![view(0), constant(1)], vec![constant(1), view(1)]]
                        } else {
                            vec![vec![view(2), view(0)], vec![constant(0), constant(1)]]
                        };
                        let decision = SatEncodingDecision::Table {
                            output: bits[3].clone(),
                            inputs: vec![view(0), view(1)],
                            rows: vec![],
                            row_views: Some(rows),
                            negative,
                            encoding: Some(EncodingSelection {
                                algorithm,
                                provenance: SelectionProvenance::ExplicitConfiguration,
                            }),
                            pb_encoding: Some(EncodingSelection {
                                algorithm: pb,
                                provenance: SelectionProvenance::ExplicitConfiguration,
                            }),
                        };
                        let mut instance = SatInstance::new();
                        let mut map = HashMap::new();
                        for variable in &variables {
                            map.insert(variable.name().clone(), instance.new_lit());
                        }
                        compile_decisions(&[decision], &mut instance, &mut map).unwrap();
                        let mut solver = SatSolver::default();
                        solver.add_cnf(instance.cnf().clone()).unwrap();
                        for assignment in 0..16 {
                            let set = |index| assignment & (1 << index) != 0;
                            let truth = if wildcard {
                                set(0) || set(1)
                            } else {
                                (set(0) == set(2) && set(1) == set(0)) || (!set(0) && set(1))
                            };
                            let assumptions = variables
                                .iter()
                                .enumerate()
                                .map(|(index, variable)| {
                                    if set(index) {
                                        map[&variable.name()]
                                    } else {
                                        !map[&variable.name()]
                                    }
                                })
                                .collect::<Vec<_>>();
                            assert_eq!(
                                solver.solve_assumps(&assumptions).unwrap() == SolverResult::Sat,
                                set(3) == (truth != negative),
                                "{algorithm} {pb} wildcard={wildcard} negative={negative} assignment={assignment}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn table_mdd_shares_suffixes_and_rejects_malformed_rows() {
        use crate::ast::sat_decision::{SatIntegerView, TableEncoding};
        let variables: Vec<_> = (0..4)
            .map(|i| {
                DeclarationPtr::new_find(Name::User(format!("mdd_{i}").into()), Domain::bool())
            })
            .collect();
        let bits: Vec<Expression> = variables
            .iter()
            .cloned()
            .map(Reference::new)
            .map(Into::into)
            .collect();
        let inputs: Vec<_> = bits[..3]
            .iter()
            .map(|bit| SatIntegerView {
                constant: 0,
                terms: vec![(1, bit.clone())],
                groups: vec![],
                choices: None,
            })
            .collect();
        let build = |strategy, rows| SatEncodingDecision::Table {
            output: bits[3].clone(),
            inputs: inputs.clone(),
            rows,
            row_views: None,
            negative: false,
            encoding: Some(EncodingSelection {
                algorithm: strategy,
                provenance: SelectionProvenance::ExplicitConfiguration,
            }),
            pb_encoding: Some(EncodingSelection {
                algorithm: PbEncoding::RustsatGeneralizedTotalizer,
                provenance: SelectionProvenance::ExplicitConfiguration,
            }),
        };
        let rows: Vec<_> = (0..8)
            .map(|n| (0..3).map(|i| (n >> i) & 1).collect())
            .collect();
        let mut counts = Vec::new();
        for strategy in [TableEncoding::Tuple, TableEncoding::Mdd] {
            let mut instance = SatInstance::new();
            let mut map = HashMap::new();
            for v in &variables {
                map.insert(v.name().clone(), instance.new_lit());
            }
            compile_decisions(&[build(strategy, rows.clone())], &mut instance, &mut map).unwrap();
            counts.push(instance.var_manager_mut().n_used());
        }
        assert!(
            counts[1] < counts[0],
            "Shared suffixes should save auxiliaries: {counts:?}"
        );
        let mut instance = SatInstance::new();
        let mut map = HashMap::new();
        for v in &variables {
            map.insert(v.name().clone(), instance.new_lit());
        }
        assert!(
            compile_decisions(
                &[build(TableEncoding::Mdd, vec![vec![0]])],
                &mut instance,
                &mut map
            )
            .is_err()
        );
    }

    #[test]
    fn dense_binary_tables_preserve_all_assignments_and_measure_provider_costs() {
        use crate::ast::sat_decision::{SatIntegerView, TableEncoding};
        let variables = (0..7)
            .map(|i| DeclarationPtr::new_find(Name::user(&format!("dense_{i}")), Domain::bool()))
            .collect::<Vec<_>>();
        let bits = variables
            .iter()
            .cloned()
            .map(Reference::new)
            .map(Expression::from)
            .collect::<Vec<_>>();
        let views = (0..2)
            .map(|column| SatIntegerView {
                constant: 0,
                terms: (0..3)
                    .map(|bit| (1 << bit, bits[column * 3 + bit].clone()))
                    .collect(),
                groups: vec![],
                choices: None,
            })
            .collect::<Vec<_>>();
        let rows = (0..8)
            .flat_map(|x| {
                (0..8)
                    .filter(move |y| (x + y) % 4 != 0)
                    .map(move |y| vec![x, y])
            })
            .collect::<Vec<_>>();
        for algorithm in TableEncoding::ALL {
            let decision = SatEncodingDecision::Table {
                output: bits[6].clone(),
                inputs: views.clone(),
                rows: rows.clone(),
                row_views: None,
                negative: false,
                encoding: Some(EncodingSelection {
                    algorithm,
                    provenance: SelectionProvenance::ExplicitConfiguration,
                }),
                pb_encoding: Some(EncodingSelection {
                    algorithm: PbEncoding::RustsatGeneralizedTotalizer,
                    provenance: SelectionProvenance::ExplicitConfiguration,
                }),
            };
            let mut instance = SatInstance::new();
            let mut map = HashMap::new();
            for variable in &variables {
                map.insert(variable.name().clone(), instance.new_lit());
            }
            compile_decisions(&[decision], &mut instance, &mut map).unwrap();
            let vars = instance.var_manager_mut().n_used();
            let (cnf, _): (Cnf, BasicVarManager) = instance.into_cnf();
            eprintln!(
                "dense binary table {algorithm:?}: {vars} variables, {} clauses",
                cnf.len()
            );
            let mut solver = SatSolver::default();
            solver.add_cnf(cnf).unwrap();
            for assignment in 0..128usize {
                let assumptions = variables
                    .iter()
                    .enumerate()
                    .map(|(i, variable)| {
                        let literal = map[&*variable.name()];
                        if assignment & (1 << i) != 0 {
                            literal
                        } else {
                            !literal
                        }
                    })
                    .collect::<Vec<_>>();
                let valid = (((assignment & 7) + ((assignment >> 3) & 7)) % 4 != 0)
                    == (assignment & 64 != 0);
                assert_eq!(
                    solver.solve_assumps(&assumptions).unwrap() == SolverResult::Sat,
                    valid,
                    "{algorithm:?}, assignment {assignment}"
                );
            }
        }
    }

    #[test]
    fn tables_preserve_numeric_views_and_both_truth_values() {
        use crate::ast::sat_decision::{SatIntegerView, TableEncoding};
        let variables: Vec<_> = (0..5)
            .map(|i| {
                DeclarationPtr::new_find(Name::User(format!("table_{i}").into()), Domain::bool())
            })
            .collect();
        let bits: Vec<Expression> = variables
            .iter()
            .cloned()
            .map(Reference::new)
            .map(Into::into)
            .collect();
        let not_a = Expression::Not(Metadata::new(), Moo::new(bits[0].clone()));
        let choice = SatIntegerView {
            constant: 0,
            terms: vec![(-2, not_a.clone()), (3, bits[0].clone())],
            groups: vec![PbTermGroup {
                start: 0,
                end: 2,
                structure: PbTermStructure::Choice,
            }],
            choices: Some(vec![(-2, not_a), (3, bits[0].clone())]),
        };
        let views = vec![
            choice.clone(),
            SatIntegerView {
                constant: -1,
                terms: vec![(2, bits[1].clone()), (3, bits[2].clone())],
                groups: vec![PbTermGroup {
                    start: 0,
                    end: 2,
                    structure: PbTermStructure::Chain,
                }],
                choices: None,
            },
            SatIntegerView {
                constant: 1,
                terms: vec![(2, bits[3].clone())],
                groups: vec![PbTermGroup {
                    start: 0,
                    end: 1,
                    structure: PbTermStructure::BoundedBinary { lower: 0, upper: 2 },
                }],
                choices: None,
            },
            choice,
            SatIntegerView {
                constant: 0,
                terms: vec![],
                groups: vec![],
                choices: Some(vec![(0, true.into())]),
            },
        ];
        let relation = vec![
            vec![-2, -1, 1, -2, 0],
            vec![3, 4, 3, 3, 0],
            vec![3, 4, 3, 3, 0],
            vec![42, 0, 1, 42, 0],
        ];
        for strategy in TableEncoding::ALL {
            for pb in PbEncoding::ALL {
                for negative in [false, true] {
                    for assertion in [None, Some(false), Some(true)] {
                        for (inputs, rows) in [
                            (views.clone(), relation.clone()),
                            (views[..2].to_vec(), vec![vec![-2, -1], vec![3, 4]]),
                            (views[..2].to_vec(), vec![]),
                            (views.clone(), vec![]),
                            (vec![], vec![]),
                            (vec![], vec![vec![]]),
                        ] {
                            if strategy == TableEncoding::BinarySupport && inputs.len() != 2 {
                                continue;
                            }
                            let mut instance = SatInstance::new();
                            let mut map = HashMap::new();
                            for variable in &variables {
                                map.insert(variable.name().clone(), instance.new_lit());
                            }
                            let mut decisions = vec![
                                SatEncodingDecision::Assert(Expression::Imply(
                                    Metadata::new(),
                                    Moo::new(bits[2].clone()),
                                    Moo::new(bits[1].clone()),
                                )),
                                SatEncodingDecision::Table {
                                    output: bits[4].clone(),
                                    inputs: inputs.clone(),
                                    rows: rows.clone(),
                                    row_views: None,
                                    negative,
                                    encoding: Some(EncodingSelection {
                                        algorithm: strategy,
                                        provenance: SelectionProvenance::ExplicitConfiguration,
                                    }),
                                    pb_encoding: Some(EncodingSelection {
                                        algorithm: pb,
                                        provenance: SelectionProvenance::ExplicitConfiguration,
                                    }),
                                },
                            ];
                            if let Some(value) = assertion {
                                decisions.push(SatEncodingDecision::Assert(if value {
                                    bits[4].clone()
                                } else {
                                    Expression::Not(Metadata::new(), Moo::new(bits[4].clone()))
                                }));
                            }
                            assert_eq!(
                                serde_json::from_str::<SatEncodingDecision>(
                                    &serde_json::to_string(&decisions[1]).unwrap()
                                )
                                .unwrap(),
                                decisions[1]
                            );
                            compile_decisions(&decisions, &mut instance, &mut map).unwrap();
                            let used = instance.var_manager_mut().n_used();
                            assert_eq!(instance.new_lit().var().idx32(), used);
                            let (cnf, _): (Cnf, BasicVarManager) = instance.into_cnf();
                            let mut solver = SatSolver::default();
                            solver.add_cnf(cnf).unwrap();
                            for assignment in 0usize..32 {
                                let set = |i| assignment & (1usize << i) != 0;
                                let mut numeric = if inputs.is_empty() {
                                    vec![]
                                } else {
                                    vec![
                                        if set(0) { 3 } else { -2 },
                                        -1 + 2 * i64::from(set(1)) + 3 * i64::from(set(2)),
                                        1 + 2 * i64::from(set(3)),
                                        if set(0) { 3 } else { -2 },
                                        0,
                                    ]
                                };
                                numeric.truncate(inputs.len());
                                let truth = rows.contains(&numeric) != negative;
                                let valid = (!set(2) || set(1))
                                    && set(4) == truth
                                    && assertion.is_none_or(|value| value == set(4));
                                let assumptions: Vec<_> = variables
                                    .iter()
                                    .enumerate()
                                    .map(|(i, v)| {
                                        if set(i) {
                                            map[&v.name()]
                                        } else {
                                            !map[&v.name()]
                                        }
                                    })
                                    .collect();
                                assert_eq!(
                                    solver.solve_assumps(&assumptions).unwrap()
                                        == SolverResult::Sat,
                                    valid,
                                    "{strategy} {pb} negative={negative} asserted={assertion:?} bits={assignment} rows={rows:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
