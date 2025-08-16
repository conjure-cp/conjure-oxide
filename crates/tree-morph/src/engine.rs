#![allow(dead_code)]
#![allow(missing_docs)]
use std::{cell::RefCell, rc::Rc};

use crate::{Commands, Rule, Update, helpers::one_or_select};
use uniplate::{Uniplate, zipper::Zipper};
/// Exhaustively rewrites a tree using a set of transformation rules.
///
/// Rewriting is complete when all rules have been attempted with no change. Rules may be organised
/// into groups to control the order in which they are attempted.
///
/// # Rule Groups
/// If all rules are treated equally, those which apply higher in the tree will take precedence
/// because of the left-most outer-most traversal order of the engine.
///
/// This can cause problems if a rule which should ideally be applied early (e.g. evaluating
/// constant expressions) is left until later.
///
/// To solve this, rules can be organised into different collections in the `groups` argument.
/// The engine will apply rules in an earlier group to the entire tree before trying later groups.
/// That is, no rule is attempted if a rule in an earlier group is applicable to any part of the
/// tree.
///
/// # Selector Functions
///
/// If multiple rules in the same group are applicable to an expression, the user-defined
/// selector function is used to choose one. This function is given an iterator over pairs of
/// rules and the engine-created [`Update`] values which contain their modifications to the tree.
///
/// Some useful selector functions are available in the [`helpers`](crate::helpers) module. One
/// common function is [`select_first`](crate::helpers::select_first), which simply returns the
/// first applicable rule.
///
/// # Optimizations
///
/// To optimize the morph function, we use a dirty/clean approach. Since whether a rule applies
/// is purely a function of a node and its children, rules should only be checked once unless a node
/// or one of its children has been changed. To enforce this we use a skeleton tree approach, which
/// keeps track of the current level of which a node has had a rule check applied.
/// # Example
/// ```rust
/// use tree_morph::{prelude::*, helpers::select_panic};
/// use uniplate::Uniplate;
///
///
/// // A simple language of multiplied and squared constant expressions
/// #[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
/// #[uniplate()]
/// enum Expr {
///     Val(i32),
///     Mul(Box<Expr>, Box<Expr>),
///     Sqr(Box<Expr>),
/// }
///
/// // a * b ~> (value of) a * b, where 'a' and 'b' are literal values
/// fn rule_eval_mul(cmds: &mut Commands<Expr, i32>, subtree: &Expr, meta: &i32) -> Option<Expr> {
///     cmds.mut_meta(Box::new(|m: &mut i32| *m += 1));
///
///     if let Expr::Mul(a, b) = subtree {
///         if let (Expr::Val(a_v), Expr::Val(b_v)) = (a.as_ref(), b.as_ref()) {
///             return Some(Expr::Val(a_v * b_v));
///         }
///     }
///     None
/// }
///
/// // e ^ 2 ~> e * e, where e is an expression
/// // If this rule is applied before the sub-expression is fully evaluated, duplicate work
/// // will be done on the resulting two identical sub-expressions.
/// fn rule_expand_sqr(cmds: &mut Commands<Expr, i32>, subtree: &Expr, meta: &i32) -> Option<Expr> {
///     cmds.mut_meta(Box::new(|m: &mut i32| *m += 1));
///
///     if let Expr::Sqr(expr) = subtree {
///         return Some(Expr::Mul(
///             Box::new(*expr.clone()),
///             Box::new(*expr.clone())
///         ));
///     }
///     None
/// }
///
/// // (1 * 2) ^ 2
/// let expr = Expr::Sqr(
///     Box::new(Expr::Mul(
///         Box::new(Expr::Val(1)),
///         Box::new(Expr::Val(2))
///     ))
/// );
///
/// // Try with both rules in the same group, keeping track of the number of rule applications
/// let (result, num_applications) = morph(
///     vec![rule_fns![rule_eval_mul, rule_expand_sqr]],
///     select_panic,
///     expr.clone(),
///     0
/// );
/// assert_eq!(result, Expr::Val(4));
/// assert_eq!(num_applications, 4); // The `Sqr` is expanded first, causing duplicate work
///
/// // Move the evaluation rule to an earlier group
/// let (result, num_applications) = morph(
///     vec![rule_fns![rule_eval_mul], rule_fns![rule_expand_sqr]],
///     select_panic,
///     expr.clone(),
///     0
/// );
/// assert_eq!(result, Expr::Val(4));
/// assert_eq!(num_applications, 3); // Now the sub-expression (1 * 2) is evaluated first
/// ```
pub fn morph<T, M, R>(
    groups: Vec<Vec<R>>,
    select: impl Fn(&T, &mut dyn Iterator<Item = (&R, Update<T, M>)>) -> Option<Update<T, M>>,
    tree: T,
    meta: M,
) -> (T, M)
where
    T: Uniplate,
    R: Rule<T, M>,
{
    let transforms: Vec<_> = groups
        .iter()
        .map(|group| {
            |subtree: &T, meta: &M| {
                let applicable = &mut group.iter().filter_map(|rule| {
                    let mut commands = Commands::new();
                    let new_tree = rule.apply(&mut commands, subtree, meta)?;
                    Some((
                        rule,
                        Update {
                            new_subtree: new_tree,
                            commands,
                        },
                    ))
                });
                one_or_select(&select, subtree, applicable)
            }
        })
        .collect();
    morph_impl(transforms, tree, meta)
}

///Used for the skeleton tree in the dirty/clean optimization. Only has one field and is used for
/// purposes of clarity.
pub struct State {
    pub node: Rc<RefCell<NodeState>>,
}

impl State {
    ///Creates a new state object with one dirty node. Used to create root nodes.
    pub fn new() -> Self {
        let node = Rc::new(RefCell::new(NodeState::new_dirty()));
        Self { node }
    }

    ///Changes the current cleanliness level of a node. It then also sets the current_child_index to 0, to
    /// ensure that the user-given tree and the skeleton tree do not get out of sync.
    pub fn mark_cleanliness(&mut self, cleanliness: usize) {
        self.node.borrow_mut().cleanliness = cleanliness;
        self.node.borrow_mut().current_child_index = 0;
    }

    ///Retrieve current cleanliness level.
    pub fn get_cleanliness(&self) -> usize {
        self.node.borrow().cleanliness
    }

    ///Clear all the children of a node. This occurs as when a rule is transformed to a node, the skeleton
    /// tree no longer knows anything about the structure of
    /// its children, so will have to rebuilt the children from scratch.
    pub fn clear_children(&mut self) {
        let mut node_state = self.node.borrow_mut();
        node_state.children.clear();
        node_state.current_child_index = 0;
    }

    ///Go to the next child, or create one if needed. Will only ever run upon the zipper being able to go down,
    ///so there will never be an error whereby the children list is empty.
    pub fn go_down(&mut self) {
        let node = Rc::clone(&self.node);
        let mut node_state = node.borrow_mut();

        if node_state.current_child_index < node_state.children.len() {
            self.node = Rc::clone(&node_state.children[node_state.current_child_index]);
            return;
        }
        let mut new_node = NodeState::new_dirty();
        new_node.parent = Some(Rc::clone(&self.node));
        node_state.children.push(Rc::new(RefCell::new(new_node)));
        // Left most outer most
        self.node = Rc::clone(&node_state.children[0]);
    }

    ///Go to the right sibling of a node. Will only be called if zipper can go right.
    pub fn go_right(&mut self) {
        let left_sibling = Rc::clone(&self.node);
        let left_sibling_state = left_sibling.borrow();

        let parent = left_sibling_state.parent.as_ref().unwrap();
        let mut parent_state = parent.borrow_mut();
        parent_state.current_child_index += 1;

        if parent_state.current_child_index < parent_state.children.len() {
            self.node = Rc::clone(&parent_state.children[parent_state.current_child_index]);
            return;
        }

        let mut new_node = NodeState::new_dirty();
        new_node.parent = Some(Rc::clone(parent));
        parent_state.children.push(Rc::new(RefCell::new(new_node)));
        self.node = Rc::clone(&parent_state.children[parent_state.current_child_index]);
    }

    ///Go up. Will only be called if zipper can go up.
    pub fn go_up(&mut self) {
        let node = Rc::clone(&self.node);
        let node_state = node.borrow();

        self.node = Rc::clone(node_state.parent.as_ref().unwrap());
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

use std::fmt;

/// Contains the core information for the the skeleton tree. The recursive tree structure is captured using `<Rc<RefCell<NodeState>>>`, which allows for
/// multiple owners of a value, as well as mutable access to data even if a reference is immutable.
pub struct NodeState {
    ///Holds the parents of a node. It is wrapped in an `Option<>` as a node may have no parents (i.e. it may be a root node)
    pub parent: Option<Rc<RefCell<NodeState>>>,
    ///C`Vec` containing all children of a node. It is empty by default.
    pub children: Vec<Rc<RefCell<NodeState>>>,
    ///Keeps track of which child is currently active in the tree traversal process.
    pub current_child_index: usize,
    ///Integer that marks up to which rule level a certain node has been checked. By default nodes start off with a cleanliness of 0.
    pub cleanliness: usize,
}

impl NodeState {
    ///Creates a new root node
    pub fn new_dirty() -> Self {
        Self {
            current_child_index: 0,
            cleanliness: 0,
            parent: None,
            children: Vec::new(),
        }
    }

    fn fmt_children(&self, f: &mut fmt::Formatter<'_>, indent: usize) -> fmt::Result {
        for (i, child) in self.children.iter().enumerate() {
            writeln!(f, "{}Child {}: {{", " ".repeat(indent), i)?;
            let child = child.borrow();
            writeln!(f, "{}  dirty: {},", " ".repeat(indent), child.cleanliness)?;
            writeln!(
                f,
                "{}  current_child_index: {},",
                " ".repeat(indent),
                child.current_child_index
            )?;
            writeln!(f, "{}  children: [", " ".repeat(indent))?;
            child.fmt_children(f, indent + 4)?;
            writeln!(f, "{}  ]", " ".repeat(indent))?;
            writeln!(f, "{}}}", " ".repeat(indent))?;
        }
        Ok(())
    }
}

impl fmt::Debug for NodeState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "NodeState {{")?;
        writeln!(f, "  cleanliness: {},", self.cleanliness)?;
        writeln!(f, "  current_child_index: {},", self.current_child_index)?;
        writeln!(f, "  parent: {},", self.parent.is_some())?;
        writeln!(f, "  children: [")?;
        self.fmt_children(f, 4)?;
        writeln!(f, "  ]")?;
        write!(f, "}}")
    }
}

///Contains two fields which correspond to the position in the user-given tree, and the skeleton tree.
///At any point in time these should be in sync, meaning that where zipper is in the user-given tree
/// should correspond to the position in the skeleton tree.
pub struct DirtyZipper<T: Uniplate> {
    ///Allows for efficient traversal of the user-given tree.
    pub zipper: Zipper<T>,
    ///Skeleton tree for cleanliness data
    pub state: State,
}

impl<T: Uniplate> DirtyZipper<T> {
    ///Given a zipper, create a root node initialised to the current location of the zipper
    pub fn new(zipper: Zipper<T>) -> Self {
        Self {
            zipper,
            state: State::new(),
        }
    }

    /// Marks all ancetors of a subtree as dirty and moves the zipper at the start of the tree,
    /// ready for new rule applications.
    pub fn mark_dirty(&mut self) {
        self.state.mark_cleanliness(0);
        self.state.clear_children();

        // Effectively the same as rebuild_root
        while self.zipper.go_up().is_some() {
            self.state.go_up();
            self.state.mark_cleanliness(0);
        }
    }

    /// Finds the next dirty node. If the current state is dirty, it will return that position of state,
    /// and otherwise will go to the first child. If the first child is not dirty, it will go to the right sibling.
    /// Once the siblings are exhausted, a next dirt state is searched for by going up and right. If
    /// this process is exhausted, then the tree is clean up to a certain level and hence `None` is returned.
    pub fn get_next_dirty(&mut self, level: usize) -> Option<&T> {
        if self.state.get_cleanliness() <= level {
            return Some(self.zipper.focus());
        }

        if self.zipper.go_down().is_some() {
            self.state.go_down();
            if self.state.get_cleanliness() <= level {
                return Some(self.zipper.focus());
            }
        }
        while self.zipper.go_right().is_some() {
            self.state.go_right();
            if self.state.get_cleanliness() <= level {
                return Some(self.zipper.focus());
            }
        }

        while self.zipper.go_up().is_some() {
            self.state.go_up();
            while self.zipper.go_right().is_some() {
                self.state.go_right();
                if self.state.get_cleanliness() <= level {
                    return Some(self.zipper.focus());
                }
            }
        }
        None
    }
}
/// Applies a series of transformation functions to a tree structure and its associated metadata.
pub fn morph_impl<T: Uniplate, M>(
    transforms: Vec<impl Fn(&T, &M) -> Option<Update<T, M>>>,
    tree: T,
    mut meta: M,
) -> (T, M) {
    let zipper = Zipper::new(tree);
    let mut dirty_zipper = DirtyZipper::new(zipper);
    'main: loop {
        for (level, transform) in transforms.iter().enumerate() {
            while let Some(node) = dirty_zipper.get_next_dirty(level) {
                if let Some(mut update) = transform(node, &meta) {
                    dirty_zipper.zipper.replace_focus(update.new_subtree);
                    dirty_zipper.mark_dirty();
                    let (new_tree, new_meta, transformed) = update
                        .commands
                        .apply(dirty_zipper.zipper.focus().clone(), meta);
                    meta = new_meta;

                    // Transformations are defined as fn(T) -> T, so sadly we must throw the state
                    // away
                    if transformed {
                        dirty_zipper.state = State::new();
                        dirty_zipper.zipper = Zipper::new(new_tree);
                    }
                    continue 'main;
                } else {
                    dirty_zipper.state.mark_cleanliness(level + 1);
                }
            }
        }
        break;
    }
    (dirty_zipper.zipper.rebuild_root(), meta)
}
