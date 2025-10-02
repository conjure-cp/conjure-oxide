//! Perform gradual rule-based transformations on trees.
//!
//! See [`morph`] for more info.

use crate::helpers::one_or_select;
use crate::prelude::{Rule, Update};
use uniplate::{Uniplate, tagged_zipper::TaggedZipper};

#[derive(Debug, Clone)]
struct EngineNodeState {
    /// Rule groups with lower indices have already been applied without change.
    /// For a level `n`, a state is 'dirty' if and only if `n >= dirty_from`.
    dirty_from: usize,
}

impl EngineNodeState {
    /// Marks the state as dirty for anything >= `level`.
    fn set_dirty_from(&mut self, level: usize) {
        self.dirty_from = level;
    }

    /// For a level `n`, a state is "dirty" if and only if `n >= dirty_from`.
    /// That is, all rules groups before `n` have been applied without change.
    fn is_dirty(&self, level: usize) -> bool {
        level >= self.dirty_from
    }
}

impl EngineNodeState {
    fn new<T: Uniplate>(_: &T) -> Self {
        Self { dirty_from: 0 }
    }
}

/// A Zipper with optimisations for tree transformation.
#[derive(Clone)]
struct EngineZipper<T: Uniplate> {
    inner: TaggedZipper<T, EngineNodeState, fn(&T) -> EngineNodeState>,
}

impl<T: Uniplate> EngineZipper<T> {
    pub fn new(tree: T) -> Self {
        Self {
            inner: TaggedZipper::new(tree, EngineNodeState::new),
        }
    }

    /// Go to the next node in the tree which is dirty for the given level.
    /// That node may be the current one if it is dirty.
    /// If no such node exists, go to the root and return `None`.
    pub fn go_next_dirty(&mut self, level: usize) -> Option<()> {
        if self.inner.tag().is_dirty(level) {
            return Some(());
        }

        self.inner
            .go_down()
            .and_then(|_| {
                // go right until we find a dirty child, if it exists.
                loop {
                    if self.inner.tag().is_dirty(level) {
                        return Some(());
                    } else if self.inner.go_right().is_none() {
                        // all children are clean
                        self.inner.go_up();
                        return None;
                    }
                }
            })
            .or_else(|| {
                // Neither this node, nor any of its children are dirty
                // Go right then up until we find a dirty node or reach the root
                loop {
                    if self.inner.go_right().is_some() {
                        if self.inner.tag().is_dirty(level) {
                            return Some(());
                        }
                    } else if self.inner.go_up().is_none() {
                        // Reached the root without finding a dirty node
                        return None;
                    }
                }
            })
    }

    /// Mark the current focus as visited at the given level.
    /// Calling `go_next_dirty` with the same level will no longer yield this node.
    pub fn set_dirty_from(&mut self, level: usize) {
        self.inner.tag_mut().set_dirty_from(level);
    }

    /// Mark ancestors as dirty for all levels, and return to the root
    pub fn mark_dirty_to_root(&mut self) {
        while self.inner.go_up().is_some() {
            self.set_dirty_from(0);
        }
    }

    pub fn rebuild_root(self) -> T {
        self.inner.rebuild_root()
    }
}

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
/// # Optimizations
///
/// To optimize the morph function, we use a dirty/clean approach. Since whether a rule applies
/// is purely a function of a node and its children, rules should only be checked once unless a node
/// or one of its children has been changed. To enforce this we use a skeleton tree approach, which
/// keeps track of the current level of which a node has had a rule check applied.
/// # Example
/// ```rust
/// use tree_morph::{prelude::*, helpers::select_panic};
/// use uniplate::Uniplate;
///
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
    mut meta: M,
) -> (T, M)
where
    T: Uniplate,
    R: Rule<T, M>,
{
    let mut zipper = EngineZipper::new(tree);

    'main: loop {
        // Return here after every successful rule application

        for (level, rules) in groups.iter().enumerate() {
            // Try each rule group in the whole tree

            while zipper.go_next_dirty(level).is_some() {
                let subtree = zipper.inner.focus();

                // Choose one transformation from all applicable rules at this level
                let applicable = &mut rules.iter().filter_map(|rule| {
                    let update = rule.apply_into_update(subtree, &meta)?;
                    Some((rule, update))
                });
                let selected = one_or_select(&select, subtree, applicable);

                if let Some(mut update) = selected {
                    // Replace the current subtree, invalidating subtree node states
                    zipper.inner.replace_focus(update.new_subtree);

                    // Mark all ancestors as dirty and move back to the root
                    zipper.mark_dirty_to_root();

                    let (new_tree, new_meta, root_transformed) =
                        update.commands.apply(zipper.inner.focus().clone(), meta);

                    meta = new_meta;
                    if root_transformed {
                        // This must unfortunately throw all node states away,
                        // since the `transform` command may redefine the whole tree
                        zipper.inner.replace_focus(new_tree);
                    }

                    continue 'main;
                } else {
                    zipper.set_dirty_from(level + 1);
                }
            }
        }

        // All rules have been tried with no more changes
        break;
    }

    (zipper.rebuild_root(), meta)
}
