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
