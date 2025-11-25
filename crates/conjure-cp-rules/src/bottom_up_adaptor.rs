use conjure_cp::rule_engine::SubmodelZipper;
use conjure_cp::{
    ast::{Expression, SymbolTable},
    rule_engine::{ApplicationError::RuleNotApplicable, ApplicationResult, Reduction},
};

/// Converts the rule function `rule` to a rule that applies `rule` bottom-up to every expression
/// in the current sub-model.
pub fn as_bottom_up(
    rule: impl Fn(&Expression, &SymbolTable) -> ApplicationResult,
) -> impl Fn(&Expression, &SymbolTable) -> ApplicationResult {
    Box::new(move |expr: &Expression, symbols: &SymbolTable| {
        // global rule
        if !matches!(expr, Expression::Root(_, _)) {
            return Err(RuleNotApplicable);
        };

        // traverse bottom up within the current sub-model, applying the rule.
        let mut symbols = symbols.clone();
        let mut new_tops = vec![];
        let mut done_something = false;

        let mut zipper = SubmodelZipper::new(expr.clone());

        while zipper.go_down().is_some() {}

        loop {
            // go right and to the bottom of that subtree
            //
            // once we have ran out of siblings, go_up.
            if zipper.go_right().is_some() {
                while zipper.go_down().is_some() {}
            } else if zipper.go_up().is_none() {
                // cannot go up anymore, at the root
                break;
            }

            let expr = zipper.focus();

            if let Ok(mut reduction) = rule(expr, &symbols) {
                zipper.replace_focus(reduction.new_expression);
                symbols.extend(reduction.symbols);
                new_tops.append(&mut reduction.new_top);
                done_something = true;
            }
        }

        let root_expr = zipper.rebuild_root();

        if done_something {
            Ok(Reduction::new(root_expr, new_tops, symbols))
        } else {
            Err(RuleNotApplicable)
        }
    })
}
