//! Define the EngineNodes and the EngineZiper

use paste::paste;
use tracing::{instrument, trace};
use uniplate::{Uniplate, tagged_zipper::TaggedZipper, zipper::Zipper};

use crate::{events::EventHandlers, rule::Rule};

#[derive(Debug, Clone)]
pub(crate) struct EngineNodeState {
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
                    trace!(concat!("Go ", stringify!($dir)));
                    self.event_handlers
                        .[<trigger_after_ $dir>](self.inner.focus(), &mut self.meta);
                })
            })*
        }
    };
}

/// A Zipper with optimisations for tree transformation.
pub(crate) struct EngineZipper<'events, T, M, R>
where
    T: Uniplate,
    R: Rule<T, M>,
{
    pub(crate) inner: TaggedZipper<T, EngineNodeState, fn(&T) -> EngineNodeState>,
    event_handlers: &'events EventHandlers<T, M, R>,
    pub(crate) meta: M,
}

impl<'events, T, M, R> EngineZipper<'events, T, M, R>
where
    T: Uniplate,
    R: Rule<T, M>,
{
    pub fn new(tree: T, meta: M, event_handlers: &'events EventHandlers<T, M, R>) -> Self {
        EngineZipper {
            inner: TaggedZipper::new(tree, EngineNodeState::new),
            event_handlers,
            meta,
        }
    }

    /// Go to the next node in the tree which is dirty for the given level.
    /// That node may be the current one if it is dirty.
    /// If no such node exists, go to the root and return `None`.
    #[instrument(skip(self))]
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
        trace!("Setting level = {}", level);
        self.inner.tag_mut().set_dirty_from(level);
    }

    /// Mark ancestors as dirty for all levels, and return to the root
    pub fn mark_dirty_to_root(&mut self) {
        trace!("Marking Dirty to Root");
        while self.go_up().is_some() {
            self.set_dirty_from(0);
        }
    }
}

impl<T, M, R> From<EngineZipper<'_, T, M, R>> for (T, M)
where
    T: Uniplate,
    R: Rule<T, M>,
{
    fn from(val: EngineZipper<'_, T, M, R>) -> Self {
        let meta = val.meta;
        let tree = val.inner.rebuild_root();
        (tree, meta)
    }
}

macro_rules! movement_fns_naive {
    (
        directions: [$($dir:ident),*]
    ) => {
        paste! {
            $(fn [<go_ $dir>](&mut self) -> Option<()> {
                self.inner.[<has_ $dir>]().then(|| {
                    self.event_handlers
                        .[<trigger_before_ $dir>](self.inner.focus(), &mut self.meta);
                    self.inner.[<go_ $dir>]().expect("zipper movement failed despite check");
                    trace!(concat!("Go ", stringify!($dir)));
                    self.event_handlers
                        .[<trigger_after_ $dir>](self.inner.focus(), &mut self.meta);
                })
            })*
        }
    };
}

/// A Naive Zipper. For testing, debugging and benching
pub(crate) struct NaiveZipper<'events, T, M, R>
where
    T: Uniplate,
    R: Rule<T, M>,
{
    pub(crate) inner: Zipper<T>,
    event_handlers: &'events EventHandlers<T, M, R>,
    pub(crate) meta: M,
}

impl<'events, T, M, R> NaiveZipper<'events, T, M, R>
where
    T: Uniplate,
    R: Rule<T, M>,
{
    pub fn new(tree: T, meta: M, event_handlers: &'events EventHandlers<T, M, R>) -> Self {
        NaiveZipper {
            inner: Zipper::new(tree),
            event_handlers,
            meta,
        }
    }

    pub fn get_next(&mut self) -> Option<()> {
        self.inner
            .go_down()
            .or_else(|| self.inner.go_right())
            .or_else(|| {
                while self.inner.go_up().is_some() {
                    if self.inner.go_right().is_some() {
                        return Some(());
                    }
                }
                None
            })
    }

    // We never move left in the tree
    movement_fns_naive! { directions: [up] }

    pub fn go_to_root(&mut self) {
        while self.go_up().is_some() {}
    }
}
