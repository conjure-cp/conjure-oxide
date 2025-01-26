// Copied and adapted from https://github.com/conjure-cp/uniplate/blob/nik/zippers/uniplate/src/zipper.rs

use std::sync::Arc;

use im::Vector;
use uniplate::{Tree, Uniplate};

use crate::helpers::tree_size;

/// Additional metadata associated with each tree node.
///
/// This lets us cache info about nodes of user-defined types,
/// and is used for engine optimisations.
#[derive(Debug, Clone)]
pub struct Meta {
    /// Transforms at and after this index should be applied.
    /// Those before it have already been tried with no change.
    clean_before: usize,

    /// The number of nodes in the associated subtree, including the root.
    subtree_size: usize,
}

impl Meta {
    /// Returns whether the associated node is clean up to the given transform index.
    ///
    /// A node is clean up to a given index iff all transforms before the index have
    /// been applied to all nodes in the subtree with no changes.
    pub fn is_clean(&self, index: usize) -> bool {
        index < self.clean_before
    }
}

/// A Zipper over `Uniplate` types, holding additional metadata information for each node.
#[derive(Clone)]
pub struct Zipper<T>
where
    T: Uniplate,
{
    /// The current node.
    focus: T,

    /// The path back to the top, immediate parent last.
    ///
    /// If empty, the focus is the top level node.
    path: Vec<PathSegment<T>>,

    /// A list of `Meta` objects in preorder traversal order.
    meta_list: Vec<Meta>,

    /// Points to the value in `meta_list` associated with the current focus.
    meta_index: usize,
}

#[derive(Clone)]
struct PathSegment<T>
where
    T: Uniplate,
{
    /// Left siblings of the node, eldest last.
    left: Vector<T>,

    /// Right siblings of the node, eldest first.
    right: Vector<T>,

    /// Function to convert this layer back into a tree given a full list of children.
    rebuild_tree: Arc<dyn Fn(Vector<T>) -> Tree<T>>,

    /// Function to create the parent node, given its children as a tree.
    ctx: Arc<dyn Fn(Tree<T>) -> T>,
}

impl<T> Zipper<T>
where
    T: Uniplate,
{
    /// Creates a new [`Zipper`] with `root` as the root node.
    ///
    /// The focus is initially the root node.
    pub fn new(root: T) -> Self {
        // TODO: This could be done slightly better, without re-calculating the size of every subtree
        // Maybe transform into a tree of the same shape holding metas at each node and flatten?
        let mut meta_list = vec![];
        for node in root.universe() {
            meta_list.push(Meta {
                clean_before: 0,
                subtree_size: tree_size(&node),
            })
        }

        Zipper {
            focus: root,
            path: Vec::new(),
            meta_list,
            meta_index: 0,
        }
    }

    /// Borrows the current focus of the [Zipper].
    pub fn focus(&self) -> &T {
        &self.focus
    }

    /// Mutably borrows the current focus of the [Zipper].
    pub fn focus_mut(&mut self) -> &T {
        &mut self.focus
    }

    /// Borrows the `Meta` object for the current focus.
    pub fn meta(&self) -> &Meta {
        &self
            .meta_list
            .get(self.meta_index)
            .expect("Meta index out of bounds")
    }

    /// Mutably borrows the `Meta` object for the current focus.
    pub fn meta_mut(&mut self) -> &Meta {
        &mut self
            .meta_list
            .get(self.meta_index)
            .expect("Meta index out of bounds")
    }

    /// Replaces the focus of the [Zipper], returning the old focus.
    pub fn replace_focus(&mut self, new_focus: T) -> T {
        // TODO: splice meta_list
        std::mem::replace(&mut self.focus, new_focus)
    }

    /// Rebuilds the root node from the Zipper.
    ///
    /// This is equivalent to calling `go_up` until the focus is the root node.
    pub fn rebuild_root(mut self) -> T {
        while self.go_up().is_some() {}
        self.focus
    }

    /// Returns the depth of the focus from the root.
    pub fn depth(&self) -> usize {
        self.path.len()
    }

    // TODO: return previous focus from cursor methods?

    /// Sets the focus to the parent of the focus (if it exists).
    pub fn go_up(&mut self) -> Option<()> {
        let mut path_seg = self.path.pop()?;

        // TODO: optimise this by looking at the cached subtree sizes
        self.meta_index -= path_seg
            .left
            .iter()
            .map(|node| tree_size(node))
            .sum::<usize>()
            + 1; // for the current node

        // TODO: get rid of the clone if possible
        path_seg.left.push_back(self.focus.clone());
        path_seg.left.append(path_seg.right);

        let tree = (path_seg.rebuild_tree)(path_seg.left);

        self.focus = (path_seg.ctx)(tree);

        Some(())
    }

    /// Sets the focus to the left-most child of the focus (if it exists).
    pub fn go_down(&mut self) -> Option<()> {
        let (children, ctx) = self.focus.uniplate();
        let (mut siblings, rebuild_tree) = children.list();
        let new_focus = siblings.pop_front()?;
        let new_segment = PathSegment {
            left: Vector::new(),
            right: siblings,
            rebuild_tree: rebuild_tree.into(),
            ctx: ctx.into(),
        };

        self.path.push(new_segment);
        self.focus = new_focus;

        self.meta_index += 1;

        Some(())
    }

    /// Sets the focus to the left sibling of the focus (if it exists).
    pub fn go_left(&mut self) -> Option<()> {
        let path_segment = self.path.last_mut()?;
        let new_focus = path_segment.left.pop_front()?;
        let old_focus = std::mem::replace(&mut self.focus, new_focus);
        path_segment.right.push_back(old_focus.clone());

        // TODO: optimise this by looking at the cached subtree sizes
        self.meta_index -= tree_size(&old_focus);

        Some(())
    }

    /// Sets the focus to the right sibling of the focus (if it exists).
    pub fn go_right(&mut self) -> Option<()> {
        let path_segment = self.path.last_mut()?;
        let new_focus = path_segment.right.pop_front()?;
        let old_focus = std::mem::replace(&mut self.focus, new_focus);
        path_segment.left.push_back(old_focus);

        self.meta_index += self.meta().subtree_size;

        Some(())
    }

    /// Sets the focus to the next node in preorder traversal order (if it exists).
    ///
    /// If the focus is the last node in the tree, the zipper will move to the root return `None`.
    pub fn go_next(&mut self) -> Option<()> {
        if self.go_down().is_some() {
            return Some(());
        }

        while self.go_right().is_none() {
            self.go_up()?;
        }

        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uniplate::derive::Uniplate;

    #[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
    #[uniplate()]
    enum Tree {
        Node(i32, Box<Tree>, Box<Tree>),
        Leaf(i32),
    }

    impl Tree {
        pub fn value(&self) -> i32 {
            match self {
                Tree::Node(v, _, _) => *v,
                Tree::Leaf(v) => *v,
            }
        }
    }

    #[test]
    fn test_go_next_single() {
        let tree = Tree::Leaf(1);
        let mut zipper = Zipper::new(tree);

        assert_eq!(zipper.focus().value(), 1);
        assert!(zipper.go_next().is_none());
    }

    #[test]
    fn test_go_next_order() {
        let tree = Tree::Node(
            1,
            Box::new(Tree::Node(
                2,
                Box::new(Tree::Leaf(3)),
                Box::new(Tree::Leaf(4)),
            )),
            Box::new(Tree::Node(
                5,
                Box::new(Tree::Leaf(6)),
                Box::new(Tree::Leaf(7)),
            )),
        );

        let mut zipper = Zipper::new(tree);

        let mut order = vec![];
        loop {
            order.push(zipper.focus().value());
            if zipper.go_next().is_none() {
                break;
            }
        }

        assert_eq!(order, (1..8).collect::<Vec<_>>());
    }

    #[test]
    fn test_go_up_meta_index() {
        let tree = Tree::Node(
            1,
            Box::new(Tree::Node(
                0,
                Box::new(Tree::Leaf(0)),
                Box::new(Tree::Leaf(0)),
            )),
            Box::new(Tree::Node(
                0,
                Box::new(Tree::Node(
                    0,
                    Box::new(Tree::Leaf(0)),
                    Box::new(Tree::Leaf(0)),
                )),
                Box::new(Tree::Leaf(0)),
            )),
        );

        let mut zipper = Zipper::new(tree);

        let idx1 = zipper.meta_index;

        zipper.go_down().unwrap();
        zipper.go_right().unwrap();

        let idx2 = zipper.meta_index;

        zipper.go_down().unwrap();
        zipper.go_right().unwrap();

        zipper.go_up().unwrap();
        assert_eq!(zipper.meta_index, idx2);

        zipper.go_up().unwrap();
        assert_eq!(zipper.meta_index, idx1);
    }
}
