use crate::{
    ast::Metadata,
    ast::{Domain, Moo, Range, ReturnType},
    matrix_expr,
};

use super::{Expression, Literal, Typeable};

/// The possible kinds of associative-commutative (AC) operator.
///
/// AC operators take a single vector as input and are commonly used alongside comprehensions.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ACOperatorKind {
    And,
    Or,
    Product,
    Sum,
}

impl ACOperatorKind {
    /// Creates a new [Expression] of this AC operator kind with the given child expression.
    ///
    /// The child expression given should be of type matrix.
    pub fn as_expression(&self, child_expr: Expression) -> Expression {
        assert!(
            matches!(child_expr.return_type(), Some(ReturnType::Matrix(_))),
            "The child expression given to ACOperatorKind::to_expression should be of type matrix."
        );
        let box_expr = Moo::new(child_expr);
        match self {
            ACOperatorKind::And => Expression::And(Metadata::new(), box_expr),
            ACOperatorKind::Or => Expression::Or(Metadata::new(), box_expr),
            ACOperatorKind::Product => Expression::Product(Metadata::new(), box_expr),
            ACOperatorKind::Sum => Expression::Sum(Metadata::new(), box_expr),
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
    pub fn identity(&self) -> Literal {
        match self {
            ACOperatorKind::And => Literal::Bool(true),
            ACOperatorKind::Or => Literal::Bool(false),
            ACOperatorKind::Product => Literal::Int(1),
            ACOperatorKind::Sum => Literal::Int(0),
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
            matches!(guard_expr.return_type(), Some(ReturnType::Bool)),
            "The guard expression in a skipping operation should be type boolean."
        );

        match self {
            ACOperatorKind::And => {
                assert!(
                    matches!(tail_expr.return_type(), Some(ReturnType::Bool)),
                    "The tail expression in an and skipping operation should be type boolean."
                );
                let tail_expr_boxed = Moo::new(tail_expr);
                let guard_expr_boxed = Moo::new(guard_expr);
                Expression::Imply(Metadata::new(), guard_expr_boxed, tail_expr_boxed)
            }
            ACOperatorKind::Or => {
                assert!(
                    matches!(tail_expr.return_type(), Some(ReturnType::Bool)),
                    "The tail expression in an or skipping operation should be type boolean."
                );
                Expression::And(
                    Metadata::new(),
                    Moo::new(matrix_expr![guard_expr, tail_expr]),
                )
            }
            ACOperatorKind::Product => {
                assert!(
                    matches!(tail_expr.return_type(), Some(ReturnType::Int)),
                    "The tail expression in a product skipping operation should be type int."
                );
                let guard_expr_boxed = Moo::new(guard_expr);
                Expression::UnsafeIndex(
                    Metadata::new(),
                    Moo::new(
                        matrix_expr![Expression::Atomic(Metadata::new(),1.into()),tail_expr;Domain::Int(vec![Range::Bounded(0,1)])],
                    ),
                    vec![Expression::ToInt(Metadata::new(), guard_expr_boxed)],
                )
            }
            ACOperatorKind::Sum => {
                let guard_expr_boxed = Moo::new(guard_expr);
                assert!(
                    matches!(tail_expr.return_type(), Some(ReturnType::Int)),
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
        }
    }

    /// Gives the return type of the operator, and the return types its elements should be.
    pub fn return_type(&self) -> ReturnType {
        match self {
            ACOperatorKind::And | ACOperatorKind::Or => ReturnType::Bool,
            ACOperatorKind::Product | ACOperatorKind::Sum => ReturnType::Int,
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
