use crate::{
    ast::Metadata,
    ast::{Domain, Moo, Range, ReturnType},
    bug, matrix_expr,
};

use super::{Expression, Literal, Typeable};
use serde::{Deserialize, Serialize};

/// The possible kinds of associative-commutative (AC) operator.
///
/// AC operators take a single vector as input and are commonly used alongside comprehensions.
///
/// `Min`/`Max` are included here for the sole purpose of tagging a comprehension's
/// [`skip_operator`](super::comprehension::Comprehension::skip_operator) so that a symbolic guard
/// inside `min([... | ...])`/`max([... | ...])` can be lowered correctly by the native
/// comprehension expander -- unlike `And`/`Or`/`Sum`/`Product`, they have no universal identity
/// element (the "safe value to substitute for a guarded-out element" depends on the element's own
/// domain), so [`identity`](Self::identity) is intentionally unreachable for them; callers that
/// might see a `Min`/`Max` operator must check for that first (see
/// `expand_native.rs`'s guard on the identity-dropping optimisation for the only current example).
/// They are deliberately not wired into `TryFrom<&Expression>` or the AC-comprehension merge/
/// via-solver machinery, which only ever handles the four true AC operators.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ACOperatorKind {
    And,
    Or,
    Product,
    Sum,
    Min,
    Max,
}

impl ACOperatorKind {
    /// Creates a new [Expression] of this AC operator kind with the given child expression.
    ///
    /// The child expression given should be of type matrix.
    pub fn as_expression(&self, child_expr: Expression) -> Expression {
        assert!(
            matches!(child_expr.return_type(), ReturnType::Matrix(_)),
            "The child expression given to ACOperatorKind::to_expression should be of type matrix."
        );
        let box_expr = Moo::new(child_expr);
        match self {
            ACOperatorKind::And => Expression::And(Metadata::new(), box_expr),
            ACOperatorKind::Or => Expression::Or(Metadata::new(), box_expr),
            ACOperatorKind::Product => Expression::Product(Metadata::new(), box_expr),
            ACOperatorKind::Sum => Expression::Sum(Metadata::new(), box_expr),
            ACOperatorKind::Min => Expression::Min(Metadata::new(), box_expr),
            ACOperatorKind::Max => Expression::Max(Metadata::new(), box_expr),
        }
    }

    /// Returns the identity element of this operation.
    ///
    /// # Example
    ///
    /// ```
    /// use conjure_cp_core::ast::{ac_operators::ACOperatorKind,Literal};
    ///
    /// let identity = ACOperatorKind::And.identity();
    /// assert_eq!(identity,Literal::Bool(true));
    /// ```
    ///
    /// # Panics
    ///
    /// `Min`/`Max` have no universal identity element -- see the type-level doc comment. Callers
    /// must not call this for those two variants.
    pub fn identity(&self) -> Literal {
        match self {
            ACOperatorKind::And => Literal::Bool(true),
            ACOperatorKind::Or => Literal::Bool(false),
            ACOperatorKind::Product => Literal::Int(1),
            ACOperatorKind::Sum => Literal::Int(0),
            ACOperatorKind::Min | ACOperatorKind::Max => {
                bug!("ACOperatorKind::{self:?} has no universal identity element")
            }
        }
    }

    /// Given some guard and tail expressions, constructs the skipping operator for this operation.
    ///
    /// The skipping operator is operator that takes some boolean guard expression b and some tail
    /// expression x. If b is true, then it evaluates to x, otherwise it evaluates to the identity
    /// element.
    ///
    /// # Usage
    ///
    /// This can be used to add guards to elements of AC operations. In the example model below, we
    /// only want to multiply y*z by 2 if multiplyByTwo is true:
    ///
    /// ```plain
    /// find multiplyByTwo: bool
    /// find x: int(1..5)
    /// find y: int(1..5)
    /// find z: int(1..5)
    ///
    /// such that
    ///  
    /// x = product([y,z,[1,x;int(0..1)][toInt(b)]])
    /// ```
    ///
    /// `[1,x;int(0..1)][toInt(b)]` is the skipping operator for product.
    ///
    /// This method constructs the skipping operator, substituting in the given expressions for b
    /// and x.
    pub fn make_skip_operation(&self, guard_expr: Expression, tail_expr: Expression) -> Expression {
        assert!(
            matches!(guard_expr.return_type(), ReturnType::Bool),
            "The guard expression in a skipping operation should be type boolean."
        );

        match self {
            ACOperatorKind::And => {
                assert!(
                    matches!(tail_expr.return_type(), ReturnType::Bool),
                    "The tail expression in an and skipping operation should be type boolean."
                );
                let tail_expr_boxed = Moo::new(tail_expr);
                let guard_expr_boxed = Moo::new(guard_expr);
                Expression::Imply(Metadata::new(), guard_expr_boxed, tail_expr_boxed)
            }
            ACOperatorKind::Or => {
                assert!(
                    matches!(tail_expr.return_type(), ReturnType::Bool),
                    "The tail expression in an or skipping operation should be type boolean."
                );
                Expression::And(
                    Metadata::new(),
                    Moo::new(matrix_expr![guard_expr, tail_expr]),
                )
            }
            ACOperatorKind::Product => {
                assert!(
                    matches!(tail_expr.return_type(), ReturnType::Int),
                    "The tail expression in a product skipping operation should be type int."
                );
                let guard_expr_boxed = Moo::new(guard_expr);
                Expression::UnsafeIndex(
                    Metadata::new(),
                    Moo::new(
                        matrix_expr![Expression::Atomic(Metadata::new(),1.into()),tail_expr;Domain::int(vec![Range::Bounded(0,1)])],
                    ),
                    vec![Expression::ToInt(Metadata::new(), guard_expr_boxed)],
                )
            }
            ACOperatorKind::Sum => {
                let guard_expr_boxed = Moo::new(guard_expr);
                assert!(
                    matches!(tail_expr.return_type(), ReturnType::Int),
                    "The tail expression in a sum skipping operation should be type int."
                );
                Expression::Product(
                    Metadata::new(),
                    Moo::new(matrix_expr![
                        Expression::ToInt(Metadata::new(), guard_expr_boxed),
                        tail_expr
                    ]),
                )
            }
            ACOperatorKind::Min | ACOperatorKind::Max => {
                bug!(
                    "ACOperatorKind::{self:?} has no universal identity element, so make_skip_operation \
                     cannot build a skip operation for it -- use make_min_max_skip_operation instead, \
                     which takes an explicit skip value"
                )
            }
        }
    }

    /// The min/max equivalent of [`make_skip_operation`](Self::make_skip_operation): `self` must
    /// be `Min` or `Max`. Unlike the four true AC operators, min/max have no value that's always
    /// safe to substitute for a guarded-out element -- callers must supply one themselves (`skip_value`),
    /// since this method has no way to know it: by the time a per-branch skip operation is being
    /// built, `tail_expr` is already the branch-specific (and so potentially far too narrow)
    /// result, not the comprehension's general return-expression domain the skip value should
    /// come from. Any domain bound safe for every element being aggregated works (e.g. the
    /// element's declared domain's max, for `min`, or min, for `max`) -- including it can never
    /// change a min/max computed over at least one *included* real element, since it is never more
    /// extreme than any value the element could actually take.
    pub fn make_min_max_skip_operation(
        &self,
        guard_expr: Expression,
        tail_expr: Expression,
        skip_value: Literal,
    ) -> Expression {
        assert!(
            matches!(self, ACOperatorKind::Min | ACOperatorKind::Max),
            "make_min_max_skip_operation is only valid for ACOperatorKind::Min/Max."
        );
        assert!(
            matches!(guard_expr.return_type(), ReturnType::Bool),
            "The guard expression in a skipping operation should be type boolean."
        );
        assert!(
            matches!(tail_expr.return_type(), ReturnType::Int),
            "The tail expression in a min/max skipping operation should be type int."
        );
        let guard_expr_boxed = Moo::new(guard_expr);
        Expression::UnsafeIndex(
            Metadata::new(),
            Moo::new(
                matrix_expr![Expression::Atomic(Metadata::new(), skip_value.into()), tail_expr; Domain::int(vec![Range::Bounded(0, 1)])],
            ),
            vec![Expression::ToInt(Metadata::new(), guard_expr_boxed)],
        )
    }

    /// Gives the return type of the operator, and the return types its elements should be.
    pub fn return_type(&self) -> ReturnType {
        match self {
            ACOperatorKind::And | ACOperatorKind::Or => ReturnType::Bool,
            ACOperatorKind::Product
            | ACOperatorKind::Sum
            | ACOperatorKind::Min
            | ACOperatorKind::Max => ReturnType::Int,
        }
    }
}

impl TryFrom<&Expression> for ACOperatorKind {
    type Error = ();
    fn try_from(expr: &Expression) -> Result<Self, Self::Error> {
        match expr {
            Expression::And(_, _) => Ok(ACOperatorKind::And),
            Expression::Or(_, _) => Ok(ACOperatorKind::Or),
            Expression::Product(_, _) => Ok(ACOperatorKind::Product),
            Expression::Sum(_, _) => Ok(ACOperatorKind::Sum),
            _ => Err(()),
        }
    }
}

impl TryFrom<Expression> for ACOperatorKind {
    type Error = ();

    fn try_from(value: Expression) -> Result<Self, Self::Error> {
        TryFrom::try_from(&value)
    }
}

impl TryFrom<Box<Expression>> for ACOperatorKind {
    type Error = ();

    fn try_from(value: Box<Expression>) -> Result<Self, Self::Error> {
        TryFrom::try_from(value.as_ref())
    }
}
