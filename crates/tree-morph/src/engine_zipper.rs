//! Define the EngineNodes and the EngineZipper

use tracing::{instrument, trace};
use uniplate::{Uniplate, tagged_zipper::TaggedZipper, zipper::Zipper};

use crate::{cache::RewriteCache, events::EventHandlers, rule::Rule};

#[derive(Debug, Clone)]
pub(crate) struct EngineNodeState {
    /// Rule groups with lower indices have already been applied without change.
    /// For a level `n`, a state is 'dirty' if and only if `n >= dirty_from`.
    dirty_from: usize,
    descend_anyway: bool,
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
        Self {
            dirty_from: 0,
            descend_anyway: false,
        }
    }
}

/// A Zipper with optimisations for tree transformation.
pub(crate) struct EngineZipper<'a, T, M, R, C>
where
    T: Uniplate,
    R: Rule<T, M>,
    C: RewriteCache<T>,
{
    inner: TaggedZipper<T, EngineNodeState, fn(&T) -> EngineNodeState>,
    event_handlers: &'a EventHandlers<T, M, R>,
    pub(crate) cache: &'a mut C,
    pub(crate) meta: M,
}

impl<'a, T, M, R, C> EngineZipper<'a, T, M, R, C>
where
    T: Uniplate,
    R: Rule<T, M>,
    C: RewriteCache<T>,
{
    pub fn new(
        tree: T,
        meta: M,
        event_handlers: &'a EventHandlers<T, M, R>,
        cache: &'a mut C,
    ) -> Self {
        EngineZipper {
            inner: TaggedZipper::new(tree, EngineNodeState::new),
            event_handlers,
            cache,
            meta,
        }
    }

    /// Returns a reference to the currently focused node.
    pub fn focus(&self) -> &T {
        self.inner.focus()
    }

    /// Returns a reference to the focused node and a mutable reference to the metadata.
    /// This avoids borrow conflicts when both are needed simultaneously.
    pub fn focus_and_meta(&mut self) -> (&T, &mut M) {
        (self.inner.focus(), &mut self.meta)
    }

    /// Replaces the currently focused node with `replacement`.
    pub fn replace_focus(&mut self, replacement: T) {
        self.inner.replace_focus(replacement);
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

    fn go_down(&mut self) -> Option<()> {
        self.cache.push_ancestor(self.inner.focus());
        if self.inner.go_down().is_none() {
            self.cache.pop_ancestor(); // undo speculative push
            return None;
        }
        trace!("Go down");
        self.event_handlers
            .trigger_after_down(self.inner.focus(), &mut self.meta);
        Some(())
    }

    fn go_up(&mut self) -> Option<()> {
        if !self.inner.zipper().has_up() {
            return None;
        }
        self.event_handlers
            .trigger_before_up(self.inner.focus(), &mut self.meta);
        self.inner.go_up().expect("checked above");
        self.cache.pop_ancestor();
        trace!("Go up");
        self.event_handlers
            .trigger_after_up(self.inner.focus(), &mut self.meta);
        Some(())
    }

    fn go_right(&mut self) -> Option<()> {
        if !self.inner.zipper().has_right() {
            return None;
        }
        self.event_handlers
            .trigger_before_right(self.inner.focus(), &mut self.meta);
        self.inner.go_right().expect("checked above");
        trace!("Go right");
        self.event_handlers
            .trigger_after_right(self.inner.focus(), &mut self.meta);
        Some(())
    }

    /// Trigger cache hit event handlers.
    pub fn trigger_cache_hit(&mut self) {
        self.event_handlers
            .trigger_on_cache_hit(self.inner.focus(), &mut self.meta);
    }

    /// Trigger cache miss event handlers.
    pub fn trigger_cache_miss(&mut self) {
        self.event_handlers
            .trigger_on_cache_miss(self.inner.focus(), &mut self.meta);
    }

    /// Mark the current focus as visited at the given level.
    /// Calling `go_next_dirty` with the same level will no longer yield this node.
    pub fn set_dirty_from(&mut self, level: usize) {
        trace!("Setting level = {}", level);
        self.inner.tag_mut().set_dirty_from(level);
    }

    /// Mark ancestors as dirty for all levels, and return to the root.
    /// Pops ancestor hashes and inserts old→new ancestor mappings into the cache.
    pub fn mark_dirty_to_root(&mut self, level: usize) {
        trace!("Marking Dirty to Root");
        self.set_dirty_from(0);
        self.cache.invalidate_node(self.inner.focus());
        while self.inner.zipper().has_up() {
            self.event_handlers
                .trigger_before_up(self.inner.focus(), &mut self.meta);
            self.inner.go_up().expect("checked above");
            self.set_dirty_from(0);
            self.cache.invalidate_node(self.inner.focus());
            self.cache.pop_and_map_ancestor(self.inner.focus(), level);
            trace!("Go up (mark dirty)");
            self.event_handlers
                .trigger_after_up(self.inner.focus(), &mut self.meta);
        }
    }
}

impl<T, M, R, C> From<EngineZipper<'_, T, M, R, C>> for (T, M)
where
    T: Uniplate,
    R: Rule<T, M>,
    C: RewriteCache<T>,
{
    fn from(val: EngineZipper<'_, T, M, R, C>) -> Self {
        let meta = val.meta;
        let tree = val.inner.rebuild_root();
        (tree, meta)
    }
}

/// A Naive Zipper. For testing, debugging and benching
pub(crate) struct NaiveZipper<'a, T, M, R, C>
where
    T: Uniplate,
    R: Rule<T, M>,
    C: RewriteCache<T>,
{
    inner: Zipper<T>,
    event_handlers: &'a EventHandlers<T, M, R>,
    pub(crate) cache: &'a mut C,
    pub(crate) meta: M,
}

impl<'a, T, M, R, C> NaiveZipper<'a, T, M, R, C>
where
    T: Uniplate,
    R: Rule<T, M>,
    C: RewriteCache<T>,
{
    pub fn new(
        tree: T,
        meta: M,
        event_handlers: &'a EventHandlers<T, M, R>,
        cache: &'a mut C,
    ) -> Self {
        NaiveZipper {
            inner: Zipper::new(tree),
            event_handlers,
            cache,
            meta,
        }
    }

    /// Returns a reference to the currently focused node.
    pub fn focus(&self) -> &T {
        self.inner.focus()
    }

    /// Returns a reference to the focused node and a mutable reference to the metadata.
    pub fn focus_and_meta(&mut self) -> (&T, &mut M) {
        (self.inner.focus(), &mut self.meta)
    }

    /// Replaces the currently focused node with `replacement`.
    pub fn replace_focus(&mut self, replacement: T) {
        self.inner.replace_focus(replacement);
    }

    /// Trigger cache hit event handlers.
    pub fn trigger_cache_hit(&mut self) {
        self.event_handlers
            .trigger_on_cache_hit(self.inner.focus(), &mut self.meta);
    }

    /// Trigger cache miss event handlers.
    pub fn trigger_cache_miss(&mut self) {
        self.event_handlers
            .trigger_on_cache_miss(self.inner.focus(), &mut self.meta);
    }

    /// Consumes the zipper and returns the reconstructed root and metadata.
    pub fn into_parts(self) -> (T, M) {
        (self.inner.rebuild_root(), self.meta)
    }

    pub fn get_next(&mut self) -> Option<()> {
        // Try going down — speculative push, undo on failure
        self.cache.push_ancestor(self.inner.focus());
        if self.inner.go_down().is_some() {
            return Some(());
        }
        self.cache.pop_ancestor();
        if self.inner.go_right().is_some() {
            return Some(());
        }
        while self.inner.go_up().is_some() {
            self.cache.pop_ancestor();
            if self.inner.go_right().is_some() {
                return Some(());
            }
        }
        None
    }

    fn go_up(&mut self) -> Option<()> {
        if !self.inner.has_up() {
            return None;
        }
        self.event_handlers
            .trigger_before_up(self.inner.focus(), &mut self.meta);
        self.inner.go_up().expect("checked above");
        self.cache.pop_ancestor();
        trace!("Go up");
        self.event_handlers
            .trigger_after_up(self.inner.focus(), &mut self.meta);
        Some(())
    }

    pub fn go_to_root(&mut self) {
        while self.go_up().is_some() {}
    }

    /// Walk back to root, inserting ancestor mappings at the given level.
    pub fn map_ancestors_to_root(&mut self, level: usize) {
        while self.inner.has_up() {
            self.event_handlers
                .trigger_before_up(self.inner.focus(), &mut self.meta);
            self.inner.go_up().expect("checked above");
            self.cache.invalidate_node(self.inner.focus());
            self.cache.pop_and_map_ancestor(self.inner.focus(), level);
            trace!("Go up (map ancestor)");
            self.event_handlers
                .trigger_after_up(self.inner.focus(), &mut self.meta);
        }
    }
}
