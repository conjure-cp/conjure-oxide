//! Traits and types representing a transformation rule to a tree.
//!
//! See the [`Rule`] trait for more information.

use std::{collections::HashMap, marker::PhantomData};

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
/// let engine = EngineBuilder::new()
///     .add_rule_group(rule_fns![my_rule_fn])
///     .build();
/// let (result, _) = engine.morph(Term::A, ());
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
/// let engine = EngineBuilder::new()
///     .add_rule(CustomRule(false))
///     .build();
/// let (result, _) = engine.morph(Term::A, ());
/// assert_eq!(result, Term::A);
///
/// let engine = EngineBuilder::new()
///     .add_rule(CustomRule(true))
///     .build();
/// let (result, _) = engine.morph(Term::A, ());
/// assert_eq!(result, Term::B);
/// ```
pub trait Rule<T: Uniplate, M> {
    /// Applies the rule to the given subtree and returns the result if applicable.
    ///
    /// See the [Rule] trait documentation for more information.
    fn apply(&self, commands: &mut Commands<T, M>, subtree: &T, meta: &M) -> Option<T>;

    /// Return the name of the rule, will default to anonymous if not specified.
    fn name(&self) -> &str {
        "Anonymous Rule"
    }

    /// None -> Rule applies to all nodes
    /// Some(ids) -> Rule only applies to nodes with these discriminant ids
    fn applicable_to(&self) -> Option<Vec<usize>> {
        None
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

/// A helper method to get an [`Update`] directly from a rule.
pub(crate) fn apply_into_update<T, M, R>(rule: &R, subtree: &T, meta: &M) -> Option<Update<T, M>>
where
    T: Uniplate,
    R: Rule<T, M>,
{
    let mut commands = Commands::new();
    let new_subtree = rule.apply(&mut commands, subtree, meta)?;
    Some(Update::new(new_subtree, commands))
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
/// ```
#[macro_export]
macro_rules! rule_fns {
    [$($x:expr),+ $(,)?] => {
        vec![$( $x as ::tree_morph::prelude::RuleFn<_, _>, )*]
    };
}

/// We can create a rule using this struct and pass it into our list of rules directly,
/// For debugging and tracing, it is helpful to see rules by a meaningful name.
/// or we can make use of the `named_rule` macro (see [`tree-morph-macros`]).
///
/// This struct and macro is for the short form way of defining named rules. You can change the name
/// of the rule by implementing the `Rule` trait as well.
///
///  ```rust
/// use tree_morph::prelude::*;
/// use tree_morph_macros::named_rule;
/// use uniplate::Uniplate;
///
/// #[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
/// #[uniplate()]
/// enum Expr {
///     # Add(Box<Expr>, Box<Expr>),
///    // Snip
/// }
///
/// struct Meta;
///
/// #[named_rule("CustomName")]
/// fn my_rule(_: &mut Commands<Expr, Meta>, expr: &Expr, _: &Meta) -> Option<Expr> {
///     /// rule implementation
///     # None
/// }
/// ```
/// This macro will return a helper function called `my_rule` which returns the NamedRule for us to
/// use. We can add this to our list of rules with `vec![my_rule]`.
///
/// If a name is not specified, the functions name will be it's identifier.
#[derive(Clone)]
pub struct NamedRule<F: Clone> {
    name: &'static str,
    function: F,
}

impl<F: Clone> NamedRule<F> {
    /// Create a Rule with a specified name.
    pub const fn new(name: &'static str, function: F) -> Self {
        Self { name, function }
    }
}

impl<T, M, F> Rule<T, M> for NamedRule<F>
where
    T: Uniplate,
    F: Fn(&mut Commands<T, M>, &T, &M) -> Option<T> + Clone,
{
    fn apply(&self, commands: &mut Commands<T, M>, subtree: &T, meta: &M) -> Option<T> {
        (self.function)(commands, subtree, meta)
    }

    fn name(&self) -> &str {
        self.name
    }
}

/// TODO
pub struct RuleGroups<T, M, R>
where
    T: Uniplate,
    R: Rule<T, M> + Clone,
{
    /// Priority -> Rules
    universal_rules: Vec<Vec<R>>,

    /// Priority -> discriminant id -> Rules
    filtered_rules: Option<Vec<HashMap<usize, Vec<R>>>>,

    /// Function to compute a unique usize id for a node, used for prefiltering.
    /// None means prefiltering is disabled.
    pub discriminant_fn: Option<fn(&T) -> usize>,

    _phantom: PhantomData<(T, M)>,
}

impl<T, M, R> RuleGroups<T, M, R>
where
    T: Uniplate,
    R: Rule<T, M> + Clone,
{
    /// TODO
    pub fn new(rule_group: Vec<Vec<R>>, discriminant_fn: Option<fn(&T) -> usize>) -> Self {
        let Some(discriminant_fn) = discriminant_fn else {
            return Self {
                filtered_rules: None,
                universal_rules: rule_group,
                discriminant_fn: None,
                _phantom: PhantomData,
            };
        };

        let mut filtered_rules = Vec::with_capacity(rule_group.len());
        let mut universal_rules = Vec::with_capacity(rule_group.len());

        for rules in rule_group {
            let mut filtered: HashMap<usize, Vec<R>> = HashMap::new();
            let mut universal = Vec::new();

            for rule in rules {
                match rule.applicable_to() {
                    None => universal.push(rule),
                    Some(ids) => {
                        for id in ids {
                            filtered.entry(id).or_default().push(rule.clone());
                        }
                    }
                };
            }

            filtered_rules.push(filtered);
            universal_rules.push(universal);
        }
        Self {
            universal_rules,
            filtered_rules: Some(filtered_rules),
            discriminant_fn: Some(discriminant_fn),
            _phantom: PhantomData,
        }
    }

    /// TODO
    pub fn levels(&self) -> usize {
        self.universal_rules.len()
    }

    /// TODO
    pub fn get_rules(&self, level: usize, id: Option<usize>) -> impl Iterator<Item = &R> {
        let filtered = id
            .and_then(|id| {
                self.filtered_rules
                    .as_ref()
                    .and_then(|filter_map| filter_map[level].get(&id))
            })
            .map(|f| f.as_slice())
            .unwrap_or(&[]);
        let universal = &self.universal_rules[level];
        universal.iter().chain(filtered.iter())
    }

    /// Returns the universal rule groups in priority order.
    pub fn get_all_rules(&self) -> &[Vec<R>] {
        self.universal_rules.as_slice()
    }
}
