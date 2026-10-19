use crate::shared::representation_prelude::*;
use conjure_cp::ast::{Reference, records::Field};
use conjure_cp::utils::BiMap;

register_representation!(
    RecordComponents("components")
    struct State<T> {
        /// Canonical field-name order and its corresponding component index.
        pub indices: BiMap<Name, usize>,
        /// One recursively represented declaration per record field.
        pub fields: Vec<T>
    }
    impl State<DeclarationPtr> {
        pub fn field_ref(&self, name: &Name) -> Option<Reference> {
            let index = self.indices.get_by_left(name)?;
            self.fields.get(*index).cloned().map(Reference::new)
        }

        pub fn field_refs(&self) -> impl Iterator<Item = Reference> + '_ {
            self.fields.iter().cloned().map(Reference::new)
        }

        pub fn field_exprs(&self) -> Vec<Expression> {
            self.field_refs().map(Expression::from).collect()
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let Some(mut entries) = dom.as_record() else {
            return Err(ReprInitError::UnsupportedDomain(
                dom,
                RecordComponents::NAME,
                "expected a record domain".to_owned(),
            ));
        };
        entries.sort_by(|lhs, rhs| lhs.name.cmp(&rhs.name));

        let mut indices = BiMap::with_capacity(entries.len());
        let mut fields = Vec::with_capacity(entries.len());
        for (index, Field { name, value }) in entries.into_iter().enumerate() {
            indices.insert(name, index);
            fields.push(value);
        }
        Ok(State { indices, fields })
    }
    fn structural(_state: &State<DeclarationPtr>) -> Vec<Expression> {
        vec![]
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(AbstractLiteral::Record(entries)) = &value else {
            return Err(ReprDownError::BadValue(value, "expected a record literal".to_owned()));
        };
        if entries.len() != state.fields.len() {
            return Err(ReprDownError::BadValue(
                value.clone(),
                format!("expected {} fields, got {}", state.fields.len(), entries.len()),
            ));
        }

        let mut fields = vec![None; state.fields.len()];
        for Field { name, value: field } in entries {
            let Some(index) = state.indices.get_by_left(name) else {
                return Err(ReprDownError::BadValue(
                    value.clone(),
                    format!("unexpected record field {name}"),
                ));
            };
            fields[*index] = Some(field.clone());
        }
        let Some(fields) = fields.into_iter().collect::<Option<Vec<_>>>() else {
            return Err(ReprDownError::BadValue(value, "missing record field".to_owned()));
        };
        Ok(State {
            indices: state.indices.clone(),
            fields,
        })
    }
    fn up(state: State<Literal>) -> Literal {
        let entries = state.fields.into_iter().enumerate().map(|(index, value)| {
            let name = state.indices.get_by_right(&index).unwrap_or_else(|| {
                bug!("record component index {index} has no field name")
            }).clone();
            Field { name, value }
        }).collect();
        Literal::AbstractLiteral(AbstractLiteral::Record(entries))
    }
);
