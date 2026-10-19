//! Helpers shared by the relation representations' generator-lowering rules.

use conjure_cp::ast::{DeclarationPtr, DomainPtr, Name, SymbolTablePtr};

/// Names the column variables an expression generator over a relation is lowered into.
///
/// The names are derived from the generator being lowered -- `i <- rel` gives `i0`, `i1`, ... --
/// and are then made unique within `symbols`. A comprehension can iterate the same relation more
/// than once (`i <- rel, j <- rel`, as the constraints for a function-as-relation do), and each of
/// those generators needs columns of its own: sharing them collapses the two loops onto a single
/// set of values, quietly turning a constraint relating the two into a tautology.
pub(super) fn fresh_column_declarations(
    generator: &DeclarationPtr,
    column_domains: impl IntoIterator<Item = DomainPtr>,
    symbols: &SymbolTablePtr,
) -> Vec<DeclarationPtr> {
    let base = generator.name().to_string();

    column_domains
        .into_iter()
        .enumerate()
        .map(|(index, domain)| {
            DeclarationPtr::new_quantified(fresh_column_name(&base, index, symbols), domain)
        })
        .collect()
}

/// `{base}{index}`, suffixed with a counter until it names nothing in `symbols`.
fn fresh_column_name(base: &str, index: usize, symbols: &SymbolTablePtr) -> Name {
    let preferred = format!("{base}{index}");
    let mut candidate = Name::user(&preferred);
    let mut disambiguator = 1usize;

    while symbols.read().lookup(&candidate).is_some() {
        candidate = Name::user(&format!("{preferred}_{disambiguator}"));
        disambiguator += 1;
    }

    candidate
}
