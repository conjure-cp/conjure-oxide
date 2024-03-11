use thiserror::Error;

#[derive(Debug, PartialEq, Eq, Error)]
pub enum UniplateError {
    #[error("Could not reconstruct node because wrong number of children was provided. Expected {0} children, got {1}.")]
    WrongNumberOfChildren(usize, usize),
}

pub trait Uniplate
where
    Self: Sized + Clone + Eq,
{
    /// The `uniplate` function. Takes a node and produces a tuple of `(children, context)`, where:
    /// - children is a list of the node's direct descendants of the same type
    /// - context is a function to reconstruct the original node with a new list of children
    ///
    /// ## Warning
    ///
    /// The number of children passed to context must be the same as the number of children in
    /// the original node.
    /// If the number of children given is different, context returns `UniplateError::NotEnoughChildren`
    #[allow(clippy::type_complexity)]
    fn uniplate(
        &self,
    ) -> (
        Vec<Self>,
        Box<dyn Fn(Vec<Self>) -> Result<Self, UniplateError> + '_>,
    );

    /// Get all children of a node, including itself and all children.
    fn universe(&self) -> Vec<Self> {
        let mut results = vec![self.clone()];
        for child in self.children() {
            results.append(&mut child.universe());
        }
        results
    }

    /// Get the DIRECT children of a node.
    fn children(&self) -> Vec<Self> {
        self.uniplate().0
    }

    /// Reconstruct this node with the given children
    ///
    /// ## Arguments
    /// - children - a vector of the same type and same size as self.children()
    fn with_children(&self, children: Vec<Self>) -> Result<Self, UniplateError> {
        let context = self.uniplate().1;
        context(children)
    }

    /// Apply the given rule to all nodes bottom up.
    fn transform(&self, f: fn(Self) -> Self) -> Result<Self, UniplateError> {
        let (children, context) = self.uniplate();

        let mut new_children: Vec<Self> = Vec::new();
        for ch in children {
            let new_ch = ch.transform(f)?;
            new_children.push(new_ch);
        }

        let transformed = context(new_children)?;
        Ok(f(transformed))
    }

    /// Rewrite by applying a rule everywhere you can.
    fn rewrite(&self, f: fn(Self) -> Option<Self>) -> Result<Self, UniplateError> {
        let (children, context) = self.uniplate();

        let mut new_children: Vec<Self> = Vec::new();
        for ch in children {
            let new_ch = ch.rewrite(f)?;
            new_children.push(new_ch);
        }

        let node: Self = context(new_children)?;
        Ok(f(node.clone()).unwrap_or(node))
    }

    /// Perform a transformation on all the immediate children, then combine them back.
    /// This operation allows additional information to be passed downwards, and can be used to provide a top-down transformation.
    fn descend(&self, f: fn(Self) -> Self) -> Result<Self, UniplateError> {
        let (children, context) = self.uniplate();
        let children: Vec<Self> = children.into_iter().map(f).collect();

        context(children)
    }

    /// Perform a fold-like computation on each value.
    ///
    /// Working from the bottom up, this applies the given callback function to each nested
    /// component.
    ///
    /// Unlike [`transform`](Uniplate::transform), this returns an arbitrary type, and is not
    /// limited to T -> T transformations. In other words, it can transform a type into a new
    /// one.
    ///
    /// The meaning of the callback function is the following:
    ///
    ///   f(element_to_fold, folded_children) -> folded_element
    ///
    fn fold<T>(&self, op: fn(Self, Vec<T>) -> T) -> T {
        op(
            self.clone(),
            self.children().into_iter().map(|c| c.fold(op)).collect(),
        )
    }

    /// Get the nth one holed context.
    ///
    /// A uniplate context for type T has holes where all the nested T's should be.
    /// This is encoded as a function Vec<T> -> T.
    ///
    /// On the other hand, the nth one-holed context has only one hole where the nth nested
    /// instance of T would be.
    ///
    /// Eg. for some type:
    /// ```ignore
    /// enum Expr {
    ///     F(A,Expr,A,Expr,A),
    ///     G(Expr,A,A)
    /// }
    /// ```
    ///
    /// The 1st one-holed context of `F` (using 0-indexing) would be:
    /// ```ignore
    /// |HOLE| F(a,b,c,HOLE,e)
    /// ```
    ///
    /// Used primarily in the implementation of Zippers.
    fn one_holed_context(&self, n: usize) -> Option<Box<dyn Fn(Self) -> Self + '_>> {
        let (children, context) = self.uniplate();
        let number_of_elems = children.len();

        if n >= number_of_elems {
            return None;
        }

        Some(Box::new(move |x| {
            let mut children = children.clone();
            children[n] = x;
            #[allow(clippy::unwrap_used)]
            // We are directly replacing a child so there can't be an error
            context(children).unwrap()
        }))
    }
}
