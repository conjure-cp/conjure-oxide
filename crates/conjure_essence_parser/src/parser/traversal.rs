use std::collections::VecDeque;

use tree_sitter::{Node, TreeCursor};

/// An iterator that traverses the syntax tree in pre-order DFS order.
pub struct WalkDFS<'a> {
    cursor: Option<TreeCursor<'a>>,
    retract: Option<&'a dyn Fn(&Node<'a>) -> bool>,
}

#[allow(dead_code)]
impl<'a> WalkDFS<'a> {
    pub fn new(node: &'a Node<'a>) -> Self {
        Self {
            cursor: Some(node.walk()),
            retract: None,
        }
    }

    pub fn with_retract(node: &'a Node<'a>, retract: &'a dyn Fn(&Node<'a>) -> bool) -> Self {
        Self {
            cursor: Some(node.walk()),
            retract: Some(retract),
        }
    }
}

impl<'a> Iterator for WalkDFS<'a> {
    type Item = Node<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let cursor = self.cursor.as_mut()?;
        let node = cursor.node();

        if self.retract.is_none() || !self.retract.as_ref().unwrap()(&node) {
            // Try to descend into the first child.
            if cursor.goto_first_child() {
                return Some(node);
            }
        }

        // If we are at a leaf, try its next sibling instead.
        if cursor.goto_next_sibling() {
            return Some(node);
        }

        // If neither has worked, we need to ascend until we can go to a sibling
        loop {
            // If we can't go to the parent, then that means we've reached the root, and our
            // iterator will be done in the next iteration
            if !cursor.goto_parent() {
                self.cursor = None;
                break;
            }

            // If we get to a sibling, then this will be the first time we touch that node,
            // so it'll be the next starting node
            if cursor.goto_next_sibling() {
                break;
            }
        }

        Some(node)
    }
}

/// An iterator that traverses the syntax tree in breadth-first order.
pub struct WalkBFS<'a> {
    queue: VecDeque<Node<'a>>,
}

#[allow(dead_code)]
impl<'a> WalkBFS<'a> {
    pub fn new(root: &'a Node<'a>) -> Self {
        Self {
            queue: VecDeque::from([*root]),
        }
    }
}

impl<'a> Iterator for WalkBFS<'a> {
    type Item = Node<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.queue.pop_front()?;
        node.children(&mut node.walk()).for_each(|child| {
            self.queue.push_back(child);
        });
        Some(node)
    }
}

#[cfg(test)]
mod test {
    use super::super::util::get_tree;
    use super::*;

    #[test]
    pub fn test_bfs() {
        let (tree, _) = get_tree("such that x, 5").unwrap();
        let root = tree.root_node();
        let mut iter = WalkBFS::new(&root).filter(|n| n.is_named());
        assert_eq!(iter.next().unwrap().kind(), "program"); //         depth = 0
        assert_eq!(iter.next().unwrap().kind(), "constraint_list"); // depth = 1
        assert_eq!(iter.next().unwrap().kind(), "expression"); //      depth = 2
        assert_eq!(iter.next().unwrap().kind(), "expression");
        assert_eq!(iter.next().unwrap().kind(), "variable"); //        depth = 3
        assert_eq!(iter.next().unwrap().kind(), "constant");
        assert_eq!(iter.next().unwrap().kind(), "integer"); //         depth = 4
    }

    #[test]
    pub fn test_dfs() {
        let (tree, _) = get_tree("such that x, 5").unwrap();
        let root = tree.root_node();
        let mut iter = WalkDFS::new(&root).filter(|n| n.is_named());
        assert_eq!(iter.next().unwrap().kind(), "program"); //         top level
        assert_eq!(iter.next().unwrap().kind(), "constraint_list");
        assert_eq!(iter.next().unwrap().kind(), "expression"); //      first branch ("x")
        assert_eq!(iter.next().unwrap().kind(), "variable");
        assert_eq!(iter.next().unwrap().kind(), "expression"); //      second branch ("5")
        assert_eq!(iter.next().unwrap().kind(), "constant");
        assert_eq!(iter.next().unwrap().kind(), "integer");
    }

    #[test]
    pub fn test_dfs_retract() {
        let (tree, _) = get_tree("such that x / 42, 5 + y").unwrap();
        let root = tree.root_node();
        let mut iter = WalkDFS::with_retract(&root, &|n: &Node<'_>| n.kind() == "expression")
            .filter(|n| n.is_named());
        assert_eq!(iter.next().unwrap().kind(), "program"); //         top level
        assert_eq!(iter.next().unwrap().kind(), "constraint_list");
        assert_eq!(iter.next().unwrap().kind(), "expression"); //      first branch ("x / 42"). Don't descend into subexpressions.
        assert_eq!(iter.next().unwrap().kind(), "expression"); //      second branch ("5 + y"). Don't descend into subexpressions.
    }
}
