use conjure_core::ast::Expression as Expr;
use conjure_core::ast::SymbolTable;
use conjure_core::rule_engine::{
    register_rule, ApplicationError::RuleNotApplicable, ApplicationResult, Reduction,
};

use conjure_core::ast::{Atom, Domain, Literal, Range};
use conjure_core::metadata::Metadata;
use conjure_core::{into_matrix_expr, matrix_expr};

#[register_rule(("CNF", 8000))]
fn integer_decision_representation(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    // thing we are representing must be a reference
    let Expr::Atomic(_, Atom::Reference(name)) = expr else {
        return Err(RuleNotApplicable);
    };

    // thing we are representing must be a variable
    symbols
        .lookup(name)
        .ok_or(RuleNotApplicable)?
        .as_var()
        .ok_or(RuleNotApplicable)?;

    // thing we are representing must be an integer
    let Domain::IntDomain(ranges) = &symbols.resolve_domain(name).unwrap() else {
        return Err(RuleNotApplicable);
    };

    let mut symbols = symbols.clone();

    let repr_exists = symbols.get_representation(name, &["int_to_atom"]).is_some();

    let representation = symbols
        .get_or_add_representation(name, &["int_to_atom"])
        .ok_or(RuleNotApplicable)?;

    let bits = representation[0]
        .clone()
        .expression_down(&symbols)?
        .into_iter()
        .map(|(_, expr)| expr.clone())
        .collect();

    let cnf_int = Expr::CnfInt(Metadata::new(), Box::new(into_matrix_expr!(bits)));

    if !repr_exists {
        // add domain ranges as constraints if this is the first time the representation is added
        Ok(Reduction::new(
            cnf_int.clone(),
            vec![int_domain_to_expr(cnf_int.clone(), ranges)], // contains domain rules
            symbols,
        ))
    } else {
        Ok(Reduction::with_symbols(cnf_int.clone(), symbols))
    }
}

#[register_rule(("CNF", 4000))]
fn literal_cnf_int(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Atomic(_, Atom::Literal(Literal::Int(mut value))) = expr else {
        return Err(RuleNotApplicable);
    };

    let mut binary_encoding = vec![];

    for _ in 0..32 {
        binary_encoding.push(Expr::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::Bool((value & 1) != 0)),
        ));
        value >>= 1;
    }

    Ok(Reduction::pure(Expr::CnfInt(
        Metadata::new(),
        Box::new(into_matrix_expr!(binary_encoding)),
    )))
}

// This function takes a target expression and a vector of ranges and creates an expression representing the ranges with the target expression as the subject
//
// E.g. x : int(4), int(10..20), int(30..) ~> Or(x=4, 10<=x<=20, x>=30)
fn int_domain_to_expr(subject: Expr, ranges: &Vec<Range<i32>>) -> Expr {
    let mut output = vec![];

    let value = Box::new(subject);

    for range in ranges {
        match range {
            Range::Single(x) => output.push(Expr::Eq(
                Metadata::new(),
                value.clone(),
                Box::new(Expr::Atomic(
                    Metadata::new(),
                    Atom::Literal(Literal::Int(*x)),
                )),
            )),
            Range::Bounded(x, y) => output.push(Expr::And(
                Metadata::new(),
                Box::new(matrix_expr![
                    Expr::Geq(
                        Metadata::new(),
                        value.clone(),
                        Box::new(Expr::Atomic(
                            Metadata::new(),
                            Atom::Literal(Literal::Int(*x))
                        )),
                    ),
                    Expr::Leq(
                        Metadata::new(),
                        value.clone(),
                        Box::new(Expr::Atomic(
                            Metadata::new(),
                            Atom::Literal(Literal::Int(*y))
                        )),
                    )
                ]),
            )),
            Range::UnboundedR(x) => output.push(Expr::Geq(
                Metadata::new(),
                value.clone(),
                Box::new(Expr::Atomic(
                    Metadata::new(),
                    Atom::Literal(Literal::Int(*x)),
                )),
            )),
            Range::UnboundedL(x) => output.push(Expr::Leq(
                Metadata::new(),
                value.clone(),
                Box::new(Expr::Atomic(
                    Metadata::new(),
                    Atom::Literal(Literal::Int(*x)),
                )),
            )),
        }
    }

    Expr::Or(Metadata::new(), Box::new(into_matrix_expr!(output)))
}

/// Converts a < expression between two CnfInts to a singular CnfInt
///
/// ```text
/// CnfInt(a) < CnfInt(b) ~> CnfInt(c)
///
/// ```

/* #[register_rule(("CNF", 4100))]
fn cnf_int_lt(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::Lt(_, x, y) => match (x, y) {
            (Expr::CnfInt(_), Expr::CnfInt(_)) => (),
            _ => return Err(RuleNotApplicable),
        },
        _ => return Err(RuleNotApplicable),
    }
    Ok(None) //TODO: Logic for < between cnf ints
} */

/// Converts a = expression between two CnfInts to a conjunction of boolean expressions
///
/// ```text
/// CnfInt(a) = CnfInt(b) ~> And(...)
///
/// ```
#[register_rule(("CNF", 4100))]
fn cnf_int_eq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Eq(_, x, y) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::CnfInt(_, x) = x.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let Expr::CnfInt(_, y) = y.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let Some(x_bits) = x.as_ref().clone().unwrap_list() else {
        return Err(RuleNotApplicable);
    };

    let Some(y_bits) = y.as_ref().clone().unwrap_list() else {
        return Err(RuleNotApplicable);
    };

    let output = x_bits
        .iter()
        .zip(y_bits.iter())
        .map(|(x_i, y_i)| {
            Expr::Iff(
                Metadata::new(),
                Box::new(x_i.clone()),
                Box::new(y_i.clone()),
            )
        })
        .collect();

    Ok(Reduction::pure(Expr::And(
        Metadata::new(),
        Box::new(into_matrix_expr!(output)),
    )))
}
