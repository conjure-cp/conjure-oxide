//! Perform gradual rule-based transformations on trees.
//!
//! See the [`morph`](Engine::morph) for more information.

use crate::cache::{CacheResult, RewriteCache};
use crate::engine_zipper::{EngineZipper, NaiveZipper};
use crate::events::EventHandlers;
use crate::helpers::{SelectorFn, one_or_select};
use crate::prelude::Rule;
use crate::rule::{RuleGroups, RuleSet, apply_into_update};
use crate::update::Update;

use rayon::prelude::*;
use tracing::{debug, error, info, instrument, trace};
use uniplate::Uniplate;

/// An engine for exhaustively transforming trees with user-defined rules.
///
/// See the [`morph`](Engine::morph) method for more information.
pub struct Engine<T, M, R, C>
where
    T: Uniplate + Send + Sync,
    R: Rule<T, M> + Clone,
    C: RewriteCache<T>,
{
    pub(crate) event_handlers: EventHandlers<T, M, R>,

    /// A collection of groups of equally-prioritised rules.
    pub(crate) rule_groups: RuleGroups<T, M, R>,

    pub(crate) selector: SelectorFn<T, M, R>,

    pub(crate) cache: C,

    /// Whether to use parallel (Rayon) iteration when checking rules.
    pub(crate) parallel: bool,

    pub(crate) faster: bool,
}

impl<T, M, R, C> Engine<T, M, R, C>
where
    T: Uniplate + Send + Sync,
    R: Rule<T, M> + Clone + Sync,
    M: Sync,
    C: RewriteCache<T>,
{
    #[instrument(skip(selector, parallel, subtree, meta, rules))]
    fn select_rule<'a>(
        selector: SelectorFn<T, M, R>,
        parallel: bool,
        subtree: &T,
        meta: &mut M,
        rules: RuleSet<'a, R>,
    ) -> Option<(&'a R, Update<T, M>)> {
        trace!("Beginning Rule Checks");
        if parallel {
            let applicable: Vec<(&'a R, Update<T, M>)> = rules
                .par_iter()
                .filter_map(|rule| {
                    let update = apply_into_update(rule, subtree, meta)?;
                    Some((rule, update))
                })
                .collect();
            trace!("Finished Rule Checks");
            one_or_select(selector, subtree, &mut applicable.into_iter())
        } else {
            let applicable = &mut rules.into_iter().filter_map(|rule| {
                let update = apply_into_update(rule, subtree, meta)?;
                Some((rule, update))
            });
            trace!("Finished Rule Checks");
            one_or_select(selector, subtree, applicable)
        }
    }

    fn apply_rule(
        zipper: &mut EngineZipper<T, M, R, C>,
        event_handlers: &EventHandlers<T, M, R>,
        application: (&R, Update<T, M>),
        level: usize,
    ) {
        let (rule, mut update) = application;
        debug!("Applying Rule '{}'", rule.name());

        let cache_active = zipper.cache.is_active();
        let original = cache_active.then(|| zipper.focus().clone());
        let replacement = cache_active.then(|| update.new_subtree.clone());

        // Replace the current subtree, invalidating subtree node states
        zipper.replace_focus(update.new_subtree);

        // Mark all ancestors as dirty, insert ancestor mappings, and move back to the root
        zipper.mark_dirty_to_root(level);

        let (focus, meta) = zipper.focus_and_meta();
        let (new_tree, root_transformed) = update.commands.apply(focus.clone(), meta);

        if root_transformed {
            debug!("Root transformed, clearing state.");
            // This must unfortunately throw all node states away,
            // since the `transform` command may redefine the whole tree
            zipper.replace_focus(new_tree);
        } else if let (Some(orig), Some(repl)) = (original, replacement) {
            if orig != repl {
                zipper.cache.insert(&orig, Some(repl), level);
            } else {
                error!("SAME TREE");
            }
        }

        let (focus, meta) = zipper.focus_and_meta();
        event_handlers.trigger_on_apply(focus, meta, rule);
    }

    fn apply_rule_faster(
        zipper: &mut EngineZipper<T, M, R, C>,
        event_handlers: &EventHandlers<T, M, R>,
        rule_groups: &RuleGroups<T, M, R>,
        selector: SelectorFn<T, M, R>,
        parallel: bool,
        application: (&R, Update<T, M>),
        level: usize,
    ) {
        // Apply the initial rule
        let (rule, mut update) = application;
        debug!("Applying Rule '{}' in Fast Mode", rule.name());

        if update.has_transform() {
            Self::apply_rule(zipper, event_handlers, (rule, update), level);
            return;
        }

        // No transform — apply locally
        let cache_active = zipper.cache.is_active();
        let original = cache_active.then(|| zipper.focus().clone());
        let replacement = cache_active.then(|| update.new_subtree.clone());
        zipper.replace_focus(update.new_subtree);

        {
            let (focus, meta) = zipper.focus_and_meta();
            update.commands.apply(focus.clone(), meta);
        }

        if let (Some(orig), Some(repl)) = (original, replacement) {
            if orig != repl {
                zipper.cache.insert(&orig, Some(repl), level);
            } else {
                error!("SAME TREE");
            }
        }

        let (focus, meta) = zipper.focus_and_meta();
        event_handlers.trigger_on_apply(focus, meta, rule);

        let mut try_level = 0;
        while try_level <= level {
            let subtree = zipper.focus();
            let id = rule_groups.discriminant_fn.map(|f| f(subtree));
            let rules = rule_groups.get_rules(try_level, id);

            let (subtree, meta) = zipper.focus_and_meta();
            match Self::select_rule(selector, parallel, subtree, meta, rules) {
                Some((rule, mut update)) => {
                    if update.has_transform() {
                        Self::apply_rule(zipper, event_handlers, (rule, update), level);
                        return;
                    }

                    debug!("Applying Rule '{}' in Fast Mode (local)", rule.name());
                    let cache_active = zipper.cache.is_active();
                    let original = cache_active.then(|| zipper.focus().clone());
                    let replacement = cache_active.then(|| update.new_subtree.clone());
                    zipper.replace_focus(update.new_subtree);

                    {
                        let (focus, meta) = zipper.focus_and_meta();
                        update.commands.apply(focus.clone(), meta);
                    }

                    if let (Some(orig), Some(repl)) = (original, replacement) {
                        if orig != repl {
                            zipper.cache.insert(&orig, Some(repl), level);
                        } else {
                            error!("SAME TREE");
                        }
                    }

                    let (focus, meta) = zipper.focus_and_meta();
                    event_handlers.trigger_on_apply(focus, meta, rule);

                    try_level = 0;
                }
                None => {
                    try_level += 1;
                }
            }
        }

        // Node stabilized — mark pass-through and go to root
        zipper.set_pass_through(level);
        zipper.mark_dirty_to_root(level);
    }

    fn apply_rule_naive_faster(
        zipper: &mut NaiveZipper<T, M, R, C>,
        event_handlers: &EventHandlers<T, M, R>,
        rule_groups: &RuleGroups<T, M, R>,
        selector: SelectorFn<T, M, R>,
        parallel: bool,
        application: (&R, Update<T, M>),
        level: usize,
    ) -> bool {
        let (rule, mut update) = application;
        debug!("Applying Rule '{}' in Fast Mode (naive)", rule.name());

        if update.has_transform() {
            Self::apply_rule_naive(zipper, event_handlers, (rule, update), level);
            return false;
        }

        // No transform — apply locally (no root traversal)
        let cache_active = zipper.cache.is_active();
        let original = cache_active.then(|| zipper.focus().clone());
        let replacement = cache_active.then(|| update.new_subtree.clone());
        zipper.replace_focus(update.new_subtree);

        {
            let (focus, meta) = zipper.focus_and_meta();
            update.commands.apply(focus.clone(), meta);
        }

        if let (Some(orig), Some(repl)) = (original, replacement) {
            if orig != repl {
                zipper.cache.insert(&orig, Some(repl), level);
            } else {
                error!("SAME TREE");
            }
        }

        let (focus, meta) = zipper.focus_and_meta();
        event_handlers.trigger_on_apply(focus, meta, rule);

        // Try rules from level 0 up to current level
        let mut try_level = 0;
        while try_level <= level {
            let subtree = zipper.focus();
            let id = rule_groups.discriminant_fn.map(|f| f(subtree));
            let rules = rule_groups.get_rules(try_level, id);

            let (subtree, meta) = zipper.focus_and_meta();
            match Self::select_rule(selector, parallel, subtree, meta, rules) {
                Some((rule, mut update)) => {
                    if update.has_transform() {
                        Self::apply_rule_naive(zipper, event_handlers, (rule, update), level);
                        return false;
                    }

                    debug!(
                        "Applying Rule '{}' in Fast Mode (naive, local)",
                        rule.name()
                    );
                    let cache_active = zipper.cache.is_active();
                    let original = cache_active.then(|| zipper.focus().clone());
                    let replacement = cache_active.then(|| update.new_subtree.clone());
                    zipper.replace_focus(update.new_subtree);

                    {
                        let (focus, meta) = zipper.focus_and_meta();
                        update.commands.apply(focus.clone(), meta);
                    }

                    if let (Some(orig), Some(repl)) = (original, replacement) {
                        if orig != repl {
                            zipper.cache.insert(&orig, Some(repl), level);
                        } else {
                            error!("SAME TREE");
                        }
                    }

                    let (focus, meta) = zipper.focus_and_meta();
                    event_handlers.trigger_on_apply(focus, meta, rule);

                    try_level = 0;
                }
                None => {
                    try_level += 1;
                }
            }
        }

        // Node stabilized — need to go to root and rebuild ancestors
        zipper.map_ancestors_to_root(level);
        true
    }

    fn apply_rule_naive(
        zipper: &mut NaiveZipper<T, M, R, C>,
        event_handlers: &EventHandlers<T, M, R>,
        application: (&R, Update<T, M>),
        level: usize,
    ) {
        let (rule, mut update) = application;
        debug!("Applying Rule '{}'", rule.name());

        let cache_active = zipper.cache.is_active();
        let original = cache_active.then(|| zipper.focus().clone());
        let replacement = cache_active.then(|| update.new_subtree.clone());

        zipper.replace_focus(update.new_subtree);
        zipper.map_ancestors_to_root(level);

        let (focus, meta) = zipper.focus_and_meta();
        let (new_tree, root_transformed) = update.commands.apply(focus.clone(), meta);

        if root_transformed {
            trace!("Root transformed.");
            zipper.replace_focus(new_tree);
        } else if let (Some(orig), Some(repl)) = (original, replacement) {
            if orig != repl {
                zipper.cache.insert(&orig, Some(repl), level);
            } else {
                error!("SAME TREE");
            }
        }

        let (focus, meta) = zipper.focus_and_meta();
        event_handlers.trigger_on_apply(focus, meta, rule);
    }

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
    // #[instrument(skip(self, tree, meta))]
    pub fn morph(&mut self, tree: T, meta: M) -> (T, M)
    where
        T: Uniplate + Send + Sync,
        R: Rule<T, M>,
    {
        // Owns the tree/meta and is consumed to get them back at the end
        let mut zipper = EngineZipper::new(tree, meta, &self.event_handlers, &mut self.cache);
        info!("Beginning Morph");

        'main: loop {
            // Return here after every successful rule application
            for level in 0..self.rule_groups.levels() {
                while zipper.go_next_dirty(level).is_some() {
                    trace!("Got Dirty, Level {}", level);

                    {
                        let subtree = zipper.focus();
                        let id = self.rule_groups.discriminant_fn.map(|f| f(subtree));
                        let rules = self.rule_groups.get_rules(level, id);

                        debug!("Checking Level {} with {} Rules", level, rules.len());
                        match zipper.cache.get(subtree, level) {
                            CacheResult::Terminal(clean_level) => {
                                debug!(
                                    "Cache Hit - Nothing Applicable (clean through level {})",
                                    clean_level
                                );
                                zipper.trigger_cache_hit();
                                zipper.set_dirty_from(clean_level + 1);
                                continue;
                            }
                            CacheResult::Rewrite(cached) => {
                                debug!("Cache Hit");
                                zipper.trigger_cache_hit();
                                zipper.replace_focus(cached);
                                zipper.mark_dirty_to_root(level);
                                continue 'main;
                            }
                            _ => {
                                zipper.trigger_cache_miss();
                            }
                        };
                    }

                    // Choose one transformation from all applicable rules at this level
                    let (subtree, meta) = zipper.focus_and_meta();
                    let id = self.rule_groups.discriminant_fn.map(|f| f(subtree));

                    let rules = self.rule_groups.get_rules(level, id);
                    match Self::select_rule(self.selector, self.parallel, subtree, meta, rules) {
                        Some(selected) => {
                            if self.faster {
                                Self::apply_rule_faster(
                                    &mut zipper,
                                    &self.event_handlers,
                                    &self.rule_groups,
                                    self.selector,
                                    self.parallel,
                                    selected,
                                    level,
                                );
                            } else {
                                Self::apply_rule(
                                    &mut zipper,
                                    &self.event_handlers,
                                    selected,
                                    level,
                                );
                            }
                            continue 'main;
                        }
                        None => {
                            trace!("Nothing Applicable");
                            if zipper.cache.is_active() {
                                let subtree = zipper.focus().clone();
                                zipper.cache.insert(&subtree, None, level);
                            }
                            zipper.set_dirty_from(level + 1);
                        }
                    }
                }
            }

            // All rules have been tried with no more changes
            break;
        }

        info!("Finished Morph");
        zipper.into()
    }

    /// Exhaustively rewrites a tree using user-defined rule groups.
    ///
    /// It is not recommended to use this function besides testing and for correctness.
    pub fn morph_naive(&mut self, tree: T, meta: M) -> (T, M)
    where
        T: Uniplate,
        R: Rule<T, M>,
    {
        let mut zipper = NaiveZipper::new(tree, meta, &self.event_handlers, &mut self.cache);
        info!("Beginning Naive Morph");

        'main: loop {
            for level in 0..self.rule_groups.levels() {
                loop {
                    {
                        let subtree = zipper.focus();
                        match zipper.cache.get(subtree, level) {
                            CacheResult::Terminal(_clean_level) => {
                                debug!("Cache Hit - Nothing Applicable");
                                zipper.trigger_cache_hit();
                                if zipper.get_next().is_none() {
                                    break;
                                }
                                continue;
                            }
                            CacheResult::Rewrite(cached) => {
                                debug!("Cache Hit");
                                zipper.trigger_cache_hit();
                                zipper.replace_focus(cached);
                                zipper.map_ancestors_to_root(level);
                                continue 'main;
                            }
                            _ => {
                                zipper.trigger_cache_miss();
                            }
                        };
                    }

                    let (subtree, meta) = zipper.focus_and_meta();
                    let id = self.rule_groups.discriminant_fn.map(|f| f(subtree));
                    let rules = self.rule_groups.get_rules(level, id);
                    debug!("Checking Level {} with {} Rules", level, rules.len());
                    // Choose one transformation from all applicable rules at this level
                    let selected =
                        Self::select_rule(self.selector, self.parallel, subtree, meta, rules);

                    if let Some(selected) = selected {
                        if self.faster {
                            let stabilized = Self::apply_rule_naive_faster(
                                &mut zipper,
                                &self.event_handlers,
                                &self.rule_groups,
                                self.selector,
                                self.parallel,
                                selected,
                                level,
                            );
                            if !stabilized {
                                continue 'main;
                            }
                            // Stabilized — continue 'main to restart from root
                            continue 'main;
                        } else {
                            Self::apply_rule_naive(
                                &mut zipper,
                                &self.event_handlers,
                                selected,
                                level,
                            );
                        }
                        continue 'main;
                    } else {
                        debug!("Nothing Applicable");
                        if zipper.cache.is_active() {
                            let subtree = zipper.focus().clone();
                            zipper.cache.insert(&subtree, None, level);
                        }
                    }

                    if zipper.get_next().is_none() {
                        break;
                    }
                }
            }

            break;
        }

        info!("Finished Naive Morph");
        zipper.into_parts()
    }
}
