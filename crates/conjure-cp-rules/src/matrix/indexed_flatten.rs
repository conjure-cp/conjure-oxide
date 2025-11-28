use conjure_cp::ast::{
    Atom, Expression as Expr, GroundDomain, Literal, Metadata, Moo, Name, Range, SymbolTable,
    matrix,
};
use conjure_cp::into_matrix_expr;
use conjure_cp::rule_engine::Rule;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
};
use itertools::{Itertools, chain, izip};
use uniplate::Uniplate;

#[register_rule(("Base", 8001))]
fn indexed_flatten_matrix(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::SafeIndex(_, subject, index) | Expr::UnsafeIndex(_, subject, index) => {
            if let Expr::Flatten(_, n, matrix) = subject.as_ref() {
                if let Some(_) = n {
                    // TODO
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

                let flat_values = matrix::enumerate_indices(index_domains.clone())
                    .map(|i| {
                        matrix_values[&Name::Represented(Box::new((
                            name.as_ref().clone(),
                            "matrix_to_atom".into(),
                            i.iter().join("_").into(),
                        )))]
                            .clone()
                    })
                    .collect_vec();
                return Ok(Reduction::pure(flat_values[(index as usize) - 1].clone()));
            }

            Err(RuleNotApplicable)
        }
        _ => Err(RuleNotApplicable),
    }
}
