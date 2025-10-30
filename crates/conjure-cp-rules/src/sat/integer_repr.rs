use conjure_cp::ast::Expression as Expr;
use conjure_cp::ast::SymbolTable;
use conjure_cp::rule_engine::{
    ApplicationError, ApplicationError::RuleNotApplicable, ApplicationResult, Reduction,
    register_rule,
};

use conjure_cp::ast::Metadata;
use conjure_cp::ast::{Atom, Domain, Literal, Moo, Range};
use conjure_cp::into_matrix_expr;

use conjure_cp::essence_expr;

// The number of bits used to represent the integer.
// This is a fixed value for the representation, but could be made dynamic if needed.
pub const BITS: usize = 8; // FIXME: Make it dynamic

/// This function takes a target expression and a vector of ranges and creates an expression representing the ranges with the target expression as the subject
///
/// E.g. x : int(4), int(10..20), int(30..) ~~> Or(x=4, 10<=x<=20, x>=30)
fn int_domain_to_expr(subject: Expr, ranges: &Vec<Range<i32>>) -> Expr {
    let mut output = vec![];

    let value = Moo::new(subject);

    for range in ranges {
        match range {
            Range::Single(x) => output.push(essence_expr!(&value = &x)),
            Range::Bounded(x, y) => output.push(essence_expr!("&value >= &x /\\ &value <= &y")),
            Range::UnboundedR(x) => output.push(essence_expr!(&value >= &x)),
            Range::UnboundedL(x) => output.push(essence_expr!(&value <= &x)),
        }
    }

    Expr::Or(Metadata::new(), Moo::new(into_matrix_expr!(output)))
}

/// This function confirms that all of the input expressions are SATInts, and returns vectors for each input of their bits
#[allow(dead_code)]
pub fn validate_sat_int_operands(exprs: Vec<Expr>) -> Result<Vec<Vec<Expr>>, ApplicationError> {
    let out: Result<Vec<Vec<_>>, _> = exprs
        .into_iter()
        .map(|expr| {
            let Expr::SATInt(_, inner) = expr else {
                return Err(RuleNotApplicable);
            };
            let Some(bits) = inner.as_ref().clone().unwrap_list() else {
                return Err(RuleNotApplicable);
            };
            Ok(bits)
        })
        .collect();

    out
}

/// Converts an integer decision variable to SATInt form, creating a new representation of boolean variables if
/// one does not yet exist
///
/// ```text
///  x
///  ~~>
///  SATInt([x#00, x#01, ...])
///
///  new variables:
///  find x#00: bool
///  find x#01: bool
///  ...
///
/// ```
#[register_rule(("SAT", 9500))]
fn integer_decision_representation(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    // thing we are representing must be a reference
    let Expr::Atomic(_, Atom::Reference(name)) = expr else {
        return Err(RuleNotApplicable);
    };

    // thing we are representing must be a variable
    // symbols
    //     .lookup(name)
    //     .ok_or(RuleNotApplicable)?
    //     .as_var()
    //     .ok_or(RuleNotApplicable)?;

    // thing we are representing must be an integer
    let Domain::Int(ranges) = name.domain().unwrap() else {
        return Err(RuleNotApplicable);
    };

    let mut symbols = symbols.clone();

    let new_name = &name.name().to_owned();

    let repr_exists = symbols
        .get_representation(new_name, &["sat_log_int"])
        .is_some();

    let representation = symbols
        .get_or_add_representation(new_name, &["sat_log_int"])
        .ok_or(RuleNotApplicable)?;

    let bits = representation[0]
        .clone()
        .expression_down(&symbols)?
        .into_values()
        .collect();

    let cnf_int = Expr::SATInt(Metadata::new(), Moo::new(into_matrix_expr!(bits)));

    if !repr_exists {
        // add domain ranges as constraints if this is the first time the representation is added
        Ok(Reduction::new(
            cnf_int.clone(),
            vec![int_domain_to_expr(cnf_int, &ranges)], // contains domain rules
            symbols,
        ))
    } else {
        Ok(Reduction::pure(cnf_int))
    }
}

/// Converts an integer literal to SATInt form
///
/// ```text
///  3
///  ~~>
///  SATInt([true,true,false,false,false,false,false,false;int(1..)])
///
/// ```
#[register_rule(("SAT", 9500))]
fn literal_cnf_int(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let mut value = {
        if let Expr::Atomic(_, Atom::Literal(Literal::Int(v))) = expr {
            *v
        } else {
            return Err(RuleNotApplicable);
        }
    };

    //TODO: add support for negatives

    let mut binary_encoding = vec![];

    for _ in 0..BITS {
        binary_encoding.push(Expr::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::Bool((value & 1) != 0)),
        ));
        value >>= 1;
    }

    Ok(Reduction::pure(Expr::SATInt(
        Metadata::new(),
        Moo::new(into_matrix_expr!(binary_encoding)),
    )))
}
