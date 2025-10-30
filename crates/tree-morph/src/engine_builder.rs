//! A builder type for constructing and configuring [`Engine`] instances.

use crate::engine::Engine;
use crate::events::EventHandlers;
use crate::helpers::{SelectorFn, select_first};
use crate::prelude::Rule;
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

    /// Register an event handler for the "enter" event.
    ///
    /// This event is triggered first on the root, and then whenever the engine moves down
    /// into a subtree. As a result, when a node is passed to rules, all nodes above it will have
    /// been passed to handlers for this event, in ascending order of their proximity to the root.
    pub fn add_on_enter(mut self, on_enter_fn: fn(&T, &mut M)) -> Self {
        self.event_handlers.add_on_enter(on_enter_fn);
        self
    }

    /// Register an event handler for the "exit" event.
    ///
    /// This event is triggered when the engine leaves a subtree.
    /// All nodes passed to "enter" event handlers will also be passed to "exit"
    /// event handlers in reverse order.
    ///
    /// This event is never triggered on the root node, since the engine never leaves its subtree.
    pub fn add_on_exit(mut self, on_exit_fn: fn(&T, &mut M)) -> Self {
        self.event_handlers.add_on_exit(on_exit_fn);
        self
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
