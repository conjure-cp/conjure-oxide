use conjure_cp::ast::{DeclarationPtr, Domain, records::RecordValue};
use itertools::Itertools;

use super::prelude::*;

register_representation!(RecordToAtom, "record_to_atom");

#[derive(Clone, Debug)]
pub struct RecordToAtom {
    src_var: Name,

    entry_names: Vec<Name>,

    // all the possible indices in this record, in order.
    indices: Vec<Literal>,

    // the element domains for each item in the tuple.
    elem_domain: Vec<Domain>,
}

impl RecordToAtom {
    /// Returns the names of the representation variable (there must be a much easier way to do this but oh well)
    fn names(&self) -> impl Iterator<Item = Name> + '_ {
        self.indices
            .iter()
            .map(move |index| self.indices_to_name(&[index.clone()]))
    }

    /// Gets the representation variable name for a specific set of indices.
    fn indices_to_name(&self, indices: &[Literal]) -> Name {
        Name::Represented(Box::new((
            self.src_var.clone(),
            self.repr_name().into(),
            indices.iter().join("_").into(),
        )))
    }
}

impl Representation for RecordToAtom {
    fn init(name: &Name, symtab: &SymbolTable) -> Option<Self> {
        let domain = symtab.resolve_domain(name)?;

        if !domain.is_finite().expect("should be finite?") {
            return None;
        }

        let Domain::Record(entries) = domain else {
            return None;
        };

        //indices may not be needed as a field as we can always use the length of the record
        let indices = (1..(entries.len() + 1) as i32).map(Literal::Int).collect();

        Some(RecordToAtom {
            src_var: name.clone(),
            entry_names: entries.iter().map(|entry| entry.name.clone()).collect(),
            indices,
            elem_domain: entries.iter().map(|entry| entry.domain.clone()).collect(),
        })
    }

    fn variable_name(&self) -> &Name {
        &self.src_var
    }

    fn value_down(
        &self,
        value: Literal,
    ) -> Result<std::collections::BTreeMap<Name, Literal>, ApplicationError> {
        let Literal::AbstractLiteral(record) = value else {
            return Err(ApplicationError::RuleNotApplicable);
        };

        let AbstractLiteral::Tuple(entries) = record else {
            return Err(ApplicationError::RuleNotApplicable);
        };

        let mut result = std::collections::BTreeMap::new();

        for (i, elem) in entries.into_iter().enumerate() {
            let name = format!("{}_{}", self.src_var, i + 1);
            result.insert(Name::user(&name), elem);
        }

        Ok(result)
    }

    fn value_up(
        &self,
        values: &std::collections::BTreeMap<Name, Literal>,
    ) -> Result<Literal, ApplicationError> {
        let mut record = Vec::new();

        for name in self.names() {
            let value = values
                .get(&name)
                .ok_or(ApplicationError::RuleNotApplicable)?;
            let Name::Represented(fields) = name.clone() else {
                return Err(ApplicationError::RuleNotApplicable);
            };

            let (_, _, idx) = *fields;

            let idx: usize = idx
                .parse()
                .map_err(|_| ApplicationError::RuleNotApplicable)?;
            if idx == 0 {
                return Err(ApplicationError::RuleNotApplicable);
            }
            let idx = idx - 1;
            record.push(RecordValue {
                name: self.entry_names[idx].clone(),
                value: value.clone(),
            });
        }

        Ok(Literal::AbstractLiteral(AbstractLiteral::Record(record)))
    }

    fn expression_down(
        &self,
        st: &SymbolTable,
    ) -> Result<std::collections::BTreeMap<Name, Expression>, ApplicationError> {
        Ok(self
            .names()
            .map(|name| {
                let decl = st.lookup(&name).unwrap();
                (
                    name,
                    Expression::Atomic(Metadata::new(), Atom::Reference(decl)),
                )
            })
            .collect())
    }

    fn declaration_down(&self) -> Result<Vec<DeclarationPtr>, ApplicationError> {
        Ok(self
            .names()
            .zip(self.elem_domain.iter().cloned())
            .map(|(name, domain)| DeclarationPtr::new_var(name, domain))
            .collect())
    }

    fn repr_name(&self) -> &str {
        "record_to_atom"
    }

    fn box_clone(&self) -> Box<dyn Representation> {
        Box::new(self.clone()) as _
    }
}
