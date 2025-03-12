use std::{cell::RefCell, rc::Rc};

use crate::{helpers::one_or_select, Commands, Rule, Update};
use uniplate::{zipper::Zipper, Uniplate};

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
/// # Example
/// ```rust
/// use tree_morph::{prelude::*, helpers::select_panic};
/// use uniplate::derive::Uniplate;
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
    // morph_impl(transforms, tree, meta)
    morph_zipper_impl(transforms, tree, meta)
}

/// This implements the core rewriting logic for the engine.
///
/// Iterate over rule groups and apply each one to the tree. If any changes are
/// made, restart with the first rule group.
fn morph_impl<T: Uniplate, M>(
    transforms: Vec<impl Fn(&T, &M) -> Option<Update<T, M>>>,
    mut tree: T,
    mut meta: M,
) -> (T, M) {
    let mut new_tree = tree;

    'main: loop {
        tree = new_tree;
        for transform in transforms.iter() {
            // Try each transform on the entire tree before moving to the next
            for (node, ctx) in tree.contexts() {
                if let Some(mut update) = transform(&node, &meta) {
                    let whole_tree = ctx(update.new_subtree);
                    (new_tree, meta) = update.commands.apply(whole_tree, meta);

                    // Restart with the first transform every time a change is made
                    continue 'main;
                }
            }
        }
        // All transforms were attempted without change
        break;
    }
    (tree, meta)
}

struct NodeState {
    parent: Option<Rc<RefCell<NodeState>>>,
    current_child_index: usize,
    children: Vec<Rc<RefCell<NodeState>>>,
    dirty: bool,
}

impl NodeState {
    pub fn new_dirty() -> Self {
        Self {
            current_child_index: 0,
            dirty: true,
            parent: None,
            children: Vec::new(),
        }
    }

    pub fn new_clean() -> Self {
        Self {
            current_child_index: 0,
            dirty: false,
            parent: None,
            children: Vec::new(),
        }
    }
}

impl std::fmt::Debug for NodeState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Start a debug struct for the current node
        let mut debug_struct = f.debug_struct("NodeState");

        // Add basic fields
        debug_struct
            .field("dirty", &self.dirty)
            .field("current_child_index", &self.current_child_index)
            .field("num_children", &self.children.len());

        // Add children information without recursing into their parents
        let children_debug: Vec<_> = self
            .children
            .iter()
            .enumerate()
            .map(|(idx, child)| {
                // Create a custom debug representation for each child
                // that avoids recursing into parent
                format!(
                    "Child[{}]: {{ dirty: {}, num_children: {} }}",
                    idx,
                    child.borrow().dirty,
                    child.borrow().children.len()
                )
            })
            .collect();

        debug_struct.field("children", &children_debug);

        // Indicate if a parent exists without recursing
        debug_struct.field("has_parent", &self.parent.is_some());

        // Finish and output the debug struct
        debug_struct.finish()
    }
}

struct DirtyZipper<T: Uniplate> {
    zipper: Zipper<T>,
    node_state: Option<Rc<RefCell<NodeState>>>,
}

impl<T: Uniplate> DirtyZipper<T> {
    pub fn new(zipper: Zipper<T>) -> Self {
        Self {
            zipper,
            node_state: None,
        }
    }

    /// Marks all ancetors of a subtree as dirty and moves the zipper at the start of the tree,
    /// ready for new rule applications.
    /// Set the children of the node to be empty - This is a new tree so we don't know what's
    /// below it.
    pub fn mark_dirty(&mut self) {
        println!("Marking Dirty!");
        let node = self
            .node_state
            .as_ref()
            .expect("NodeState and Zipper out of Sync");
        let mut node_state = Rc::clone(&node);
        {
            let mut node_state_mut = node_state.borrow_mut();
            node_state_mut.dirty = true;
            node_state_mut.current_child_index = 0;
            node_state_mut.children.clear();
        }
        while let Some(_) = self.zipper.go_up() {
            println!("Marked Parent as dirty");
            // Is there a way to do this without cloning?
            let parent_option = node_state.borrow().parent.clone();
            let parent_ref = parent_option
                .as_ref()
                .expect("NodeState and Zipper out of Sync. Missing Parent");
            let mut parent_mut = parent_ref.borrow_mut();
            parent_mut.dirty = true;
            parent_mut.current_child_index = 0;
            node_state.borrow_mut().parent = Some(Rc::clone(parent_ref));
            node_state = Rc::clone(parent_ref);
            self.node_state = Some(Rc::clone(parent_ref));
        }
        dbg!(&self.node_state);
    }

    pub fn mark_clean(&mut self) {
        let node = self
            .node_state
            .as_ref()
            .expect("NodeState and Zipper out of Sync");
        let node_state = Rc::clone(&node);
        node_state.borrow_mut().dirty = false;
    }

    /// Im in the thick of it rn...
    /// This is currently NOT working
    /// The state is not properly synced right now 
    /// This will still exhaustively go through all nodes 
    ///
    /// Skill issue from my end:
    /// The way I've handled the state movement isnt very clear 
    /// so I probably got confused from my own writing and somehow not linked the parent and child
    /// note states together 
    ///
    /// When it marks as dirty and moves up, for some reason the root has no children (where did
    /// they go???)
    pub fn get_next_dirty(&mut self) -> Option<&T> {
        if self.node_state.is_none() {
            // TODO: Revisit Clean or Dirty
            self.node_state = Some(Rc::new(RefCell::new(NodeState::new_clean())));
            return Some(self.zipper.focus());
        }

        if self.node_state.as_ref().unwrap().borrow().dirty {
            return Some(self.zipper.focus());
        }

        let node = Rc::clone(self.node_state.as_ref().unwrap());

        if let Some(_) = self.zipper.go_down() {
            println!("Going Down");
            let mut node_state = node.borrow_mut();
            // Safety: The parent is already created as above
            let child_index = node_state.current_child_index;

            // We have a child
            println!("{}  < {}", child_index, node_state.children.len());
            if child_index < node_state.children.len() {
                let child = &node_state.children[child_index];
                if child.borrow().dirty {
                    self.node_state = Some(Rc::clone(child));
                    return Some(self.zipper.focus());
                }
                println!("Child is clean");
                // Child is Clean, move right so do nothing
            } else {
                // No child, create state and return focus
                println!("New Child Made");
                let mut child = NodeState::new_clean();
                child.parent = Some(Rc::clone(&node));
                node_state.children.push(Rc::new(RefCell::new(child)));
                self.node_state = Some(Rc::clone(&node_state.children.last().unwrap()));
                return Some(self.zipper.focus());
            }
        }

        // TODO: ASK FELIX
        // Either the child is clean or there is no child
        while let Some(_) = self.zipper.go_right() {
            println!("Going Right");
            let node_state = node.borrow();
            let mut parent_node_state = node_state.parent.as_ref().unwrap().borrow_mut();
            // If we can go right then there is a parent state node
            parent_node_state.current_child_index += 1;
            let child_index = parent_node_state.current_child_index;

            println!("{}  < {}", child_index, node_state.children.len());
            if child_index < parent_node_state.children.len() {
                let child = &parent_node_state.children[child_index];
                if child.borrow().dirty {
                    self.node_state = Some(Rc::clone(child));
                    return Some(self.zipper.focus());
                }
                println!("Child is clean");
            } else {
                // Need to make child
                println!("New Child Made");
                let mut child = NodeState::new_clean();
                child.parent = Some(Rc::clone(&node));
                parent_node_state
                    .children
                    .push(Rc::new(RefCell::new(child)));
                self.node_state = Some(Rc::clone(&parent_node_state.children.last().unwrap()));
                return Some(self.zipper.focus());
            }
        }

        // Failed to go Down, Failed to go Right

        while let Some(_) = self.zipper.go_up() {
            println!("Going Up");
            let node_state = node.borrow();
            let mut parent_node_state = node_state
                .parent
                .as_ref()
                .expect("NodeState and Zipper out of Sync")
                .borrow_mut();
            while let Some(_) = self.zipper.go_right() {
                println!("Going Right");
                parent_node_state.current_child_index += 1;
                let child_index = parent_node_state.current_child_index;
                println!("{}  < {}", child_index, node_state.children.len());
                if child_index < parent_node_state.children.len() {
                    let child = &parent_node_state.children[child_index];
                    if child.borrow().dirty {
                        self.node_state = Some(Rc::clone(child));
                        return Some(self.zipper.focus());
                    }
                    println!("Child is clean");
                } else {
                    // Need to make child
                    println!("New Child Made");
                    let mut child = NodeState::new_clean();
                    child.parent = Some(Rc::clone(&node));
                    parent_node_state
                        .children
                        .push(Rc::new(RefCell::new(child)));
                    self.node_state = Some(Rc::clone(&parent_node_state.children.last().unwrap()));
                    return Some(self.zipper.focus());
                }
            }
        }
        None
    }
}

fn morph_zipper_impl<T: Uniplate, M>(
    transforms: Vec<impl Fn(&T, &M) -> Option<Update<T, M>>>,
    mut tree: T,
    mut meta: M,
) -> (T, M) {
    let mut new_tree = tree;
    let zipper = Zipper::new(new_tree.clone());
    let mut dirty_zipper = DirtyZipper::new(zipper);
    'main: loop {
        // I need to do my thing
        for transform in transforms.iter() {
            while let Some(node) = dirty_zipper.get_next_dirty() {
                println!("Got next dirty node");
                if let Some(update) = transform(&node, &meta) {
                    println!("Applying Rule!!!!!");
                    dirty_zipper.zipper.replace_focus(update.new_subtree);
                    dirty_zipper.mark_dirty();
                    // update.commands.apply(dirtyZipper.zipper.focus().clone(), meta);
                    continue 'main;
                } else {
                    dirty_zipper.mark_clean();
                }
            }
        }
        break;
    }
    (dirty_zipper.zipper.rebuild_root(), meta)
}
