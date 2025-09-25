use crate::engine::Engine;
use crate::events::EventHandlers;
use crate::helpers::{SelectorFn, select_first};
use crate::prelude::Rule;
use uniplate::Uniplate;

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
    pub fn new() -> Self {
        EngineBuilder {
            event_handlers: EventHandlers::new(),
            rule_groups: Vec::new(),
            selector: select_first,
        }
    }

    pub fn build(self) -> Engine<T, M, R> {
        Engine {
            event_handlers: self.event_handlers,
            rule_groups: self.rule_groups,
            selector: self.selector,
        }
    }

    pub fn add_rule_group(mut self, rules: Vec<R>) -> Self {
        self.rule_groups.push(rules);
        self
    }

    pub fn add_rule(self, rule: R) -> Self {
        self.add_rule_group(vec![rule])
    }

    pub fn append_rule_groups(mut self, groups: Vec<Vec<R>>) -> Self {
        self.rule_groups.extend(groups.into_iter());
        self
    }

    pub fn add_on_enter(mut self, on_enter_fn: fn(&T, &mut M)) -> Self {
        self.event_handlers.add_on_enter(on_enter_fn);
        self
    }

    pub fn add_on_exit(mut self, on_exit_fn: fn(&T, &mut M)) -> Self {
        self.event_handlers.add_on_exit(on_exit_fn);
        self
    }

    pub fn set_selector(mut self, selector: SelectorFn<T, M, R>) -> Self {
        self.selector = selector;
        self
    }
}

impl<T, M, R> Into<Engine<T, M, R>> for EngineBuilder<T, M, R>
where
    T: Uniplate,
    R: Rule<T, M>,
{
    fn into(self) -> Engine<T, M, R> {
        self.build()
    }
}
