//! A builder type for constructing and configuring [`Engine`] instances.

use crate::cache::{NoCache, RewriteCache};
use crate::engine::Engine;
use crate::events::EventHandlers;
use crate::helpers::{SelectorFn, select_first};
use crate::prelude::Rule;

use paste::paste;
use uniplate::Uniplate;

/// A builder type for constructing and configuring [`Engine`] instances.
pub struct EngineBuilder<T, M, R, C>
where
    T: Uniplate,
    R: Rule<T, M>,
    C: RewriteCache<T>,
{
    event_handlers: EventHandlers<T, M, R>,

    /// Groups of rules, each with a selector function.
    rule_groups: Vec<Vec<R>>,

    selector: SelectorFn<T, M, R>,

    cache: C,

    movement_filter: fn(&T) -> bool,
}

macro_rules! add_handler_fns {
    (
        directions: [$($dir:ident),*]
    ) => {
        paste! {$(
            /// Register an event handler to be called before moving $dir in the tree.
            pub fn [<add_before_ $dir>](mut self, handler: fn(&T, &mut M)) -> Self {
                self.event_handlers.[<add_before_ $dir>](handler);
                self
            }

            /// Register an event handler to be called after moving $dir one node in the tree.
            pub fn [<add_after_ $dir>](mut self, handler: fn(&T, &mut M)) -> Self {
                self.event_handlers.[<add_after_ $dir>](handler);
                self
            }
        )*}
    };
}

impl<T, M, R> EngineBuilder<T, M, R, NoCache>
where
    T: Uniplate,
    R: Rule<T, M>,
{
    /// Creates a new builder instance with the default [`select_first`] selector.
    pub fn new() -> Self {
        EngineBuilder {
            event_handlers: EventHandlers::new(),
            rule_groups: Vec::new(),
            selector: select_first,
            cache: NoCache,
            movement_filter: |_| true,
        }
    }
}

impl<T, M, R, C> EngineBuilder<T, M, R, C>
where
    T: Uniplate,
    R: Rule<T, M>,
    C: RewriteCache<T>,
{
    /// Consumes the builder and returns the constructed [`Engine`] instance.
    pub fn build(self) -> Engine<T, M, R, C> {
        Engine {
            event_handlers: self.event_handlers,
            rule_groups: self.rule_groups,
            selector: self.selector,
            cache: self.cache,
            movement_filter: self.movement_filter
        }
    }

    /// Adds a collection of rules with the same priority.
    ///
    /// These rules will have a lower priority than previously added groups.
    pub fn add_rule_group(mut self, rules: Vec<R>) -> Self {
        self.rule_groups.push(rules);
        self
    }

    /// Adds a single rule in a group by itself.
    ///
    /// This is a special case of [`add_rule_group`](EngineBuilder::add_rule_group).
    pub fn add_rule(self, rule: R) -> Self {
        self.add_rule_group(vec![rule])
    }

    /// Adds a collection of rule groups to the existing one.
    ///
    /// Rule groups maintain the same order and will be lower priority than existing groups.
    pub fn append_rule_groups(mut self, groups: Vec<Vec<R>>) -> Self {
        self.rule_groups.extend(groups);
        self
    }

    add_handler_fns! {
        directions: [up, down, right]
    }

    /// Register an event handler to be called before attempting a rule
    pub fn add_before_rule(mut self, handler: fn(&T, &mut M, &R)) -> Self {
        self.event_handlers.add_before_rule(handler);
        self
    }

    /// Register an event handler to be called after attempting a rule
    /// The boolean signifies whether the rule is applicable
    pub fn add_after_rule(mut self, handler: fn(&T, &mut M, &R, bool)) -> Self {
        self.event_handlers.add_after_rule(handler);
        self
    }

    /// Register an event handler to be called after applying a rule
    pub fn add_after_apply(mut self, handler: fn(&T, &mut M, &R)) -> Self {
        self.event_handlers.add_after_apply(handler);
        self
    }

    /// Sets the selector function to be used when multiple rules are applicable to the same node.
    ///
    /// See the [`morph`](Engine::morph) method of the Engine type for more information.
    pub fn set_selector(mut self, selector: SelectorFn<T, M, R>) -> Self {
        self.selector = selector;
        self
    }

    /// Adds caching support to tree-morph.
    ///
    /// Recommended to use [`HashMapCache`] as it has a concrete
    /// implementation
    pub fn add_cacher<Cache: RewriteCache<T>>(
        self,
        cacher: Cache,
    ) -> EngineBuilder<T, M, R, Cache> {
        EngineBuilder {
            event_handlers: self.event_handlers,
            rule_groups: self.rule_groups,
            selector: self.selector,
            cache: cacher,
            movement_filter: self.movement_filter
        }
    }

    pub fn add_movement_filter(mut self, filter: fn(&T) -> bool) -> Self {
        self.movement_filter = filter;
        self
    }
}

impl<T, M, R> Default for EngineBuilder<T, M, R, NoCache>
where
    T: Uniplate,
    R: Rule<T, M>,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T, M, R, C> From<EngineBuilder<T, M, R, C>> for Engine<T, M, R, C>
where
    T: Uniplate,
    R: Rule<T, M>,
    C: RewriteCache<T>,
{
    fn from(val: EngineBuilder<T, M, R, C>) -> Self {
        val.build()
    }
}
