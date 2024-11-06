use uniplate::Uniplate;

use crate::ast::Expression;

/// True iff the entire AST is constants.
pub fn is_all_constant(expression: &Expression) -> bool {
    for expr in expression.universe() {
        match expr {
            Expression::Constant(_, _) => {}
            Expression::Reference(_, _) => {
                return false;
            }
            _ => {}
        }
    }

    true
}
