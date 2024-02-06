pub trait Uniplate
where
    Self: Sized + Clone + Eq,
{
    fn uniplate(&self) -> (Vec<Self>, Box<dyn Fn(Vec<Self>) -> Self + '_>);

    /// Get the DIRECT children of a node.
    fn children(self) -> Vec<Self> {
        self.uniplate().0
    }

    /// Get all children of a node, including itself and all children.
    fn universe(self) -> Vec<Self> {
        let mut results = vec![self.clone()];
        for child in self.children() {
            results.append(&mut child.universe());
        }
        results
    }

    /// Apply the given rule to all nodes bottom up.
    fn transform(self, f: fn(Self) -> Self) -> Self {
        let (children, context) = self.uniplate();
        f(context(
            children.into_iter().map(|a| a.transform(f)).collect(),
        ))
    }

    fn rewrite(self, f: fn(Self) -> Option<Self>) -> Self {
        todo!()
    }

    /// Perform a transformation on all the immediate children, then combine them back.
    /// This operation allows additional information to be passed downwards, and can be used to provide a top-down transformation.
    fn descend(self, f: fn(Self) -> Self) -> Self {
        let (children, context) = self.uniplate();
        let children: Vec<Self> = children.into_iter().map(f).collect();

        context(children)
    }

    /// Perform a fold-like computation on each value, technically a paramorphism
    fn para<T>(self, op: fn(Self, Vec<T>) -> T) -> T {
        op(
            self.clone(),
            self.children().into_iter().map(|c| c.para(op)).collect(),
        )
    }

    fn zipper(self) -> Zipper<Self> {
        todo!()
    }
}

pub struct Zipper<T>
where
    T: Uniplate,
{
    hole: T,
    // TODO
}

impl<T: Uniplate> Zipper<T> {
    fn left(self) -> Option<Zipper<T>> {
        todo!();
    }

    fn right(self) -> Option<Zipper<T>> {
        todo!();
    }

    fn up(self) -> Option<Zipper<T>> {
        todo!();
    }

    fn down(self) -> Option<Zipper<T>> {
        todo!();
    }
}
