pub trait Uniplate
where
    Self: Sized + Clone + Eq,
{
    #[allow(clippy::type_complexity)]
    fn uniplate(&self) -> (Vec<Self>, Box<dyn Fn(Vec<Self>) -> Self + '_>);

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

    /// Apply the given rule to all nodes bottom up.
    fn transform(&self, f: fn(Self) -> Self) -> Self {
        let (children, context) = self.uniplate();
        f(context(
            children.into_iter().map(|a| a.transform(f)).collect(),
        ))
    }

    /// Rewrite by applying a rule everywhere you can.
    fn rewrite(&self, f: fn(Self) -> Option<Self>) -> Self {
        let (children, context) = self.uniplate();
        let node: Self = context(children.into_iter().map(|a| a.rewrite(f)).collect());

        f(node.clone()).unwrap_or(node)
    }

    /// Perform a transformation on all the immediate children, then combine them back.
    /// This operation allows additional information to be passed downwards, and can be used to provide a top-down transformation.
    fn descend(&self, f: fn(Self) -> Self) -> Self {
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
            context(children)
        }))
    }
}
