use std::{cell::RefCell, rc::Rc, usize};

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
                    (new_tree, meta, _) = update.commands.apply(whole_tree, meta);

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

struct State {
    node: Rc<RefCell<NodeState>>,
}

impl State {
    pub fn new() -> Self {
        let node = Rc::new(RefCell::new(NodeState::new_dirty()));
        Self { node }
    }
    
    pub fn mark_cleanliness(&mut self, cleanliness: usize) {
        self.node.borrow_mut().cleanliness = cleanliness;
        self.node.borrow_mut().current_child_index = 0;
    }

    pub fn get_cleanliness(&self) -> usize {
        self.node.borrow().cleanliness
    }

    pub fn clear_children(&mut self) {
        let mut node_state = self.node.borrow_mut();
        node_state.children.clear();
        node_state.current_child_index = 0;
    }

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
        new_node.parent = Some(Rc::clone(&parent));
        parent_state.children.push(Rc::new(RefCell::new(new_node)));
        self.node = Rc::clone(&parent_state.children[parent_state.current_child_index]);
    }

    pub fn go_up(&mut self) {
        let node = Rc::clone(&self.node);
        let node_state = node.borrow();

        self.node = Rc::clone(node_state.parent.as_ref().unwrap());
    }
}

use std::fmt;

struct NodeState {
    parent: Option<Rc<RefCell<NodeState>>>,
    children: Vec<Rc<RefCell<NodeState>>>,
    current_child_index: usize,
    cleanliness: usize,
}

impl NodeState {
    pub fn new_dirty() -> Self {
        Self {
            current_child_index: 0,
            cleanliness: 0,
            parent: None,
            children: Vec::new(),
        }
    }

    pub fn new_clean() -> Self {
        Self {
            current_child_index: 0,
            cleanliness: usize::MAX,
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

struct DirtyZipper<T: Uniplate> {
    zipper: Zipper<T>,
    state: State,
}

impl<T: Uniplate> DirtyZipper<T> {
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
        while let Some(_) = self.zipper.go_up() {
            self.state.go_up();
            self.state.mark_cleanliness(0);
        }
    }
    
    // Can I use recursion here to make this less clunky?
    pub fn get_next_dirty(&mut self, level: usize) -> Option<&T> {
        if self.state.get_cleanliness() <= level {
            return Some(self.zipper.focus());
        }

        if let Some(_) = self.zipper.go_down() {
            self.state.go_down();
            if self.state.get_cleanliness() <= level {
                return Some(self.zipper.focus());
            }
        }
        while let Some(_) = self.zipper.go_right() {
            self.state.go_right();
            if self.state.get_cleanliness() <= level {
                return Some(self.zipper.focus());
            }
        }

        while let Some(_) = self.zipper.go_up() {
            self.state.go_up();
            while let Some(_) = self.zipper.go_right() {
                self.state.go_right();
                if self.state.get_cleanliness() <= level {
                    return Some(self.zipper.focus());
                }
            }
        }
        None
    }
}

fn morph_zipper_impl<T: Uniplate, M>(
    transforms: Vec<impl Fn(&T, &M) -> Option<Update<T, M>>>,
    tree: T,
    mut meta: M,
) -> (T, M) {
    let zipper = Zipper::new(tree);
    let mut dirty_zipper = DirtyZipper::new(zipper);
    'main: loop {
        for (level, transform) in transforms.iter().enumerate() {
            while let Some(node) = dirty_zipper.get_next_dirty(level) {
                if let Some(mut update) = transform(&node, &meta) {
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
