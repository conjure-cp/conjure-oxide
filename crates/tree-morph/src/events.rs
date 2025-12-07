use paste::paste;
use uniplate::Uniplate;

macro_rules! event_handlers {
    (
        directions: [$($dir:ident),*]
    ) => {

        paste! {
        pub(crate) struct EventHandlers<T, M, R> {
            $(
                [<before_ $dir>]: Vec<fn(&T, &mut M)>,
                [<after_ $dir>]: Vec<fn(&T, &mut M)>,
            )*
            before_rule: Vec<fn(&T, &mut M, &R)>,
            after_rule: Vec<fn(&T, &mut M, &R, bool)>,
            after_apply: Vec<fn(&T, &mut M, &R)>,
        }

        impl<T: Uniplate, M, R> EventHandlers<T, M, R> {
            pub(crate) fn new() -> Self {
                Self {
                    $(
                        [<before_ $dir>]: vec![],
                        [<after_ $dir>]: vec![],
                    )*
                    before_rule: vec![],
                    after_rule: vec![],
                    after_apply: vec![],
                }
            }

            $(
                pub(crate) fn [<trigger_before_ $dir>](&self, node: &T, meta: &mut M) {
                    for f in &self.[<before_ $dir>] {
                        f(node, meta)
                    }
                }
                pub(crate) fn [<trigger_after_ $dir>](&self, node: &T, meta: &mut M) {
                    for f in &self.[<after_ $dir>] {
                        f(node, meta)
                    }
                }
                pub(crate) fn [<add_before_ $dir>](&mut self, handler: fn(&T, &mut M)) {
                    self.[<before_ $dir>].push(handler);
                }
                pub(crate) fn [<add_after_ $dir>](&mut self, handler: fn(&T, &mut M)) {
                    self.[<after_ $dir>].push(handler);
                }
            )*

            pub(crate) fn trigger_before_rule(&self, node: &T, meta: &mut M, rule: &R) {
                for f in &self.before_rule {
                    f(node, meta, rule)
                }
            }

            pub(crate) fn trigger_after_rule(&self, node: &T, meta: &mut M, rule: &R, applicable: bool) {
                for f in &self.after_rule {
                    f(node, meta, rule, applicable)
                }
            }

            pub(crate) fn trigger_on_apply(&self, node: &T, meta: &mut M, rule: &R) {
                for f in &self.after_apply {
                    f(node, meta, rule)
                }
            }

            pub(crate) fn add_before_rule(&mut self, handler: fn(&T, &mut M, &R)) {
                self.before_rule.push(handler);
            }

            pub(crate) fn add_after_rule(&mut self, handler: fn(&T, &mut M, &R, bool)) {
                self.after_rule.push(handler);
            }

            pub(crate) fn add_after_apply(&mut self, handler: fn(&T, &mut M, &R)) {
                self.after_apply.push(handler);
            }
        }
    }};
}

// We don't need event handlers for "left" since we never move left
event_handlers! {
    directions: [up, down, right]
}
