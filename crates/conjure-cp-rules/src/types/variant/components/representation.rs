use crate::shared::representation_prelude::*;
use conjure_cp::ast::Moo;
use conjure_cp::ast::{Reference, records::Field};
use conjure_cp::utils::BiMap;
use conjure_cp::{domain_int, essence_expr, range};

register_representation!(
    VariantComponents("components")
    struct State<T> {
        /// One-based active-alternative tag.
        pub tag: T,
        /// Alternative names in declaration order.
        pub indices: BiMap<Name, usize>,
        /// One recursively represented declaration per alternative.
        pub fields: Vec<T>,
        /// Canonical values used by inactive alternatives.
        pub zero_values: Vec<Literal>
    }
    impl State<DeclarationPtr> {
        pub fn tag_ref(&self) -> Reference {
            Reference::new(self.tag.clone())
        }

        pub fn tag_expr(&self) -> Expression {
            self.tag_ref().into()
        }

        pub fn field_ref(&self, name: &Name) -> Option<Reference> {
            let index = self.indices.get_by_left(name)?;
            self.fields.get(*index).cloned().map(Reference::new)
        }

        pub fn field_refs(&self) -> impl Iterator<Item = Reference> + '_ {
            self.fields.iter().cloned().map(Reference::new)
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let Some(entries) = dom.as_variant() else {
            return Err(ReprInitError::UnsupportedDomain(
                dom,
                VariantComponents::NAME,
                "expected a variant domain".to_owned(),
            ));
        };
        if entries.is_empty() {
            return Err(ReprInitError::UnsupportedDomain(
                dom,
                VariantComponents::NAME,
                "variant domains must contain an alternative".to_owned(),
            ));
        }

        let mut indices = BiMap::with_capacity(entries.len());
        let mut fields = Vec::with_capacity(entries.len());
        let mut zero_values = Vec::with_capacity(entries.len());
        for (index, Field { name, value }) in entries.into_iter().enumerate() {
            let zero = value
                .resolve()
                .ok()
                .and_then(|domain| domain.values().ok()?.next())
                .ok_or_else(|| ReprInitError::UnsupportedDomain(
                    dom.clone(),
                    VariantComponents::NAME,
                    format!("alternative {name} has no finite canonical value"),
                ))?;
            indices.insert(name, index);
            fields.push(value);
            zero_values.push(zero);
        }
        let tag = domain_int!(1..i32::try_from(fields.len()).map_err(|_| {
            ReprInitError::UnsupportedDomain(
                dom.clone(),
                VariantComponents::NAME,
                "too many variant alternatives".to_owned(),
            )
        })?);
        Ok(State { tag, indices, fields, zero_values })
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        let tag = state.tag_expr();
        state
            .field_refs()
            .zip(&state.zero_values)
            .enumerate()
            .map(|(index, (field, zero))| {
                let active_tag = i32::try_from(index + 1).unwrap();
                essence_expr!((&tag != &active_tag) -> (&field = &zero))
            })
            .collect()
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(AbstractLiteral::Variant(entry)) = &value else {
            return Err(ReprDownError::BadValue(value, "expected a variant literal".to_owned()));
        };
        let Some(index) = state.indices.get_by_left(&entry.name).copied() else {
            return Err(ReprDownError::BadValue(
                value.clone(),
                format!("unexpected variant alternative {}", entry.name),
            ));
        };
        let mut fields = state.zero_values.clone();
        fields[index] = entry.value.clone();
        Ok(State {
            tag: Literal::Int(i32::try_from(index + 1).unwrap()),
            indices: state.indices.clone(),
            fields,
            zero_values: state.zero_values.clone(),
        })
    }
    fn up(state: State<Literal>) -> Literal {
        let Literal::Int(tag) = state.tag else {
            bug!("expected a variant tag integer, got {}", state.tag)
        };
        let index = usize::try_from(tag - 1)
            .ok()
            .filter(|index| *index < state.fields.len())
            .unwrap_or_else(|| bug!("variant tag {tag} is outside its representation"));
        let name = state.indices.get_by_right(&index).unwrap_or_else(|| {
            bug!("variant component index {index} has no alternative name")
        }).clone();
        let value = state.fields.into_iter().nth(index).unwrap();
        Literal::AbstractLiteral(AbstractLiteral::Variant(Moo::new(Field { name, value })))
    }
);
