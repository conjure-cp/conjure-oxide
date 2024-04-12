//! Implementations of Uniplate and Biplate for common types
//!
//! This includes stdlib types as well as common collections
//!
//! Box types are excluded, and are inlined by the macro.

// NOTE (niklasdewally): my assumption is that we can do all this here, and that llvm will inline
// this and/or devirtualise the Box<dyn Fn()> when necessary to make this fast.
// https://users.rust-lang.org/t/why-box-dyn-fn-is-the-same-fast-as-normal-fn/96392

use crate::biplate::*;
use crate::Tree::*;

// Blanket implementation for known std unplatable types.
trait UnplatableMarker {}

impl<T: UnplatableMarker> Biplate<T> for T
where
    T: Clone + Eq + Uniplate + Sized + 'static,
{
    fn biplate(&self) -> (Tree<T>, Box<dyn Fn(Tree<T>) -> Self>) {
        let val = self.clone();
        (One(self.clone()), Box::new(move |_| val.clone()))
    }
}

impl<T: UnplatableMarker> Uniplate for T
where
    T: Clone + Eq + Sized + 'static,
{
    fn uniplate(&self) -> (Tree<Self>, Box<dyn Fn(Tree<Self>) -> Self>) {
        let val = self.clone();
        (One(self.clone()), Box::new(move |_| val.clone()))
    }
}

impl UnplatableMarker for i8 {}
impl UnplatableMarker for i16 {}
impl UnplatableMarker for i32 {}
impl UnplatableMarker for i64 {}
impl UnplatableMarker for i128 {}

impl UnplatableMarker for u8 {}
impl UnplatableMarker for u16 {}
impl UnplatableMarker for u32 {}
impl UnplatableMarker for u64 {}
impl UnplatableMarker for u128 {}

impl UnplatableMarker for String {}
impl UnplatableMarker for &str {}

/*****************************/
/*        Collections        */
/*****************************/

// Implement Biplate for collections by converting them to iterators.

// NOTE (niklasdewally):
// In the future / if someone understands trait hacks more, I would like to implement this for all
// From: IntoIter<Item=To>, not just these predefined collections.
// ..
// Rust cannot be sure that UnplatableMarker and IntoIterator are mutually exclusive, so thinks
// there are conflicting impls of Biplate if I try to do this generically.

impl<T> Biplate<T> for Vec<T>
where
    T: Clone + Eq + Uniplate + Sized + 'static,
{
    fn biplate(&self) -> (Tree<T>, Box<dyn Fn(Tree<T>) -> Self>) {
        todo!()
    }
}

impl<T> Uniplate for Vec<T>
where
    T: Clone + Eq + Uniplate + Sized + 'static,
{
    fn uniplate(&self) -> (Tree<Self>, Box<dyn Fn(Tree<Self>) -> Self>) {
        let val = self.clone();
        (Zero, Box::new(move |_| val.clone()))
    }
}
