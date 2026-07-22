use crate::backends::sat::integer::{defer_integer_representation, int_domain_to_expr};
use conjure_cp::ast::SymbolTable;
use conjure_cp::ast::{Atom, Expression as Expr, GroundDomain, Metadata, Moo, SATIntEncoding};
use conjure_cp::into_matrix_expr;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

#[register_rule("SAT_Order", 9500, [Atomic])]
fn integer_decision_representation_order(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    defer_integer_representation(expr, materialise_integer_decision_representation_order)
}

fn materialise_integer_decision_representation_order(
    expr: &Expr,
    symbols: &SymbolTable,
) -> ApplicationResult {
    let Expr::Atomic(_, Atom::Reference(name)) = expr else {
        return Err(RuleNotApplicable);
    };

    let dom = name.resolved_domain().ok_or(RuleNotApplicable)?;
    let GroundDomain::Int(ranges) = dom.as_ref() else {
        return Err(RuleNotApplicable);
    };
    let (min, max) = ranges
        .iter()
        .fold((i32::MAX, i32::MIN), |(min_a, max_b), range| {
            (
                min_a.min(*range.low().unwrap()),
                max_b.max(*range.high().unwrap()),
            )
        });

    let mut symbols = symbols.clone();
    let name = name.name().to_owned();
    let repr_exists = symbols.get_representation(&name, &["int_order"]).is_some();
    let representation = symbols
        .get_or_add_representation(&name, &["int_order"])
        .ok_or(RuleNotApplicable)?;
    let bits: Vec<Expr> = representation[0]
        .clone()
        .expression_down(&symbols)?
        .into_values()
        .collect();
    let cnf_int = Expr::SATInt(
        Metadata::new(),
        SATIntEncoding::Order,
        Moo::new(into_matrix_expr!(bits.clone())),
        (min, max),
    );

    if repr_exists {
        return Ok(RuleEffect::pure(cnf_int));
    }

    let constraints = vec![int_domain_to_expr(cnf_int.clone(), ranges)];
    let mut clauses = vec![];
    for i in 1..bits.len() {
        clauses.push(conjure_cp::ast::CnfClause::new(vec![
            Expr::Not(Metadata::new(), Moo::new(bits[i].clone())),
            bits[i - 1].clone(),
        ]));
    }
    if !bits.is_empty() {
        clauses.push(conjure_cp::ast::CnfClause::new(vec![bits[0].clone()]));
    }

    let mut reduction = RuleEffect::cnf(cnf_int, clauses, symbols);
    reduction.new_top = constraints;
    Ok(reduction)
}
