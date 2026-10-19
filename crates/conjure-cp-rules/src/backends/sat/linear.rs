//! Materialise linear operands through equalities handled by the selected PB provider.
use conjure_cp::ast::{Expression as Expr, Metadata, Moo, Reference, SATIntEncoding, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};
use uniplate::Uniplate;

/// Leave linear comparisons intact; only circuits need an intermediate represented value.
#[register_rule("SAT", 9450, [Root])]
fn materialise_linear_operand(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let mut path = Vec::new();
    let input = find_operand(expr, false, &mut path).ok_or(RuleNotApplicable)?;
    let domain = input.domain_of().ok_or(RuleNotApplicable)?;
    let mut symbols = symbols.clone();
    let result: Expr = Reference::new(symbols.gen_find_auxiliary(&domain)).into();
    let definition = Expr::Eq(Metadata::new(), Moo::new(result.clone()), Moo::new(input));
    let rewritten = replace_at(expr, &path, result);
    Ok(RuleEffect::new(rewritten, vec![definition], symbols))
}

fn find_operand(expr: &Expr, needed: bool, path: &mut Vec<usize>) -> Option<Expr> {
    // Code bits contain no numeric operands; binders must be expanded before hoisting.
    if matches!(
        expr,
        Expr::Atomic(..) | Expr::SATInt(..) | Expr::Comprehension(..)
    ) {
        return None;
    }
    if needed
        && is_linear_operation(expr)
        && !free_negation(expr)
        && super::pseudo_boolean::integer_view(expr).is_some_and(|view| !view.terms.is_empty())
    {
        return Some(expr.clone());
    }
    let nonlinear = matches!(
        expr,
        Expr::Abs(..)
            | Expr::Min(..)
            | Expr::Max(..)
            | Expr::SafeDiv(..)
            | Expr::SafeMod(..)
            | Expr::SafePow(..)
    ) || (matches!(expr, Expr::Product(..))
        && super::pseudo_boolean::integer_view(expr).is_none());
    // List wrappers pass the enclosing operation's requirement to each numeric operand.
    let children_needed = nonlinear
        || (needed
            && matches!(
                expr,
                Expr::AbstractLiteral(_, conjure_cp::ast::AbstractLiteral::Matrix(..))
            ));
    for (index, child) in expr.children().iter().enumerate() {
        path.push(index);
        if let Some(found) = find_operand(child, children_needed, path) {
            return Some(found);
        }
        path.pop();
    }
    None
}

fn is_linear_operation(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Sum(..) | Expr::Minus(..) | Expr::Neg(..) | Expr::Product(..)
    )
}

fn free_negation(expr: &Expr) -> bool {
    matches!(expr, Expr::Neg(_, input)
        if matches!(input.as_ref(), Expr::SATInt(_, SATIntEncoding::Direct | SATIntEncoding::Order, ..)))
}

fn replace_at(expr: &Expr, path: &[usize], replacement: Expr) -> Expr {
    let Some((&index, tail)) = path.split_first() else {
        return replacement;
    };
    let mut children = expr.children();
    children[index] = replace_at(&children[index], tail, replacement);
    expr.with_children(children)
}
