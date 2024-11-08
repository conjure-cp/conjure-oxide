use uniplate::Biplate;

use crate::ast::{Expression, Factor};

/// True iff the entire AST is constants.
pub fn is_all_constant(expression: &Expression) -> bool {
    for factor in <Expression as Biplate<Factor>>::universe_bi(expression) {
        match factor {
            Factor::Literal(_) => {}
            Factor::Reference(_) => {
                return false;
            }
        }
    }

    true
}
