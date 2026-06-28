use std::collections::VecDeque;
use std::sync::atomic::Ordering;

use uniplate::Uniplate;

use super::Expression;

const NO_CLEAN_RULE_PRIORITY: u16 = u16::MAX;

/// Stable handle for an expression node stored in an [`ExpressionArena`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExpressionNodeId(usize);

/// Arena-backed representation of an [`Expression`] tree.
///
/// The arena keeps direct parent/child links so callers can jump to known nodes without walking
/// down from the root each time. Subtree replacement preserves the replaced node's id and appends
/// the replacement descendants; descendants of the old subtree become unreachable from the root.
#[derive(Clone, Debug)]
pub struct ExpressionArena {
    nodes: Vec<ExpressionArenaNode>,
    root: ExpressionNodeId,
}

#[derive(Clone, Debug)]
struct ExpressionArenaNode {
    expr: Expression,
    parent: Option<ExpressionNodeId>,
    children: Vec<ExpressionNodeId>,
    clean_rule_priority: u16,
    cached_content_hash: Option<u64>,
}

impl ExpressionArena {
    /// Builds an arena from an expression tree.
    pub fn from_root(root: Expression) -> Self {
        let mut arena = Self {
            nodes: Vec::new(),
            root: ExpressionNodeId(0),
        };
        arena.root = arena.push_subtree(root, None);
        arena
    }

    /// Returns the root node id.
    pub fn root(&self) -> ExpressionNodeId {
        self.root
    }

    /// Returns the expression payload stored at `id`.
    pub fn expression(&self, id: ExpressionNodeId) -> &Expression {
        &self.node(id).expr
    }

    /// Returns the mutable expression payload stored at `id`.
    ///
    /// Callers that change the number or order of direct expression children should use
    /// [`ExpressionArena::replace_subtree`] instead so the arena links stay consistent.
    pub fn expression_mut(&mut self, id: ExpressionNodeId) -> &mut Expression {
        self.invalidate_content_hash_to_root(id);
        &mut self.node_mut(id).expr
    }

    /// Returns the cached content hash for `id`.
    ///
    /// Hashes are computed incrementally from arena child node hashes rather than by walking
    /// embedded expression subtrees in each payload.
    pub fn content_hash(&mut self, id: ExpressionNodeId) -> u64 {
        self.content_hash_with_cache_status(id).0
    }

    /// Returns the cached content hash for `id` and whether it was already cached.
    pub fn content_hash_with_cache_status(&mut self, id: ExpressionNodeId) -> (u64, bool) {
        if let Some(hash) = self.node(id).cached_content_hash {
            return (hash, true);
        }

        let child_ids: Vec<ExpressionNodeId> = self.children(id).to_vec();
        let child_hashes: Vec<u64> = child_ids
            .iter()
            .map(|&child_id| self.content_hash(child_id))
            .collect();
        let hash = self
            .expression(id)
            .content_hash_from_child_hashes(&mut child_hashes.into_iter());
        self.node_mut(id).cached_content_hash = Some(hash);
        (hash, false)
    }

    /// Returns the parent of `id`, or `None` for the root.
    pub fn parent(&self, id: ExpressionNodeId) -> Option<ExpressionNodeId> {
        self.node(id).parent
    }

    /// Returns the direct expression children of `id`.
    pub fn children(&self, id: ExpressionNodeId) -> &[ExpressionNodeId] {
        &self.node(id).children
    }

    /// Records that rules at `priority` and higher have been attempted and failed for `id`.
    pub fn mark_clean_for_rule_priority(&mut self, id: ExpressionNodeId, priority: u16) {
        let node = self.node_mut(id);
        node.clean_rule_priority = node.clean_rule_priority.min(priority);
    }

    /// Returns whether `id` is known clean for the given rule priority.
    pub fn is_clean_for_rule_priority(&self, id: ExpressionNodeId, priority: u16) -> bool {
        priority >= self.node(id).clean_rule_priority
    }

    /// Returns the highest-priority clean marker currently stored on `id`.
    pub fn clean_rule_priority(&self, id: ExpressionNodeId) -> u16 {
        self.node(id).clean_rule_priority
    }

    /// Clears any clean-rule marker on `id`.
    pub fn clear_clean_rule_priority(&mut self, id: ExpressionNodeId) {
        self.node_mut(id).clean_rule_priority = NO_CLEAN_RULE_PRIORITY;
    }

    /// Replaces the subtree at `id` while preserving `id` itself.
    pub fn replace_subtree(&mut self, id: ExpressionNodeId, replacement: Expression) {
        self.assert_valid_id(id);

        let children = replacement
            .children()
            .into_iter()
            .map(|child| self.push_subtree(child, Some(id)))
            .collect();

        let node = self.node_mut(id);
        node.expr = replacement;
        node.children = children;
        node.clean_rule_priority = NO_CLEAN_RULE_PRIORITY;
        node.cached_content_hash = None;
    }

    /// Appends top-level constraints to the root expression.
    pub fn add_root_children(&mut self, children: Vec<Expression>) {
        let root = self.root;
        let Expression::Root(metadata, _) = self.expression(root) else {
            panic!("arena root is not an Expression::Root");
        };
        let metadata = metadata.clone();

        let new_children = children
            .into_iter()
            .map(|child| self.push_subtree(child, Some(root)))
            .collect::<Vec<_>>();
        self.node_mut(root).children.extend(new_children);

        let rebuilt_children = self
            .children(root)
            .iter()
            .map(|child| self.direct_child_expression(*child))
            .collect();
        let root_expr = Expression::Root(metadata, rebuilt_children);
        self.node_mut(root).expr = root_expr;
        self.invalidate_content_hash_to_root(root);
        self.clear_clean_rule_priority(root);
    }

    /// Rebuilds the stored expression payload at `id` from its direct arena children.
    ///
    /// Only one tree level is materialised here; each child subtree is rebuilt from its own
    /// arena links. Ancestor repair should walk upward so deeper nodes are refreshed first.
    pub fn rebuild_payload_from_children(&mut self, id: ExpressionNodeId) {
        let child_exprs: VecDeque<Expression> = self
            .children(id)
            .iter()
            .map(|&child_id| self.direct_child_expression(child_id))
            .collect();
        let clean_rule_priority = self.node(id).clean_rule_priority;
        let rebuilt = self.node(id).expr.with_children(child_exprs);
        rebuilt.meta_ref().clear_clean_rule_priority();
        if clean_rule_priority != NO_CLEAN_RULE_PRIORITY {
            rebuilt
                .meta_ref()
                .mark_clean_for_rule_priority(clean_rule_priority);
        }
        let node = self.node_mut(id);
        node.expr = rebuilt;
        node.cached_content_hash = None;
    }

    /// Clears cached arena content hashes from `id` through the root.
    pub fn invalidate_content_hash_to_root(&mut self, id: ExpressionNodeId) {
        let mut node = Some(id);
        while let Some(node_id) = node {
            self.node_mut(node_id).cached_content_hash = None;
            node = self.parent(node_id);
        }
    }

    /// Rebuilds the expression tree reachable from the arena root.
    pub fn into_root_expression(self) -> Expression {
        self.expression_from(self.root)
    }

    fn direct_child_expression(&self, id: ExpressionNodeId) -> Expression {
        self.expression(id).clone()
    }

    /// Rebuilds the expression tree reachable from `id`.
    pub fn expression_from(&self, id: ExpressionNodeId) -> Expression {
        let node = self.node(id);
        let children = node
            .children
            .iter()
            .map(|child| self.expression_from(*child))
            .collect::<VecDeque<_>>();

        let rebuilt = node.expr.with_children(children);
        rebuilt.meta_ref().clear_clean_rule_priority();
        if node.clean_rule_priority != NO_CLEAN_RULE_PRIORITY {
            rebuilt
                .meta_ref()
                .mark_clean_for_rule_priority(node.clean_rule_priority);
        }
        rebuilt.invalidate_cached_content_hash();
        rebuilt
    }

    /// Returns the number of slots currently allocated in the arena.
    ///
    /// This includes unreachable slots left behind by subtree replacement.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns true when the arena contains no nodes.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    fn push_subtree(
        &mut self,
        expr: Expression,
        parent: Option<ExpressionNodeId>,
    ) -> ExpressionNodeId {
        let id = ExpressionNodeId(self.nodes.len());
        let child_exprs = expr.children();
        let clean_rule_priority = expr.meta_ref().clean_rule_priority.load(Ordering::Relaxed);

        self.nodes.push(ExpressionArenaNode {
            expr,
            parent,
            children: Vec::new(),
            clean_rule_priority,
            cached_content_hash: None,
        });

        let children = child_exprs
            .into_iter()
            .map(|child| self.push_subtree(child, Some(id)))
            .collect();
        self.nodes[id.0].children = children;

        id
    }

    fn node(&self, id: ExpressionNodeId) -> &ExpressionArenaNode {
        self.nodes
            .get(id.0)
            .unwrap_or_else(|| panic!("invalid expression node id: {id:?}"))
    }

    fn node_mut(&mut self, id: ExpressionNodeId) -> &mut ExpressionArenaNode {
        self.nodes
            .get_mut(id.0)
            .unwrap_or_else(|| panic!("invalid expression node id: {id:?}"))
    }

    fn assert_valid_id(&self, id: ExpressionNodeId) {
        let _ = self.node(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Metadata, Moo};

    fn int(value: i32) -> Expression {
        value.into()
    }

    fn eq(left: Expression, right: Expression) -> Expression {
        Expression::Eq(Metadata::new(), Moo::new(left), Moo::new(right))
    }

    fn root(exprs: Vec<Expression>) -> Expression {
        Expression::Root(Metadata::new(), exprs)
    }

    #[test]
    fn round_trips_expression_tree() {
        let expr = eq(int(1), eq(int(2), int(3)));
        let arena = ExpressionArena::from_root(expr.clone());

        assert_eq!(arena.into_root_expression(), expr);
    }

    #[test]
    fn records_parent_and_child_links() {
        let arena = ExpressionArena::from_root(eq(int(1), int(2)));
        let root = arena.root();
        let children = arena.children(root);

        assert_eq!(children.len(), 2);
        assert_eq!(arena.parent(root), None);
        assert_eq!(arena.parent(children[0]), Some(root));
        assert_eq!(arena.parent(children[1]), Some(root));
    }

    #[test]
    fn replaces_subtree_without_changing_replaced_node_id() {
        let mut arena = ExpressionArena::from_root(eq(int(1), int(2)));
        let root = arena.root();
        let left = arena.children(root)[0];

        arena.replace_subtree(left, eq(int(3), int(4)));

        assert_eq!(arena.children(root)[0], left);
        assert_eq!(arena.parent(arena.children(left)[0]), Some(left));
        assert_eq!(arena.into_root_expression(), eq(eq(int(3), int(4)), int(2)));
    }

    #[test]
    fn rebuilds_ancestor_payload_after_child_replacement() {
        let mut arena = ExpressionArena::from_root(root(vec![eq(int(1), int(2))]));
        let root_id = arena.root();
        let eq_id = arena.children(root_id)[0];
        let left = arena.children(eq_id)[0];

        arena.replace_subtree(left, int(3));
        arena.rebuild_payload_from_children(eq_id);
        arena.rebuild_payload_from_children(root_id);

        assert_eq!(arena.expression(eq_id), &eq(int(3), int(2)));
        assert_eq!(arena.expression(root_id), &root(vec![eq(int(3), int(2))]));
    }

    #[test]
    fn invalidates_content_hashes_on_changed_path() {
        let mut arena = ExpressionArena::from_root(root(vec![eq(int(1), int(2))]));
        let root_id = arena.root();
        let eq_id = arena.children(root_id)[0];
        let left = arena.children(eq_id)[0];

        let old_root_hash = arena.content_hash(root_id);
        let old_eq_hash = arena.content_hash(eq_id);

        arena.replace_subtree(left, int(3));
        arena.rebuild_payload_from_children(eq_id);
        arena.rebuild_payload_from_children(root_id);

        assert_ne!(arena.content_hash(eq_id), old_eq_hash);
        assert_ne!(arena.content_hash(root_id), old_root_hash);
    }

    #[test]
    fn clean_rule_state_is_arena_side_and_exported() {
        let mut arena = ExpressionArena::from_root(eq(int(1), int(2)));
        let root = arena.root();

        arena.mark_clean_for_rule_priority(root, 5);

        assert!(arena.is_clean_for_rule_priority(root, 5));
        assert!(
            arena
                .into_root_expression()
                .meta_ref()
                .is_clean_for_rule_priority(5)
        );
    }

    #[test]
    fn replacing_subtree_clears_arena_clean_rule_state() {
        let mut arena = ExpressionArena::from_root(eq(int(1), int(2)));
        let root = arena.root();

        arena.mark_clean_for_rule_priority(root, 5);
        arena.replace_subtree(root, eq(int(3), int(4)));

        assert!(!arena.is_clean_for_rule_priority(root, 5));
    }

    #[test]
    fn arena_content_hash_matches_expression_hash() {
        let expr = root(vec![eq(eq(int(1), int(2)), eq(int(3), int(4)))]);
        let expected = expr.cached_content_hash();

        let mut arena = ExpressionArena::from_root(expr);
        let root_id = arena.root();
        assert_eq!(arena.content_hash(root_id), expected);

        let eq_id = arena.children(root_id)[0];
        assert_eq!(
            arena.content_hash(eq_id),
            arena.expression(eq_id).cached_content_hash()
        );
    }

    #[test]
    fn incremental_content_hash_survives_ancestor_rebuild() {
        let mut arena = ExpressionArena::from_root(root(vec![eq(int(1), int(2))]));
        let root_id = arena.root();
        let eq_id = arena.children(root_id)[0];
        let left = arena.children(eq_id)[0];

        let hash_before = arena.content_hash(root_id);

        arena.replace_subtree(left, int(3));
        arena.rebuild_payload_from_children(eq_id);
        arena.rebuild_payload_from_children(root_id);

        assert_eq!(arena.expression(eq_id), &eq(int(3), int(2)));
        assert_ne!(arena.content_hash(root_id), hash_before);
        assert_eq!(
            arena.content_hash(root_id),
            arena.expression(root_id).cached_content_hash()
        );
    }

    #[test]
    fn appends_root_children() {
        let mut arena = ExpressionArena::from_root(root(vec![int(1)]));

        arena.add_root_children(vec![int(2), int(3)]);

        assert_eq!(
            arena.into_root_expression(),
            root(vec![int(1), int(2), int(3)])
        );
    }
}
