use std::collections::VecDeque;
use uniplate::Uniplate;

enum Command<T: Uniplate, M> {
    Transform(fn(T) -> T),
    MutMeta(fn(&mut M)),
}

/// A queue of commands (side-effects) to be applied after a successful rule application.
///
/// A rule is given a mutable reference to a [`Commands`] and can use it to register side-effects.
/// These side-effects are applied in order of registration **after** the rule itself updates
/// a part of the tree.
///
/// # Application
///
/// A rule may not be applied due to different reasons, for example:
/// - It does not return a new subtree (i.e. it returns `None`).
/// - It returns a new subtree but the resulting [`Update`](crate::update::Update) is not chosen
/// by the user-defined selector function. The function may select a different rule's update or
/// no update at all.
/// - It is part of a lower-priority rule group and a higher-priority rule is applied first.
///
/// In these cases, any side-effects which are registered by the rule are not applied and are
/// dropped by the engine.
///
/// # Example
/// ```rust
/// use tree_morph::prelude::*;
/// use uniplate::derive::Uniplate;
///
/// #[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
/// #[uniplate()]
/// enum Expr {
///     A,
///     B,
///     C,
/// }
///
/// fn rule(cmds: &mut Commands<Expr, bool>, subtree: &Expr, meta: &bool) -> Option<Expr> {
///     cmds.transform(|t| match t { // A pure transformation (no other side-effects)
///         Expr::B => Expr::C,
///         _ => t,
///     });
///     cmds.mut_meta(|m| *m = true); // Set the metadata to 'true'
///
///     match subtree {
///         Expr::A => Some(Expr::B),
///         _ => None,
///     }
/// }
///
/// // Start with the expression 'A' and a metadata value of 'false'
/// let (result, meta) = morph(vec![rule_fns![rule]], select_first, Expr::A, false);
///
/// // After applying the rule itself, the commands are applied in order
/// assert_eq!(result, Expr::C);
/// assert_eq!(meta, true);
/// ```

pub struct Commands<T: Uniplate, M> {
    commands: VecDeque<Command<T, M>>,
}

impl<T: Uniplate, M> Commands<T, M> {
    pub(crate) fn new() -> Self {
        Self {
            commands: VecDeque::new(),
        }
    }

    /// Registers a pure transformation of the whole tree.
    ///
    /// In this case, "pure" means that the transformation cannot register additional side-effects.
    /// The transformation function is given ownership of the tree and should return the updated
    /// tree.
    ///
    /// Side-effects are applied in order of registration after the rule is applied.
    pub fn transform(&mut self, f: fn(T) -> T) {
        self.commands.push_back(Command::Transform(f));
    }

    /// Updates the global metadata in-place via a mutable reference.
    ///
    /// Side-effects are applied in order of registration after the rule is applied.
    pub fn mut_meta(&mut self, f: fn(&mut M)) {
        self.commands.push_back(Command::MutMeta(f));
    }

    /// Removes all side-effects previously registered by the rule.
    pub fn clear(&mut self) {
        self.commands.clear();
    }

    /// Consumes and apply the side-effects currently in the queue.
    pub(crate) fn apply(&mut self, mut tree: T, mut meta: M) -> (T, M) {
        while let Some(cmd) = self.commands.pop_front() {
            match cmd {
                Command::Transform(f) => tree = f(tree),
                Command::MutMeta(f) => f(&mut meta),
            }
        }
        (tree, meta)
    }
}
