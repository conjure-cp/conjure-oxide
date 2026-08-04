use conjure_cp::ast::{AbstractLiteral, GroundDomain, Literal, records::Field};
use itertools::Itertools;

/// Enumerate finite product values in Conjure's recursive symmetry order.
pub(crate) fn symmetry_values(domain: &GroundDomain) -> Option<Vec<Literal>> {
    match domain {
        GroundDomain::Bool => Some(vec![Literal::Bool(true), Literal::Bool(false)]),
        GroundDomain::Int(_) => domain.values().ok().map(Iterator::collect),
        GroundDomain::Tuple(fields) => {
            let fields = fields
                .iter()
                .map(|field| symmetry_values(field))
                .collect::<Option<Vec<_>>>()?;
            Some(
                fields
                    .into_iter()
                    .multi_cartesian_product()
                    .map(|fields| Literal::AbstractLiteral(AbstractLiteral::Tuple(fields)))
                    .collect(),
            )
        }
        GroundDomain::Record(fields) => {
            let mut fields = fields.clone();
            fields.sort_by(|lhs, rhs| lhs.name.cmp(&rhs.name));
            let names = fields
                .iter()
                .map(|field| field.name.clone())
                .collect::<Vec<_>>();
            let values = fields
                .iter()
                .map(|field| symmetry_values(&field.value))
                .collect::<Option<Vec<_>>>()?;
            Some(
                values
                    .into_iter()
                    .multi_cartesian_product()
                    .map(|values| {
                        let fields = names
                            .iter()
                            .cloned()
                            .zip(values)
                            .map(|(name, value)| Field { name, value })
                            .collect();
                        Literal::AbstractLiteral(AbstractLiteral::Record(fields))
                    })
                    .collect(),
            )
        }
        GroundDomain::Variant(fields) => {
            let alternatives = fields
                .iter()
                .map(|field| {
                    Some(
                        symmetry_values(&field.value)?
                            .into_iter()
                            .map(|value| {
                                Literal::AbstractLiteral(AbstractLiteral::Variant(
                                    conjure_cp::ast::Moo::new(Field {
                                        name: field.name.clone(),
                                        value,
                                    }),
                                ))
                            })
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<Option<Vec<_>>>()?;
            Some(alternatives.into_iter().flatten().collect())
        }
        _ => None,
    }
}

/// Put product literals into the same canonical field order used by packed layouts.
pub(crate) fn canonical_product_literal(literal: Literal) -> Literal {
    match literal {
        Literal::AbstractLiteral(AbstractLiteral::Tuple(fields)) => Literal::AbstractLiteral(
            AbstractLiteral::Tuple(fields.into_iter().map(canonical_product_literal).collect()),
        ),
        Literal::AbstractLiteral(AbstractLiteral::Record(mut fields)) => {
            fields.sort_by(|lhs, rhs| lhs.name.cmp(&rhs.name));
            for field in &mut fields {
                field.value = canonical_product_literal(field.value.clone());
            }
            Literal::AbstractLiteral(AbstractLiteral::Record(fields))
        }
        Literal::AbstractLiteral(AbstractLiteral::Variant(field)) => {
            let mut field = conjure_cp::ast::Moo::unwrap_or_clone(field);
            field.value = canonical_product_literal(field.value);
            Literal::AbstractLiteral(AbstractLiteral::Variant(conjure_cp::ast::Moo::new(field)))
        }
        literal => literal,
    }
}
