use conjure_core::ast::{
    matrix, Atom, Domain, Expression as Expr, Literal, Name, Range, SymbolTable,
};
use conjure_core::into_matrix_expr;
use conjure_core::metadata::Metadata;
use conjure_core::rule_engine::{
    register_rule, ApplicationError::RuleNotApplicable, ApplicationResult, Reduction,
};
use conjure_essence_macros::essence_expr;
use itertools::{chain, izip, Itertools};
use uniplate::Uniplate;

/// Using the `matrix_to_atom`  representation rule, rewrite matrix indexing.
#[register_rule(("Base", 2000))]
fn index_matrix_to_atom(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    // is this an indexing operation?
    let Expr::SafeIndex(_, subject, indices) = expr else {
        return Err(RuleNotApplicable);
    };

    // ensure that we are indexing a decision variable with the representation "matrix_to_atom"
    // selected for it.
    let Expr::Atomic(_, Atom::Reference(Name::WithRepresentation(name, reprs))) = &**subject else {
        return Err(RuleNotApplicable);
    };

    if reprs.first().is_none_or(|x| x.as_str() != "matrix_to_atom") {
        return Err(RuleNotApplicable);
    }

    let repr = symbols
        .get_representation(name, &["matrix_to_atom"])
        .unwrap()[0]
        .clone();

    // ensure that the subject has a matrix domain.
    let decl = symbols.lookup(name).unwrap();

    // resolve index domains so that we can enumerate them later
    let Some(Domain::DomainMatrix(_, index_domains)) =
        decl.domain().cloned().map(|x| x.resolve(symbols))
    else {
        return Err(RuleNotApplicable);
    };

    // checks are all ok: do the actual rewrite!

    // 1. indices are constant -> find the element being indexed and only return that variable.
    // 2. indices are not constant -> flatten matrix and return [flattened_matrix][flattened_index_expr]

    // are the indices constant?
    let mut indices_are_const = true;
    let mut indices_as_lits: Vec<Literal> = vec![];

    for index in indices {
        let Some(index) = index.clone().to_literal() else {
            indices_are_const = false;
            break;
        };
        indices_as_lits.push(index);
    }

    if indices_are_const {
        // indices are constant -> find the element being indexed and only return that variable.
        //
        let indices_as_name = Name::RepresentedName(
            name.clone(),
            "matrix_to_atom".into(),
            indices_as_lits.iter().join("_"),
        );

        let subject = repr.expression_down(symbols)?[&indices_as_name].clone();

        Ok(Reduction::pure(subject))
    } else {
        // indices are not constant -> flatten matrix and return [flattened_matrix][flattened_index_expr]

        // For now, only supports matrices with index domains in the form int(n..m).
        //
        // Assuming this, to turn some x[a,b] and x[a,b,c] into x'[z]:
        //
        // z =                               + size(b) * (a-lb(a)) + 1 * (b-lb(b))  + 1 [2d matrix]
        // z = (size(b)*size(c))*(a−lb(a))   + size(c) * (b−lb(b)) + 1 * (c−lb(c))  + 1 [3d matrix]
        //
        // where lb(a) is the lower bound for a.
        //
        //
        // TODO: For other cases, we should generate table constraints that map the flat indices to
        // the real ones.

        // only need to do this for >1d matrices.
        let n_dims = index_domains.len();
        if n_dims <= 1 {
            return Err(RuleNotApplicable);
        };

        // some intermediate values we need to do the above..

        // [(lb(a),ub(a)),(lb(b),ub(b)),(lb(c),ub(c),...]
        let bounds = index_domains
            .iter()
            .map(|dom| {
                let Domain::IntDomain(ranges) = dom else {
                    return Err(RuleNotApplicable);
                };

                let &[Range::Bounded(from, to)] = &ranges[..] else {
                    return Err(RuleNotApplicable);
                };

                Ok((from, to))
            })
            .process_results(|it| it.collect_vec())?;

        // [size(a),size(b),size(c),..]
        let sizes = bounds
            .iter()
            .map(|(from, to)| (to - from) + 1)
            .collect_vec();

        // [lb(a),lb(b),lb(c),..]
        let lower_bounds = bounds.iter().map(|(from, _)| from).collect_vec();

        // from the examples above:
        //
        // index = (coefficients . terms) + 1
        //
        // where coefficients = [size(b)*size(c), size(c), 1      ]
        //       terms =        [a-lb(a)        , b-lb(b), c-lb(c)]

        // building coefficients.
        //
        // starting with sizes==[size(a),size(b),size(c)]
        //
        // ~~ skip(1) ~~>
        //
        // [size(b),size(c)]
        //
        // ~~ rev ~~>
        //
        // [size(c),size(b)]
        //
        // ~~ chain!(std::iter::once(&1),...) ~~>
        //
        // [1,size(c),size(b)]
        //
        // ~~ scan * ~~>
        //
        // [1,1*size(c),1*size(c)*size(b)]
        //
        // ~~ reverse ~~>
        //
        // [size(b)*size(c),size(c),1]
        let mut coeffs: Vec<Expr> = chain!(std::iter::once(&1), sizes.iter().skip(1).rev())
            .scan(1, |state, &x| {
                *state *= x;
                Some(*state)
            })
            .map(|x| essence_expr!(&x))
            .collect_vec();

        coeffs.reverse();

        // [(a-lb(a)),b-lb(b),c-lb(c)]
        let terms: Vec<Expr> = izip!(indices, lower_bounds)
            .map(|(i, lbi)| essence_expr!(&i - &lbi))
            .collect_vec();

        // coeffs . terms
        let mut sum_terms: Vec<Expr> = izip!(coeffs, terms)
            .map(|(coeff, term)| essence_expr!(&coeff * &term))
            .collect_vec();

        // (coeffs . terms) + 1
        sum_terms.push(essence_expr!(1));

        let flat_index = Expr::Sum(Metadata::new(), Box::new(into_matrix_expr![sum_terms]));

        // now lets get the flat matrix.

        let repr_exprs = repr.expression_down(symbols)?;
        let flat_elems = matrix::enumerate_indices(index_domains.clone())
            .map(|xs| {
                Name::RepresentedName(
                    name.clone(),
                    "matrix_to_atom".into(),
                    xs.into_iter().join("_"),
                )
            })
            .map(|x| repr_exprs[&x].clone())
            .collect_vec();

        let flat_matrix = into_matrix_expr![flat_elems];

        Ok(Reduction::pure(Expr::SafeIndex(
            Metadata::new(),
            Box::new(flat_matrix),
            vec![flat_index],
        )))
    }
}

/// Using the `matrix_to_atom` representation rule, rewrite matrix slicing.
#[register_rule(("Base", 2000))]
fn slice_matrix_to_atom(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::SafeSlice(_, subject, indices) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, Atom::Reference(Name::WithRepresentation(name, reprs))) = &**subject else {
        return Err(RuleNotApplicable);
    };

    if reprs.first().is_none_or(|x| x.as_str() != "matrix_to_atom") {
        return Err(RuleNotApplicable);
    }

    let decl = symbols.lookup(name).unwrap();
    let repr = symbols
        .get_representation(name, &["matrix_to_atom"])
        .unwrap()[0]
        .clone();

    // resolve index domains so that we can enumerate them later
    let Some(Domain::DomainMatrix(_, index_domains)) =
        decl.domain().cloned().map(|x| x.resolve(symbols))
    else {
        return Err(RuleNotApplicable);
    };

    let mut indices_as_lits: Vec<Option<Literal>> = vec![];
    let mut hole_dim: i32 = -1;
    for (i, index) in indices.iter().enumerate() {
        match index {
            Some(e) => {
                let lit = e.clone().to_literal().ok_or(RuleNotApplicable)?;
                indices_as_lits.push(Some(lit.clone()));
            }
            None => {
                indices_as_lits.push(None);
                assert_eq!(hole_dim, -1);
                hole_dim = i as _;
            }
        }
    }

    assert_ne!(hole_dim, -1);

    let repr_values = repr.expression_down(symbols)?;

    let slice = index_domains[hole_dim as usize]
        .values()
        .expect("index domain should be finite and enumerable")
        .into_iter()
        .map(|i| {
            let mut indices_as_lits = indices_as_lits.clone();
            indices_as_lits[hole_dim as usize] = Some(i);
            let name = Name::RepresentedName(
                name.clone(),
                "matrix_to_atom".into(),
                indices_as_lits.into_iter().map(|x| x.unwrap()).join("_"),
            );
            repr_values[&name].clone()
        })
        .collect_vec();

    let new_expr = into_matrix_expr!(slice);

    Ok(Reduction::pure(new_expr))
}

/// Converts a reference to a 1d-matrix not contained within an indexing or slicing expression to its atoms.
#[register_rule(("Base", 2000))]
fn matrix_ref_to_atom(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    if let Expr::SafeSlice(_, _, _)
    | Expr::UnsafeSlice(_, _, _)
    | Expr::SafeIndex(_, _, _)
    | Expr::UnsafeIndex(_, _, _) = expr
    {
        return Err(RuleNotApplicable);
    };

    for (child, ctx) in expr.holes() {
        let Expr::Atomic(_, Atom::Reference(Name::WithRepresentation(name, reprs))) = child else {
            continue;
        };

        if reprs.first().is_none_or(|x| x.as_str() != "matrix_to_atom") {
            continue;
        }

        let decl = symbols.lookup(name.as_ref()).unwrap();
        let repr = symbols
            .get_representation(name.as_ref(), &["matrix_to_atom"])
            .unwrap()[0]
            .clone();

        // resolve index domains so that we can enumerate them later
        let Some(Domain::DomainMatrix(_, index_domains)) =
            decl.domain().cloned().map(|x| x.resolve(symbols))
        else {
            continue;
        };

        if index_domains.len() > 1 {
            continue;
        }

        let Ok(matrix_values) = repr.expression_down(symbols) else {
            continue;
        };

        let flat_values = matrix::enumerate_indices(index_domains)
            .map(|i| {
                matrix_values[&Name::RepresentedName(
                    name.clone(),
                    "matrix_to_atom".into(),
                    i.iter().join("_"),
                )]
                    .clone()
            })
            .collect_vec();
        return Ok(Reduction::pure(ctx(into_matrix_expr![flat_values])));
    }

    Err(RuleNotApplicable)
}
