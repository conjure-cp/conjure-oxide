use conjure_cp::ast::{Atom, Expression as Expr, GroundDomain, Literal, Moo, Name, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
};
use itertools::Itertools;

#[register_rule(("Base", 8001))]
fn indexed_flatten_matrix(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::SafeIndex(_, subject, index) | Expr::UnsafeIndex(_, subject, index) => {
            if let Expr::Flatten(_, n, matrix) = subject.as_ref() {
                if n.is_some() {
                    // TODO handle flatten with n dimension option
                    return Err(RuleNotApplicable);
                }

                if index.len() > 1 {
                    return Err(RuleNotApplicable);
                }

                // get the actual number of the index
                let Expr::Atomic(_, Atom::Literal(Literal::Int(index))) = index[0] else {
                    return Err(RuleNotApplicable);
                };

                let Expr::Atomic(_, Atom::Reference(decl)) = matrix.as_ref() else {
                    return Err(RuleNotApplicable);
                };

                let Name::WithRepresentation(name, reprs) = &decl.name() as &Name else {
                    return Err(RuleNotApplicable);
                };

                if reprs.first().is_none_or(|x| x.as_str() != "matrix_to_atom") {
                    return Err(RuleNotApplicable);
                }

                let decl = symbols.lookup(name.as_ref()).unwrap();
                let repr = symbols
                    .get_representation(name.as_ref(), &["matrix_to_atom"])
                    .unwrap()[0]
                    .clone();

                // resolve index domains so that we can enumerate them later
                let dom = decl.resolved_domain().ok_or(RuleNotApplicable)?;
                let GroundDomain::Matrix(_, index_domains) = dom.as_ref() else {
                    return Err(RuleNotApplicable);
                };

                let Ok(matrix_values) = repr.expression_down(symbols) else {
                    return Err(RuleNotApplicable);
                };

                let flat_index = ndim_to_flat_index(index_domains.clone(), index as usize - 1);
                println!("{}", flat_index.iter().join("_"));

                let flat_value = matrix_values[&Name::Represented(Box::new((
                    name.as_ref().clone(),
                    "matrix_to_atom".into(),
                    flat_index.iter().join("_").into(),
                )))]
                    .clone();

                return Ok(Reduction::pure(flat_value));
            }

            Err(RuleNotApplicable)
        }
        _ => Err(RuleNotApplicable),
    }
}

// Given index domains for a multi-dimensional matrix and the nth index in the flattened matrix, find the coordinates in the original matrix
fn ndim_to_flat_index(index_domains: Vec<Moo<GroundDomain>>, index: usize) -> Vec<usize> {
    let mut remaining = index;
    let mut multipliers = vec![1; index_domains.len()];

    for i in (0..index_domains.len() - 1).rev() {
        multipliers[i] = multipliers[i + 1] * index_domains[i + 1].as_ref().length().unwrap();
    }

    let mut coords = vec![0; index_domains.len()];
    for i in 0..index_domains.len() {
        coords[i] = remaining / multipliers[i] as usize;
        remaining %= multipliers[i] as usize;
    }

    // adjust for 1-based indexing
    for coord in coords.iter_mut() {
        *coord += 1;
    }
    coords
}
