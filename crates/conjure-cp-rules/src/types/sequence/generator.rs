use conjure_cp::ast::serde::HasId as _;
use conjure_cp::ast::{
    AbstractLiteral, Atom, DeclarationPtr, Expression as Expr, Metadata, Moo, ReturnType,
    SymbolTable, Typeable, comprehension::ComprehensionQualifier,
};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};
use uniplate::Uniplate as _;

/// Lower iteration over a sequence to iteration over its positions.
///
/// ```plain
/// [ e | t <- s ]
/// ~~>
/// [ e[t := (i, s(i))] | i : int(1..maxSize), i <= |s| ]
/// ```
///
/// A sequence is a function from `int(1..|s|)`, so iterating it yields `(position, value)` pairs
/// -- the same convention [`Domain::element_domain`] uses, and what a destructuring generator such
/// as `forAll (index, _) in s` expects.
#[register_rule("Base", 8600, [Comprehension])]
fn lower_sequence_expression_generator(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Comprehension(metadata, comprehension) = expr else {
        return Err(RuleNotApplicable);
    };

    let Some((index, old_ptr, source)) =
        comprehension
            .qualifiers
            .iter()
            .enumerate()
            .find_map(|(index, qualifier)| {
                let ComprehensionQualifier::ExpressionGenerator { ptr } = qualifier else {
                    return None;
                };
                let source = (*ptr.as_quantified_expr()?).clone();
                matches!(source.return_type(), ReturnType::Sequence(_))
                    .then(|| (index, ptr.clone(), source))
            })
    else {
        return Err(RuleNotApplicable);
    };

    // The element domain of a sequence is the (position, value) pair; its first component is the
    // range of positions to iterate over.
    let element_domain = source
        .domain_of()
        .and_then(|domain| domain.element_domain())
        .ok_or(RuleNotApplicable)?;
    let position_domain = element_domain
        .as_ground()
        .and_then(|ground| match ground {
            conjure_cp::ast::GroundDomain::Tuple(components) => components.first().cloned(),
            _ => None,
        })
        .ok_or(RuleNotApplicable)?;

    let position = DeclarationPtr::new_quantified(old_ptr.name().clone(), position_domain.into());
    let position_ref = Expr::from(conjure_cp::ast::Reference::new(position.clone()));
    let pair = Expr::AbstractLiteral(
        Metadata::new(),
        AbstractLiteral::Tuple(vec![
            position_ref.clone(),
            Expr::Image(
                Metadata::new(),
                Moo::new(source.clone()),
                Moo::new(position_ref.clone()),
            ),
        ]),
    );

    // References to the generator variable stand for the whole pair, so they are replaced
    // expression-for-expression rather than by rebinding the declaration.
    let old_id = old_ptr.id();
    let substitute = |expr: Expr| {
        expr.transform(&|expr| match &expr {
            Expr::Atomic(_, Atom::Reference(reference)) if reference.id() == old_id => pair.clone(),
            _ => expr,
        })
    };

    let mut comprehension = comprehension.as_ref().clone();
    comprehension.symbols = comprehension.symbols.detach();
    comprehension.return_expression = substitute(comprehension.return_expression);
    comprehension.qualifiers = comprehension
        .qualifiers
        .into_iter()
        .map(|qualifier| match qualifier {
            ComprehensionQualifier::Condition(condition) => {
                ComprehensionQualifier::Condition(substitute(condition))
            }
            qualifier => qualifier,
        })
        .collect();

    let within_length = Expr::Leq(
        Metadata::new(),
        Moo::new(position_ref),
        Moo::new(Expr::Card(Metadata::new(), Moo::new(source))),
    );
    comprehension.qualifiers.splice(
        index..=index,
        [
            ComprehensionQualifier::Generator {
                ptr: position.clone(),
            },
            ComprehensionQualifier::Condition(within_length),
        ],
    );
    comprehension.symbols.write().update_insert(position);

    Ok(RuleEffect::pure(Expr::Comprehension(
        metadata.clone(),
        Moo::new(comprehension),
    )))
}
