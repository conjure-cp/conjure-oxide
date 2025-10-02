// NOTE
//
// we use a wrapper type over Arc, instead of just using Arc, so that we can implement traits on it
// (e.g. Uniplate, Serialize).
//
// As we are just using Arc for copy on write, not shared ownership, it is safe to break shared
// ownership in Moo's Uniplate implementation, by calling Arc::make_mut and Arc::new on modified
// values. In general, this is not safe for all Rc/Arc types, e.g. those that use Cell / RefCell
// internally.
//
// ~niklasdewally 13/08/25

use std::{collections::VecDeque, fmt::Display, ops::Deref, sync::Arc};

use polyquine::Quine;
use proc_macro2::TokenStream;
use serde::{Deserialize, Serialize};
use uniplate::{
    Biplate, Tree, Uniplate,
    impl_helpers::{transmute_if_same_type, try_transmute_if_same_type},
    spez::try_biplate_to,
};

/// A clone-on-write, reference counted pointer to an AST type.
///
/// Cloning values of this type will not clone the underlying value until it is modified, e.g.,
/// with [`Moo::make_mut`].
///
/// Unlike `Rc` and `Arc`, trait implementations on this type do not need to preserve shared
/// ownership - that is, two pointers that used to point to the same value may not do so after
/// calling a trait method on them. In particular, calling Uniplate methods may cause a
/// clone-on-write to occur.
///
/// **Note:** like Box` and `Rc`, methods on `Moo` are all associated functions, which means you
/// have to call them as, e.g. `Moo::make_mut(&value)` instead of `value.make_mut()`. This is so
/// that there are no conflicts with the inner type `T`, which this type dereferences to.
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Moo<T> {
    inner: Arc<T>,
}

impl<T: Quine> Quine for Moo<T> {
    fn ctor_tokens(&self) -> TokenStream {
        let inner = self.inner.as_ref().ctor_tokens();
        quote::quote! { ::conjure_cp::ast::Moo::new(#inner) }
    }
}

impl<T> Moo<T> {
    /// Constructs a new `Moo<T>`.
    pub fn new(value: T) -> Moo<T> {
        Moo {
            inner: Arc::new(value),
        }
    }
}

impl<T: Clone> Moo<T> {
    /// Makes a mutable reference into the given `Moo`.
    ///
    /// If there are other `Moo` pointers to the same allocation, then `make_mut` will `clone` the
    /// inner value to a new allocation to ensure unique ownership. This is also referred to as
    /// clone-on-write.
    pub fn make_mut(this: &mut Moo<T>) -> &mut T {
        Arc::make_mut(&mut this.inner)
    }

    /// If we have the only reference to T then unwrap it. Otherwise, clone T and return the clone.
    ///
    /// Assuming moo_t is of type `Moo<T>`, this function is functionally equivalent to
    /// `(*moo_t).clone()`, but will avoid cloning the inner value where possible.
    pub fn unwrap_or_clone(this: Moo<T>) -> T {
        Arc::unwrap_or_clone(this.inner)
    }
}

impl<T> AsRef<T> for Moo<T> {
    fn as_ref(&self) -> &T {
        self.inner.as_ref()
    }
}

impl<T> Deref for Moo<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}

impl<T> Uniplate for Moo<T>
where
    T: Uniplate,
{
    fn uniplate(
        &self,
    ) -> (
        uniplate::Tree<Self>,
        Box<dyn Fn(uniplate::Tree<Self>) -> Self>,
    ) {
        let this = Moo::clone(self);

        // do not need to preserve shared ownership, so treat this identically to values of the
        // inner type.
        let (tree, ctx) = try_biplate_to!((**self).clone(), Moo<T>);
        (
            Tree::Many(VecDeque::from([tree.clone()])),
            Box::new(move |x| {
                let Tree::Many(trees) = x else { panic!() };
                let new_tree = trees.into_iter().next().unwrap();
                let mut this = Moo::clone(&this);

                // Only update the pointer with the new value if the value has changed. Without
                // this check, writing to the pointer might trigger a clone on write, even
                // though the value inside the pointer remained the same.
                if new_tree != tree {
                    let this = Moo::make_mut(&mut this);
                    *this = ctx(new_tree)
                }

                this
            }),
        )
    }
}

impl<To, U> Biplate<To> for Moo<U>
where
    To: Uniplate,
    U: Uniplate + Biplate<To>,
{
    fn biplate(&self) -> (Tree<To>, Box<dyn Fn(Tree<To>) -> Self>) {
        if let Some(self_as_to) = transmute_if_same_type::<Self, To>(self) {
            // To = Self -> return self
            let tree = Tree::One(self_as_to.clone());
            let ctx = Box::new(move |x| {
                let Tree::One(self_as_to) = x else { panic!() };

                let self_as_self = try_transmute_if_same_type::<To, Self>(&self_as_to);

                Moo::clone(self_as_self)
            });

            (tree, ctx)
        } else {
            // To != Self -> return children of type To

            let this = Moo::clone(self);

            // Do not need to preserve shared ownership, so treat this identically to values of the
            // inner type.
            let (tree, ctx) = try_biplate_to!((**self).clone(), To);
            (
                Tree::Many(VecDeque::from([tree.clone()])),
                Box::new(move |x| {
                    let Tree::Many(trees) = x else { panic!() };
                    let new_tree = trees.into_iter().next().unwrap();
                    let mut this = Moo::clone(&this);

                    // Only update the pointer with the new value if the value has changed. Without
                    // this check, writing to the pointer might trigger a clone on write, even
                    // though the value inside the pointer remained the same.
                    if new_tree != tree {
                        let this = Moo::make_mut(&mut this);
                        *this = ctx(new_tree)
                    }

                    this
                }),
            )
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Moo<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Moo::new(T::deserialize(deserializer)?))
    }
}

impl<T: Serialize> Serialize for Moo<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        T::serialize(&**self, serializer)
    }
}

impl<T: Display> Display for Moo<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        (**self).fmt(f)
    }
}
