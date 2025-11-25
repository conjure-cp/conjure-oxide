use paste::paste;
use uniplate::Uniplate;

macro_rules! event_handlers {
    (
        directions: [$($dir:ident),*]
    ) => {

        paste! {
        pub(crate) struct EventHandlers<T, M> {
            $(
                [<before_ $dir>]: Vec<fn(&T, &mut M)>,
                [<after_ $dir>]: Vec<fn(&T, &mut M)>,
            )*
        }

        impl<T: Uniplate, M> EventHandlers<T, M> {
            pub(crate) fn new() -> Self {
                Self {
                    $(
                        [<before_ $dir>]: vec![],
                        [<after_ $dir>]: vec![],
                    )*
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
        }
    }};
}

// We don't need event handlers for "left" since we never move left
event_handlers! {
    directions: [up, down, right]
}
