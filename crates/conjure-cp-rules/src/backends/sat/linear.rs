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

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{DeclarationPtr, Domain, Name};
    use conjure_cp::into_matrix_expr;

    fn value(name: &str) -> Expr {
        let bits = (0..3)
            .map(|index| {
                Expr::from(Reference::new(DeclarationPtr::new_find(
                    Name::user(&format!("{name}{index}")),
                    Domain::bool(),
                )))
            })
            .collect();
        Expr::SATInt(
            Metadata::new(),
            SATIntEncoding::Log,
            Moo::new(into_matrix_expr!(bits)),
            (-2, 2),
        )
    }

    fn sum() -> Expr {
        Expr::Sum(
            Metadata::new(),
            Moo::new(into_matrix_expr!(vec![value("x"), 2.into()])),
        )
    }

    fn root(input: Expr) -> Expr {
        Expr::Root(
            Metadata::new(),
            vec![Expr::Eq(
                Metadata::new(),
                Moo::new(input),
                Moo::new(value("z")),
            )],
        )
    }

    #[test]
    fn linear_comparisons_do_not_introduce_intermediate_values() {
        assert!(materialise_linear_operand(&root(sum()), &SymbolTable::new()).is_err());
    }

    #[test]
    fn hoisted_definitions_are_not_materialised_again() {
        let linear = sum();
        let input = Expr::SafeDiv(
            Metadata::new(),
            Moo::new(linear.clone()),
            Moo::new(value("y")),
        );
        let effect = materialise_linear_operand(&root(input), &SymbolTable::new()).unwrap();
        assert_eq!(effect.new_top.len(), 1);
        let Expr::Eq(_, result, original) = &effect.new_top[0] else {
            panic!("expected a definition");
        };
        assert!(matches!(result.as_ref(), Expr::Atomic(..)));
        assert_eq!(original.as_ref(), &linear);
        let definition = Expr::Root(Metadata::new(), effect.new_top.clone());
        assert!(materialise_linear_operand(&definition, &effect.symbols).is_err());
    }

    #[test]
    fn constant_scaling_accepts_encoded_literal_bits() {
        let constant = Expr::SATInt(
            Metadata::new(),
            SATIntEncoding::Log,
            Moo::new(into_matrix_expr!(vec![
                true.into(),
                false.into(),
                true.into()
            ])),
            (-3, -3),
        );
        let product = Expr::Product(
            Metadata::new(),
            Moo::new(into_matrix_expr!(vec![value("x"), constant])),
        );
        let view = super::super::pseudo_boolean::integer_view(&product).unwrap();
        assert_eq!(
            view.terms
                .iter()
                .map(|(weight, _)| *weight)
                .collect::<Vec<_>>(),
            vec![-3, -6, 12]
        );
        let division = Expr::SafeDiv(Metadata::new(), Moo::new(product), Moo::new(value("y")));
        assert!(materialise_linear_operand(&root(division), &SymbolTable::new()).is_ok());
    }
}
