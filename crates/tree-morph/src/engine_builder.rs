//! A builder type for constructing and configuring [`Engine`] instances.

use crate::engine::Engine;
use crate::events::EventHandlers;
use crate::helpers::{SelectorFn, select_first};
use crate::prelude::Rule;

use paste::paste;
use uniplate::Uniplate;

/// A builder type for constructing and configuring [`Engine`] instances.
pub struct EngineBuilder<T, M, R>
where
    T: Uniplate,
    R: Rule<T, M>,
{
    event_handlers: EventHandlers<T, M>,

    /// Groups of rules, each with a selector function.
    rule_groups: Vec<Vec<R>>,

    selector: SelectorFn<T, M, R>,
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

impl<T, M, R> EngineBuilder<T, M, R>
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
        }
    }

    /// Consumes the builder and returns the constructed [`Engine`] instance.
    pub fn build(self) -> Engine<T, M, R> {
        Engine {
            event_handlers: self.event_handlers,
            rule_groups: self.rule_groups,
            selector: self.selector,
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

    /// Sets the selector function to be used when multiple rules are applicable to the same node.
    ///
    /// See the [`morph`](Engine::morph) method of the Engine type for more information.
    pub fn set_selector(mut self, selector: SelectorFn<T, M, R>) -> Self {
        self.selector = selector;
        self
    }
}

impl<T, M, R> Default for EngineBuilder<T, M, R>
where
    T: Uniplate,
    R: Rule<T, M>,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T, M, R> From<EngineBuilder<T, M, R>> for Engine<T, M, R>
where
    T: Uniplate,
    R: Rule<T, M>,
{
    fn from(val: EngineBuilder<T, M, R>) -> Self {
        val.build()
    }
}
