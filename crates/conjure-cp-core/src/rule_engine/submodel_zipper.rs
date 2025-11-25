#![allow(dead_code)]
use std::sync::Arc;

use uniplate::zipper::Zipper;

use crate::ast::{Expression, SubModel};

/// Traverses expressions in this sub-model, but not into inner sub-models.
///
/// Same types and usage as `Biplate::contexts_bi`.
pub(super) fn submodel_ctx(
    m: SubModel,
) -> impl Iterator<Item = (Expression, Arc<dyn Fn(Expression) -> SubModel>)> {
    SubmodelCtx {
        zipper: SubmodelZipper {
            inner: Zipper::new(m.root().clone()),
        },
        submodel: m.clone(),
        done: false,
    }
}

/// A zipper that traverses over the current submodel only, and does not traverse into nested
/// scopes.
#[derive(Clone)]
#[doc(hidden)]
pub struct SubmodelZipper {
    inner: Zipper<Expression>,
}

impl SubmodelZipper {
    #[doc(hidden)]
    pub fn go_left(&mut self) -> Option<()> {
        self.inner.go_left()
    }

    #[doc(hidden)]
    pub fn go_right(&mut self) -> Option<()> {
        self.inner.go_right()
    }

    #[doc(hidden)]
    pub fn go_up(&mut self) -> Option<()> {
        self.inner.go_up()
    }

    #[doc(hidden)]
    pub fn rebuild_root(self) -> Expression {
        self.inner.rebuild_root()
    }

    #[doc(hidden)]
    pub fn go_down(&mut self) -> Option<()> {
        // do not enter things that create new submodels
        if matches!(
            self.inner.focus(),
            Expression::Scope(_, _) | Expression::Comprehension(_, _)
        ) {
            None
        } else {
            self.inner.go_down()
        }
    }

    #[doc(hidden)]
    pub fn focus(&self) -> &Expression {
        self.inner.focus()
    }

    #[doc(hidden)]
    pub fn replace_focus(&mut self, new_focus: Expression) -> Expression {
        self.inner.replace_focus(new_focus)
    }

    #[doc(hidden)]
    pub fn focus_mut(&mut self) -> &mut Expression {
        self.inner.focus_mut()
    }

    #[doc(hidden)]
    pub fn new(root_expression: Expression) -> Self {
        SubmodelZipper {
            inner: Zipper::new(root_expression),
        }
    }
}

pub struct SubmodelCtx {
    zipper: SubmodelZipper,
    submodel: SubModel,
    done: bool,
}

impl Iterator for SubmodelCtx {
    type Item = (Expression, Arc<dyn Fn(Expression) -> SubModel>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let node = self.zipper.focus().clone();
        let submodel = self.submodel.clone();
        let zipper = self.zipper.clone();

        #[allow(clippy::arc_with_non_send_sync)]
        let ctx = Arc::new(move |x| {
            let mut zipper2 = zipper.clone();
            *zipper2.focus_mut() = x;
            let root = zipper2.rebuild_root();
            let mut submodel2 = submodel.clone();
            submodel2.replace_root(root);
            submodel2
        });

        // prepare iterator for next element.
        // try moving down or right. if we can't move up the tree until we can move right.
        if self.zipper.go_down().is_none() {
            while self.zipper.go_right().is_none() {
                if self.zipper.go_up().is_none() {
                    // at the top again, so this will be the last time we return a node
                    self.done = true;
                    break;
                };
            }
        }

        Some((node, ctx))
    }
}
