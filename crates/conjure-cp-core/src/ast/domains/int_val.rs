use crate::ast::{
    DeclarationKind, DomainOpError, Expression, FuncAttr, Literal, Metadata, Moo, PartitionAttr,
    Reference, RelAttr, ReturnType, SequenceAttr, Typeable,
    domains::{Int, MSetAttr, Range, SetAttr},
    eval_constant,
};
use crate::{bug, into_matrix_expr, matrix_expr};
use funcmap::{FuncMap, TryFuncMap};
use polyquine::Quine;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use uniplate::Uniplate;

/// A variable or expression appearing inside an int range of an unresolved domain;
/// E.g `int(1..x)`, `int(2, 4..(2*y))`, `set (minSize x) of int(1..5)`, etc
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine, Uniplate)]
#[path_prefix(conjure_cp::ast)]
#[biplate(to=Expression)]
#[biplate(to=Reference)]
pub enum IntVal {
    // For ergonomics, we use a type bigger than both Int and UInt so both fit;
    // Overflows are handled when resolving
    Const(i64),
    #[polyquine_skip]
    Reference(Reference),
    Expr(Moo<Expression>),
}

// ------------------------------------
// ------ Trait impls for IntVal ------
// ------------------------------------

impl<T> From<T> for IntVal
where
    T: Into<i64>,
{
    fn from(v: T) -> Self {
        IntVal::Const(v.into())
    }
}

impl TryFrom<IntVal> for Int {
    type Error = DomainOpError;

    fn try_from(value: IntVal) -> Result<Int, Self::Error> {
        match value {
            IntVal::Const(val) => val.try_into().map_err(|_| DomainOpError::OutOfBounds),
            _ => Err(DomainOpError::NotGround),
        }
    }
}

impl Display for IntVal {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            IntVal::Const(val) => write!(f, "{val}"),
            IntVal::Reference(re) => write!(f, "{re}"),
            IntVal::Expr(expr) => write!(f, "({expr})"),
        }
    }
}

impl std::ops::Neg for IntVal {
    type Output = IntVal;

    fn neg(self) -> Self::Output {
        match self {
            IntVal::Const(val) => IntVal::Const(-val),
            IntVal::Reference(re) => IntVal::Expr(Moo::new(Expression::Neg(
                Metadata::new(),
                Moo::new(re.into()),
            ))),
            IntVal::Expr(expr) => IntVal::Expr(Moo::new(Expression::Neg(Metadata::new(), expr))),
        }
    }
}

// ----------------------------------------
// ------ Core IntVal implementation ------
// ----------------------------------------

impl IntVal {
    pub fn new_const(val: Int) -> IntVal {
        IntVal::Const(val as i64)
    }

    pub fn new_ref(re: &Reference) -> Result<IntVal, DomainOpError> {
        match re.ptr.kind().deref() {
            DeclarationKind::ValueLetting(expr, _)
            | DeclarationKind::TemporaryValueLetting(expr) => match expr.return_type() {
                ReturnType::Int => Ok(IntVal::Reference(re.clone())),
                _ => Err(DomainOpError::WrongType),
            },
            DeclarationKind::Given(dom) => match dom.return_type() {
                ReturnType::Int => Ok(IntVal::Reference(re.clone())),
                _ => Err(DomainOpError::WrongType),
            },
            // TODO: I'm not sure if this is correct, see discussion #1890
            // Assume that for `y : int(1..x)`, where x is an induction variable, to be valid,
            // x must be an integer (e.g x : int(1..3)). I don't know how the generator expression
            // fits into this.
            DeclarationKind::Quantified(inner) => match inner.domain().return_type() {
                ReturnType::Int => Ok(IntVal::Reference(re.clone())),
                _ => Err(DomainOpError::WrongType),
            },
            // TODO: I'm not sure if this is correct, see discussion #1890
            // Assume that for `y : int(1..x)`, where `x <- expr`, to be valid,
            // x must be a collection of integers; E.g, `x <- {1, 2, 3}`.
            DeclarationKind::QuantifiedExpr(expr) => match expr.return_type().elem_type() {
                Some(ReturnType::Int) => Ok(IntVal::Reference(re.clone())),
                _ => Err(DomainOpError::WrongType),
            },
            DeclarationKind::Find(var) => match var.return_type() {
                ReturnType::Int => Ok(IntVal::Reference(re.clone())),
                _ => Err(DomainOpError::WrongType),
            },
            DeclarationKind::DomainLetting(_) => Err(DomainOpError::WrongType),
        }
    }

    pub fn new_expr(value: Moo<Expression>) -> Result<IntVal, DomainOpError> {
        if value.return_type() != ReturnType::Int {
            return Err(DomainOpError::WrongType);
        }
        Ok(IntVal::Expr(value))
    }

    pub fn resolve(&self) -> Result<Int, DomainOpError> {
        match self {
            IntVal::Const(value) => (*value).try_into().map_err(|_| DomainOpError::OutOfBounds),
            IntVal::Expr(expr) => eval_expr_to_int(expr).ok_or(DomainOpError::NotGround),
            IntVal::Reference(re) => match re.ptr.kind().deref() {
                DeclarationKind::ValueLetting(expr, _)
                | DeclarationKind::TemporaryValueLetting(expr) => {
                    eval_expr_to_int(expr).ok_or(DomainOpError::NotGround)
                }
                // If this is an int given we will be able to resolve it eventually, but not yet
                DeclarationKind::Given(_) => Err(DomainOpError::NotGround),
                DeclarationKind::Quantified(inner) => {
                    if let Some(generator) = inner.generator()
                        && let Some(expr) = generator.as_value_letting()
                    {
                        eval_expr_to_int(&expr).ok_or(DomainOpError::NotGround)
                    } else {
                        Err(DomainOpError::NotGround)
                    }
                }
                // Decision variables inside domains are unresolved until solving.
                DeclarationKind::Find(_) => Err(DomainOpError::NotGround),
                DeclarationKind::DomainLetting(_) | DeclarationKind::QuantifiedExpr(_) => bug!(
                    "Expected integer expression, given, or letting inside int domain; Got: {re}"
                ),
            },
        }
    }

    pub fn try_add<T: Into<Expression>>(self, rhs: T) -> Result<IntVal, DomainOpError> {
        let sum = Expression::Sum(
            Metadata::new(),
            Moo::new(matrix_expr!(self.try_into()?, rhs.into())),
        );
        Ok(IntVal::Expr(Moo::new(sum)))
    }

    pub fn try_sub<T: Into<Expression>>(self, rhs: T) -> Result<IntVal, DomainOpError> {
        let rhs_neg = Expression::Neg(Metadata::new(), Moo::new(rhs.into()));
        let sum = Expression::Sum(
            Metadata::new(),
            Moo::new(matrix_expr!(self.try_into()?, rhs_neg)),
        );
        Ok(IntVal::Expr(Moo::new(sum)))
    }
}

// ------------------------------------------
// ------ Expression-related miscellanea ----
// ------------------------------------------

impl Range<IntVal> {
    /// Generates the expression to compute the size of this range
    pub fn len_expr(self) -> Result<Expression, DomainOpError> {
        match self {
            Range::Single(a) => Ok(a.try_into()?),
            Range::Bounded(a, b) => {
                let neg_b = Expression::Neg(Metadata::new(), b.try_into()?);
                let sum_matr = into_matrix_expr!(vec![a.try_into()?, neg_b]);
                Ok(Expression::Sum(Metadata::new(), sum_matr.into()))
            }
            _ => Err(DomainOpError::Unbounded),
        }
    }

    /// Generates the expression to compute the size of a list of ranges
    pub fn len_expr_of(rngs: &[Range<IntVal>]) -> Result<Expression, DomainOpError> {
        let mut rng_sizes = Vec::with_capacity(rngs.len());
        for rng in rngs {
            rng_sizes.push(rng.clone().len_expr()?);
        }
        let rng_sizes = into_matrix_expr!(rng_sizes);
        Ok(Expression::Sum(Metadata::new(), rng_sizes.into()))
    }
}

fn eval_expr_to_int(expr: &Expression) -> Option<Int> {
    match eval_constant(expr)? {
        Literal::Int(v) => Some(v),
        _ => bug!("Expected integer expression, got: {expr}"),
    }
}

impl TryFrom<IntVal> for Moo<Expression> {
    type Error = DomainOpError;

    fn try_from(value: IntVal) -> Result<Self, Self::Error> {
        match value {
            IntVal::Const(val) => {
                let val: Int = val.try_into().map_err(|_| DomainOpError::OutOfBounds)?;
                Ok(Moo::new(val.into()))
            }
            IntVal::Reference(re) => Ok(Moo::new(re.into())),
            IntVal::Expr(expr) => Ok(expr),
        }
    }
}

impl TryFrom<IntVal> for Expression {
    type Error = DomainOpError;
    fn try_from(value: IntVal) -> Result<Self, Self::Error> {
        Ok(Moo::unwrap_or_clone(value.try_into()?))
    }
}

// --------------------------------------------------------------
// ------ Derive into / resolve for container types by macro ----
// --------------------------------------------------------------

macro_rules! impl_int_conversions {
    ($container:ident) => {
        impl From<$container<Int>> for $container<IntVal> {
            fn from(val: $container<Int>) -> Self {
                val.func_map(IntVal::from)
            }
        }

        impl TryFrom<$container<IntVal>> for $container<Int> {
            type Error = DomainOpError;

            fn try_from(val: $container<IntVal>) -> Result<Self, Self::Error> {
                val.try_func_map(IntVal::try_into)
            }
        }

        impl $container<IntVal> {
            // All inner types are either i64 or pointers so cloning should be relatively cheap;
            // so, for ergonomics, we pretend that `resolve` methods take a reference :)
            pub fn resolve(&self) -> Result<$container<Int>, DomainOpError> {
                self.clone().try_func_map(|x| IntVal::resolve(&x))
            }
        }
    };
}

macro_rules! impl_int_conversions_for {
    ($($container:ident),+ $(,)?) => {
        $(impl_int_conversions!($container);)+
    };
}

// To add a new type in the future:
// 1. Add #[derive(FuncMap, TryFuncMap)] to the container type or impl the traits yourself
// 2. Add the container type to the list below
impl_int_conversions_for!(
    Range,
    SetAttr,
    MSetAttr,
    FuncAttr,
    SequenceAttr,
    PartitionAttr,
    RelAttr
);
