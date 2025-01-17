use std::rc::Rc;

use uniplate::{Tree, Uniplate};

/// Additional metadata associated with each tree node.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Meta {
    /// Transforms at and after this index should be applied.
    /// Those before it have already been tried with no change.
    clean_before: usize,
}

impl Meta {
    fn new() -> Self {
        Self { clean_before: 0 }
    }
}

/// Wraps around a tree and associates additional metadata with each node for rewriter optimisations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skel<T>
where
    T: Uniplate,
{
    pub node: Rc<T>,
    pub meta: Meta,
    pub children: Vec<Skel<T>>,
}

impl<T> Skel<T>
where
    T: Uniplate,
{
    /// Construct a new `Skel` wrapper around an entire tree.
    pub fn new(node: T) -> Self {
        Self::_new(Rc::new(node))
    }

    fn _new(node: Rc<T>) -> Self {
        Self {
            node: Rc::clone(&node),
            meta: Meta::new(),
            children: node
                .children()
                .into_iter()
                .map(|child| Skel::_new(Rc::new(child)))
                .collect(),
        }
    }
}

impl<T> Uniplate for Skel<T>
where
    T: Uniplate,
{
    // Implemented here as Uniplate doesn't yet support this struct
    fn uniplate(&self) -> (Tree<Self>, Box<dyn Fn(Tree<Self>) -> Self>) {
        let node = Rc::clone(&self.node);
        let meta = self.meta.clone();

        let tree = Tree::Many(im::Vector::from_iter(
            self.children.clone().into_iter().map(Tree::One),
        ));

        let ctx = Box::new(move |tr| {
            let Tree::Many(trs) = tr else { panic!() };
            let children = trs
                .into_iter()
                .map(|ch| {
                    let Tree::One(sk) = ch else { panic!() };
                    sk
                })
                .collect();
            Skel {
                node: node.clone(),
                meta: meta.clone(),
                children,
            }
        });

        (tree, ctx)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    use im::Vector;
    use uniplate::derive::Uniplate;
    use uniplate::Uniplate;

    #[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
    enum Expr {
        A,
        B,
        P(Box<Expr>, Box<Expr>),
    }

    #[test]
    fn test_skel_reconstruct() {
        let tree = Expr::P(
            Box::new(Expr::P(
                Box::new(Expr::P(Box::new(Expr::A), Box::new(Expr::A))),
                Box::new(Expr::B),
            )),
            Box::new(Expr::B),
        );

        let skel = Skel::new(tree.clone());
        let reconstructed = skel.cata(Arc::new(|sk: Skel<Expr>, chs| {
            sk.node.with_children(Vector::from(chs))
        }));

        // println!("Tree: {:?}", tree);
        // println!("Skel: {:?}", skel);
        assert_eq!(tree, reconstructed);
    }
}
