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
