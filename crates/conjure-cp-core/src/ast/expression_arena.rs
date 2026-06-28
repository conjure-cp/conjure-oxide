use std::collections::VecDeque;

use uniplate::Uniplate;

use super::Expression;

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
        self.node_mut(id).expr.invalidate_cached_content_hash();
        &mut self.node_mut(id).expr
    }

    /// Returns the parent of `id`, or `None` for the root.
    pub fn parent(&self, id: ExpressionNodeId) -> Option<ExpressionNodeId> {
        self.node(id).parent
    }

    /// Returns the direct expression children of `id`.
    pub fn children(&self, id: ExpressionNodeId) -> &[ExpressionNodeId] {
        &self.node(id).children
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
        node.expr.invalidate_cached_content_hash();
        node.children = children;
    }

    /// Rebuilds the expression tree reachable from the arena root.
    pub fn into_root_expression(self) -> Expression {
        self.expression_from(self.root)
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

        self.nodes.push(ExpressionArenaNode {
            expr,
            parent,
            children: Vec::new(),
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
}
