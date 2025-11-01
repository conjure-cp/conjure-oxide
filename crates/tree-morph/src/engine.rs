//! Perform gradual rule-based transformations on trees.
//!
//! See the [`morph`](Engine::morph) for more information.

use crate::events::EventHandlers;
use crate::helpers::{SelectorFn, one_or_select};
use crate::prelude::Rule;
use crate::rule::apply_into_update;

use paste::paste;
use uniplate::{Uniplate, tagged_zipper::TaggedZipper};

/// An engine for exhaustively transforming trees with user-defined rules.
///
/// See the [`morph`](Engine::morph) method for more information.
pub struct Engine<T, M, R>
where
    T: Uniplate,
    R: Rule<T, M>,
{
    pub(crate) event_handlers: EventHandlers<T, M>,

    /// A collection of groups of equally-prioritised rules.
    pub(crate) rule_groups: Vec<Vec<R>>,

    pub(crate) selector: SelectorFn<T, M, R>,
}

impl<T, M, R> Engine<T, M, R>
where
    T: Uniplate,
    R: Rule<T, M>,
{
    /// Exhaustively rewrites a tree using user-defined rule groups.
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
    /// To solve this, rules are organised into different collections or "groups".
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
    /// # Event Handlers
    ///
    /// The engine provides events for more fine-tuned control over rewriting behaviour. Events have mutable
    /// access to the current metadata.
    ///
    /// The engine will call the provided handlers for the "enter" and
    /// "exits" events as it enters and exits subtrees while traversing, respectively.
    ///
    /// The "enter" event is triggered first on the root, and then whenever the engine moves down
    /// into a subtree. As a result, when a node is passed to rules, all nodes above it will have
    /// been passed to handlers for this event, in ascending order of their proximity to the root.
    ///
    /// The "exit" event is triggered when the engine leaves a subtree.
    /// All nodes passed to "enter" event handlers (except the root) will also be passed to "exit"
    /// event handlers in reverse order.
    ///
    /// In effect, before a node is passed to rules, all nodes in the path from the root (including the
    /// current node) will have been passed to the "enter" event in order. No nodes are skipped.
    ///
    /// Event handling is useful when, for example, using a symbol table to keep track of variable definitions.
    /// When entering a scope where a variable is defined, one can place the variable and its value into the table.
    /// That stack can then be used for quick value lookup while inside the scope. When leaving the scope the
    /// variable can be removed from the table.
    ///
    /// # Optimizations
    ///
    /// To optimize the morph function, we use a dirty/clean approach. Since whether a rule applies
    /// is purely a function of a node and its children, rules should only be checked once unless a node
    /// or one of its children has been changed.
    ///
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
    /// let engine = EngineBuilder::new()
    ///     .set_selector(select_panic)
    ///     .add_rule_group(rule_fns![rule_eval_mul, rule_expand_sqr])
    ///     .build();
    /// let (result, num_applications) = engine.morph(expr.clone(), 0);
    /// assert_eq!(result, Expr::Val(4));
    /// assert_eq!(num_applications, 4); // The `Sqr` is expanded first, causing duplicate work
    ///
    /// // Move the evaluation rule to an earlier group
    /// let engine = EngineBuilder::new()
    ///     .set_selector(select_panic)
    ///     .add_rule_group(rule_fns![rule_eval_mul])
    ///     .add_rule_group(rule_fns![rule_expand_sqr])
    ///     .build();
    /// let (result, num_applications) = engine.morph(expr.clone(), 0);
    /// assert_eq!(result, Expr::Val(4));
    /// assert_eq!(num_applications, 3); // Now the sub-expression (1 * 2) is evaluated first
    /// ```
    pub fn morph(&self, tree: T, meta: M) -> (T, M)
    where
        T: Uniplate,
        R: Rule<T, M>,
    {
        // Owns the tree/meta and is consumed to get them back at the end
        let mut zipper = EngineZipper::new(tree, meta, &self.event_handlers);

        'main: loop {
            // Return here after every successful rule application

            for (level, rules) in self.rule_groups.iter().enumerate() {
                // Try each rule group in the whole tree

                while zipper.go_next_dirty(level).is_some() {
                    let subtree = zipper.inner.focus();

                    // Choose one transformation from all applicable rules at this level
                    let selected = {
                        let applicable = &mut rules.iter().filter_map(|rule| {
                            let update = apply_into_update(rule, subtree, &zipper.meta)?;
                            Some((rule, update))
                        });
                        one_or_select(self.selector, subtree, applicable)
                    };

                    if let Some(mut update) = selected {
                        // Replace the current subtree, invalidating subtree node states
                        zipper.inner.replace_focus(update.new_subtree);

                        // Mark all ancestors as dirty and move back to the root
                        zipper.mark_dirty_to_root();

                        let (new_tree, root_transformed) = update
                            .commands
                            .apply(zipper.inner.focus().clone(), &mut zipper.meta);

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

        zipper.into()
    }
}

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

macro_rules! movement_fns {
    (
        directions: [$($dir:ident),*]
    ) => {
        paste! {
            $(fn [<go_ $dir>](&mut self) -> Option<()> {
                self.inner.zipper().[<has_ $dir>]().then(|| {
                    self.event_handlers
                        .[<trigger_before_ $dir>](self.inner.focus(), &mut self.meta);
                    self.inner.[<go_ $dir>]().expect("zipper movement failed despite check");
                    self.event_handlers
                        .[<trigger_after_ $dir>](self.inner.focus(), &mut self.meta);
                })
            })*
        }
    };
}

/// A Zipper with optimisations for tree transformation.
struct EngineZipper<'events, T: Uniplate, M> {
    inner: TaggedZipper<T, EngineNodeState, fn(&T) -> EngineNodeState>,
    event_handlers: &'events EventHandlers<T, M>,
    meta: M,
}

impl<'events, T: Uniplate, M> EngineZipper<'events, T, M> {
    pub fn new(tree: T, meta: M, event_handlers: &'events EventHandlers<T, M>) -> Self {
        EngineZipper {
            inner: TaggedZipper::new(tree, EngineNodeState::new),
            event_handlers,
            meta,
        }
    }

    /// Go to the next node in the tree which is dirty for the given level.
    /// That node may be the current one if it is dirty.
    /// If no such node exists, go to the root and return `None`.
    pub fn go_next_dirty(&mut self, level: usize) -> Option<()> {
        if self.inner.tag().is_dirty(level) {
            return Some(());
        }

        self.go_down()
            .and_then(|_| {
                // go right until we find a dirty child, if it exists.
                loop {
                    if self.inner.tag().is_dirty(level) {
                        return Some(());
                    } else if self.go_right().is_none() {
                        // all children are clean
                        self.go_up();
                        return None;
                    }
                }
            })
            .or_else(|| {
                // Neither this node, nor any of its children are dirty
                // Go right then up until we find a dirty node or reach the root
                loop {
                    if self.go_right().is_some() {
                        if self.inner.tag().is_dirty(level) {
                            return Some(());
                        }
                    } else if self.go_up().is_none() {
                        // Reached the root without finding a dirty node
                        return None;
                    }
                }
            })
    }

    // We never move left in the tree
    movement_fns! { directions: [up, down, right] }

    /// Mark the current focus as visited at the given level.
    /// Calling `go_next_dirty` with the same level will no longer yield this node.
    pub fn set_dirty_from(&mut self, level: usize) {
        self.inner.tag_mut().set_dirty_from(level);
    }

    /// Mark ancestors as dirty for all levels, and return to the root
    pub fn mark_dirty_to_root(&mut self) {
        while self.go_up().is_some() {
            self.set_dirty_from(0);
        }
    }
}

impl<T: Uniplate, M> From<EngineZipper<'_, T, M>> for (T, M) {
    fn from(val: EngineZipper<'_, T, M>) -> Self {
        let meta = val.meta;
        let tree = val.inner.rebuild_root();
        (tree, meta)
    }
}
