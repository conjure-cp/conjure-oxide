#![allow(dead_code)]
use std::sync::Arc;

use uniplate::zipper::Zipper;

use crate::ast::Expression;

/// Traverses expressions in a root expression tree, but not into nested scopes.
///
/// Same types and usage as `Biplate::contexts_bi`.
pub(super) fn expression_ctx(
    root_expression: Expression,
) -> impl Iterator<Item = (Expression, Arc<dyn Fn(Expression) -> Expression>)> {
    ExpressionCtx {
        zipper: SubmodelZipper {
            inner: Zipper::new(root_expression),
        },
        done: false,
    }
}

/// A zipper that traverses over the current expression tree and does not traverse into nested
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
        // Do not enter comprehensions, which have their own local symbol table.
        if matches!(self.inner.focus(), Expression::Comprehension(_, _)) {
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

pub struct ExpressionCtx {
    zipper: SubmodelZipper,
    done: bool,
}

impl Iterator for ExpressionCtx {
    type Item = (Expression, Arc<dyn Fn(Expression) -> Expression>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let node = self.zipper.focus().clone();
        let zipper = self.zipper.clone();

        #[allow(clippy::arc_with_non_send_sync)]
        let ctx = Arc::new(move |x| {
            let mut zipper2 = zipper.clone();
            *zipper2.focus_mut() = x;
            zipper2.rebuild_root()
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
