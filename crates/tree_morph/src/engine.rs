use crate::{helpers::one_or_select, Commands, Rule, Update};
use uniplate::Uniplate;

/// Exhaustively rewrites a tree using a set of transformation rules.
///
/// Rewriting is complete when all rules have been attempted with no change. Rules may be organised
/// into groups to control the order in which they are attempted.
///
/// # Rule Groups
/// If all rules are treated equally, those which apply higher in the tree will take precedence
/// because of the left-most outer-most traversal order of the engine.
///
/// This can cause problems if a rule which should ideally be applied early (e.g. evaluating
/// constant expressions) is left until later.
///
/// To solve this, rules can be organised into different collections in the `groups` argument.
/// The engine will apply rules in an earlier group to the entire tree before trying later groups.
/// That is, no rule is attempted if a rule in an earlier group is applicable to any part of the
/// tree.
///
/// # Selector Functions
///
/// If multiple rules in the same group are applicable to an expression, the user-defined
/// selector function is used to choose one. This function is given an iterator over pairs of
/// rules and the engine-created [`Update`] values which contain their modifications to the tree.
///
/// Some useful selector functions are available in the [`helpers`](crate::helpers) module. One
/// common function is [`select_first`](crate::helpers::select_first), which simply returns the
/// first applicable rule.
///
/// # Example
/// ```rust
/// use tree_morph::{prelude::*, helpers::select_panic};
/// use uniplate::derive::Uniplate;
///
/// // A simple language of multiplied and squared constant expressions
/// #[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
/// #[uniplate()]
/// enum Expr {
///     Val(i32),
///     Mul(Box<Expr>, Box<Expr>),
///     Sqr(Box<Expr>),
/// }
///
/// // a * b ~> (value of) a * b, where 'a' and 'b' are literal values
/// fn rule_eval_mul(cmds: &mut Commands<Expr, i32>, subtree: &Expr, meta: &i32) -> Option<Expr> {
///     cmds.mut_meta(Box::new(|m: &mut i32| *m += 1));
///
///     if let Expr::Mul(a, b) = subtree {
///         if let (Expr::Val(a_v), Expr::Val(b_v)) = (a.as_ref(), b.as_ref()) {
///             return Some(Expr::Val(a_v * b_v));
///         }
///     }
///     None
/// }
///
/// // e ^ 2 ~> e * e, where e is an expression
/// // If this rule is applied before the sub-expression is fully evaluated, duplicate work
/// // will be done on the resulting two identical sub-expressions.
/// fn rule_expand_sqr(cmds: &mut Commands<Expr, i32>, subtree: &Expr, meta: &i32) -> Option<Expr> {
///     cmds.mut_meta(Box::new(|m: &mut i32| *m += 1));
///
///     if let Expr::Sqr(expr) = subtree {
///         return Some(Expr::Mul(
///             Box::new(*expr.clone()),
///             Box::new(*expr.clone())
///         ));
///     }
///     None
/// }
///
/// // (1 * 2) ^ 2
/// let expr = Expr::Sqr(
///     Box::new(Expr::Mul(
///         Box::new(Expr::Val(1)),
///         Box::new(Expr::Val(2))
///     ))
/// );
///
/// // Try with both rules in the same group, keeping track of the number of rule applications
/// let (result, num_applications) = morph(
///     vec![rule_fns![rule_eval_mul, rule_expand_sqr]],
///     select_panic,
///     expr.clone(),
///     0
/// );
/// assert_eq!(result, Expr::Val(4));
/// assert_eq!(num_applications, 4); // The `Sqr` is expanded first, causing duplicate work
///
/// // Move the evaluation rule to an earlier group
/// let (result, num_applications) = morph(
///     vec![rule_fns![rule_eval_mul], rule_fns![rule_expand_sqr]],
///     select_panic,
///     expr.clone(),
///     0
/// );
/// assert_eq!(result, Expr::Val(4));
/// assert_eq!(num_applications, 3); // Now the sub-expression (1 * 2) is evaluated first
/// ```
pub fn morph<T, M, R>(
    groups: Vec<Vec<R>>,
    select: impl Fn(&T, &mut dyn Iterator<Item = (&R, Update<T, M>)>) -> Option<Update<T, M>>,
    tree: T,
    meta: M,
) -> (T, M)
where
    T: Uniplate,
    R: Rule<T, M>,
{
    let transforms: Vec<_> = groups
        .iter()
        .map(|group| {
            |subtree: &T, meta: &M| {
                let applicable = &mut group.iter().filter_map(|rule| {
                    let mut commands = Commands::new();
                    let new_tree = rule.apply(&mut commands, subtree, meta)?;
                    Some((
                        rule,
                        Update {
                            new_subtree: new_tree,
                            commands,
                        },
                    ))
                });
                one_or_select(&select, subtree, applicable)
            }
        })
        .collect();
    morph_impl(transforms, tree, meta)
}

/// This implements the core rewriting logic for the engine.
///
/// Iterate over rule groups and apply each one to the tree. If any changes are
/// made, restart with the first rule group.
fn morph_impl<T: Uniplate, M>(
    transforms: Vec<impl Fn(&T, &M) -> Option<Update<T, M>>>,
    mut tree: T,
    mut meta: M,
) -> (T, M) {
    let mut new_tree = tree;

    'main: loop {
        tree = new_tree;
        for transform in transforms.iter() {
            // Try each transform on the entire tree before moving to the next
            for (node, ctx) in tree.contexts() {
                if let Some(mut update) = transform(&node, &meta) {
                    let whole_tree = ctx(update.new_subtree);
                    (new_tree, meta) = update.commands.apply(whole_tree, meta);

                    // Restart with the first transform every time a change is made
                    continue 'main;
                }
            }
        }
        // All transforms were attempted without change
        break;
    }
    (tree, meta)
}
