//! Traits and types representing a transformation rule to a tree.
//!
//! See the [`Rule`] trait for more information.

use crate::prelude::{Commands, Update};
use uniplate::Uniplate;

/// Trait implemented by rules to transform parts of a tree.
///
/// Rules contain a method `apply` which accepts a [`Commands`] instance, a subtree, and
/// global metadata. If the rule is applicable to the subtree, it should return `Some(<new_tree>)`,
/// otherwise it should return `None`.
///
/// # Rule Application
/// As the engine traverses the tree (in left-most, outer-most order), it will apply rules to each
/// node. The `subtree` argument passed to the rule is the current node being visited.
///
/// If a rule is applicable to the given node/subtree (i.e. can transform it), then it should return
/// the resulting new subtree, which will be inserted into the tree in place of the original node.
///
/// # Side-Effects
///
/// The [`Commands`] instance passed to the rule can be used to apply side-effects after the rule
/// has been applied. This can be used to update global metadata, or to apply transformations to the
/// entire tree.
///
/// # Global Metadata
/// In contrast to the `subtree` argument given to rules, the `meta` argument is a
/// reference to a global value which is available to all rules regardless of where in
/// the tree they are applied. This user-defined value can be used to store information
/// such as a symbol table, or the number of times a specific rule has been applied.
///
/// The global metadata may be mutated as a side-effect of applying a rule, using the
/// [`Commands::mut_meta`] method.
///
/// # Provided Implementations
/// This trait is automatically implemented by all types which implement
/// `Fn(&mut Commands<T, M>, &T, &M) -> Option<T>` for types `T: Uniplate` and `M`. This allows
/// function pointers and closures with the correct signatures to be used as rules directly.
///
/// # Example
/// ```rust
/// use tree_morph::prelude::*;
/// use uniplate::Uniplate;
///
///
/// #[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
/// #[uniplate()]
/// enum Term {
///     A,
///     B,
/// }
///
/// // Functions and closures automatically implement the Rule trait
/// fn my_rule_fn(_: &mut Commands<Term, ()>, _: &Term, _: &()) -> Option<Term> {
///     None // Never applicable
/// }
///
/// let (result, _) = morph(vec![rule_fns![my_rule_fn]], select_first, Term::A, ());
/// assert_eq!(result, Term::A);
///
///
/// // Custom types can implement the Rule trait to allow more complex behaviour
/// // Here a rule can be "toggled" to change whether it is applicable
/// struct CustomRule(bool);
///
/// impl Rule<Term, ()> for CustomRule {
///     fn apply(&self, _: &mut Commands<Term, ()>, t: &Term, _: &()) -> Option<Term> {
///         if self.0 && matches!(t, Term::A) {
///             Some(Term::B)
///         } else {
///             None
///         }
///     }
/// }
///
/// let (result, _) = morph(vec![vec![CustomRule(false)]], select_first, Term::A, ());
/// assert_eq!(result, Term::A);
///
/// let (result, _) = morph(vec![vec![CustomRule(true)]], select_first, Term::A, ());
/// assert_eq!(result, Term::B);
/// ```
pub trait Rule<T: Uniplate, M> {
    /// Applies the rule to the given subtree and returns the result if applicable.
    ///
    /// See the [Rule] trait documentation for more information.
    fn apply(&self, commands: &mut Commands<T, M>, subtree: &T, meta: &M) -> Option<T>;

    /// A helper method to get an [`Update`] directly from a rule.
    #[doc(hidden)]
    fn apply_into_update(&self, subtree: &T, meta: &M) -> Option<Update<T, M>> {
        let mut commands = Commands::new();
        let new_subtree = self.apply(&mut commands, subtree, meta)?;
        Some(Update::new(new_subtree, commands))
    }
}

// Allows the user to pass closures and function pointers directly as rules
impl<T, M, F> Rule<T, M> for F
where
    T: Uniplate,
    F: Fn(&mut Commands<T, M>, &T, &M) -> Option<T>,
{
    fn apply(&self, commands: &mut Commands<T, M>, subtree: &T, meta: &M) -> Option<T> {
        (self)(commands, subtree, meta)
    }
}

/// A uniform type for `fn` pointers and closures, which implements the [Rule] trait.
///
/// Casting an `fn` pointer or closure to this type allows it to be passed to the engine alongside
/// other such types. This is necessary since no two `fn` pointers or closures have the same
/// type, and thus cannot be stored in a single collection without casting.
///
/// See the [rule_fns!](crate::rule_fns) macro for a convenient way to do this.
pub type RuleFn<T, M> = fn(&mut Commands<T, M>, &T, &M) -> Option<T>;

/// A convenience macro to cast a list of `fn` pointers or closures to a uniform type.
///
/// Casting an `fn` pointer or closure to this type allows it to be passed to the engine alongside
/// other such types. This is necessary since no two `fn` pointers or closures have the same
/// type, and thus cannot be stored in a single collection without casting.
///
/// # Example
/// ```rust
/// use tree_morph::prelude::*;
/// use uniplate::Uniplate;
///
///
/// #[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
/// #[uniplate()]
/// struct Foo;
///
/// fn rule_a(_: &mut Commands<Foo, ()>, _: &Foo, _: &()) -> Option<Foo> {
///     None
/// }
///
/// fn rule_b(_: &mut Commands<Foo, ()>, _: &Foo, _: &()) -> Option<Foo> {
///     None
/// }
///
/// let rules = vec![
///     rule_fns![rule_a],
///     vec![rule_a as RuleFn<_, _>], // Same as above
///     rule_fns![rule_b, |_, _, _| None], // Closures and fn pointers can be mixed
/// ];
///
/// morph(rules, select_first, Foo, ());
/// ```
#[macro_export]
macro_rules! rule_fns {
    [$($x:expr),+ $(,)?] => {
        vec![$( $x as ::tree_morph::prelude::RuleFn<_, _>, )*]
    };
}
