//#![cfg(feature = "unstable")]

#![allow(clippy::type_complexity)]

pub use super::Tree;
pub trait Biplate<To>
where
    Self: Sized + Clone + Eq,
    To: Sized + Clone + Eq,
{
    fn biplate(&self) -> (Tree<To>, Box<dyn Fn(Tree<To>) -> Self>);

    fn descend_bi(&self, op: Box<dyn Fn(To) -> To>) -> Self {
        todo!()
    }

    fn universe_bi(&self) -> Vec<Self> {
        todo!()
    }

    fn children_bi(&self) -> Vec<Self> {
        todo!()
    }

    fn transform_bi(&self, op: Box<dyn Fn(To) -> To>) -> Self {
        todo!()
    }
}

pub trait Uniplate
where
    Self: Sized + Clone + Eq,
{
    fn uniplate(&self) -> (Tree<Self>, Box<dyn Fn(Tree<Self>) -> Self>);

    fn descend(&self, op: Box<dyn Fn(Self) -> Self>) -> Vec<Self> {
        todo!()
    }

    fn universe(&self) -> Vec<Self> {
        todo!()
    }

    fn children(&self) -> Vec<Self> {
        todo!()
    }

    fn transform(&self, f: fn(Self) -> Self) {
        todo!()
    }

    fn cata<T>(&self, op: Box<dyn Fn(Self, Vec<T>) -> T>) -> T {
        todo!()
    }
}
