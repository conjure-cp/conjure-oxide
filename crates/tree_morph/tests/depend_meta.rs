//! Here we test an interesting side-effect case, with rules which return a reduction based on a metadata field.
//! These rules will not be run a second time if no other rule applies to the same node, which might be unexpected.

use tree_morph::*;
use uniplate::derive::Uniplate;

#[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
#[uniplate()]
enum Expr {
    One,
    Two,
}

fn transform(cmd: &mut Commands<Expr, bool>, expr: &Expr, meta: &bool) -> Option<Expr> {
    if let Expr::One = expr {
        if *meta {
            return Some(Expr::Two);
        } else {
            cmd.mut_meta(|m| *m = true); // The next application of this rule will apply
        }
    }
    None
}

// #[test] // TODO (Felix) how might we fix this, in the engine or by blocking this use case?
fn test_meta_branching_side_effect() {
    let expr = Expr::One;
    let (expr, meta) = reduce(transform, expr, false);
    assert_eq!(expr, Expr::Two);
    assert_eq!(meta, true);
}
