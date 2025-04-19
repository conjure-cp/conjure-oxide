use std::collections::VecDeque;
use std::fmt::{Display, Formatter};
use std::sync::Arc;

use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::ast::literals::AbstractLiteral;
use crate::ast::literals::Literal;
use crate::ast::pretty::{pretty_expressions_as_top_level, pretty_vec};
use crate::ast::symbol_table::SymbolTable;
use crate::ast::Atom;
use crate::ast::Name;
use crate::ast::ReturnType;
use crate::bug;
use crate::metadata::Metadata;
use enum_compatability_macro::document_compatibility;
use uniplate::derive::Uniplate;
use uniplate::{Biplate, Uniplate as _};

use super::comprehension::Comprehension;
use super::records::RecordValue;
use super::{Domain, Range, SubModel, Typeable};

/// Represents different types of expressions used to define rules and constraints in the model.
///
/// The `Expression` enum includes operations, constants, and variable references
/// used to build rules and conditions for the model.
#[document_compatibility]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Uniplate)]
#[uniplate(walk_into=[Atom,SubModel,AbstractLiteral<Expression>,Comprehension])]
#[biplate(to=Metadata)]
#[biplate(to=Atom)]
#[biplate(to=Name,walk_into=[Atom])]
#[biplate(to=Vec<Expression>)]
#[biplate(to=Option<Expression>)]
#[biplate(to=SubModel,walk_into=[Comprehension])]
#[biplate(to=Comprehension)]
#[biplate(to=AbstractLiteral<Expression>)]
#[biplate(to=AbstractLiteral<Literal>,walk_into=[Atom])]
#[biplate(to=RecordValue<Expression>,walk_into=[AbstractLiteral<Expression>])]
#[biplate(to=RecordValue<Literal>,walk_into=[Atom,Literal,AbstractLiteral<Literal>,AbstractLiteral<Expression>])]
#[biplate(to=Literal,walk_into=[Atom])]
pub enum Expression {
    AbstractLiteral(Metadata, AbstractLiteral<Expression>),
    /// The top of the model
    Root(Metadata, Vec<Expression>),

    /// An expression representing "A is valid as long as B is true"
    /// Turns into a conjunction when it reaches a boolean context
    Bubble(Metadata, Box<Expression>, Box<Expression>),

    /// A comprehension.
    ///
    /// The inside of the comprehension opens a new scope.
    Comprehension(Metadata, Box<Comprehension>),

    /// Defines dominance ("Solution A is preferred over Solution B")
    DominanceRelation(Metadata, Box<Expression>),
    /// `fromSolution(name)` - Used in dominance relation definitions
    FromSolution(Metadata, Box<Expression>),

    Atomic(Metadata, Atom),

    /// A matrix index.
    ///
    /// Defined iff the indices are within their respective index domains.
    #[compatible(JsonInput)]
    UnsafeIndex(Metadata, Box<Expression>, Vec<Expression>),

    /// A safe matrix index.
    ///
    /// See [`Expression::UnsafeIndex`]
    SafeIndex(Metadata, Box<Expression>, Vec<Expression>),

    /// A matrix slice: `a[indices]`.
    ///
    /// One of the indicies may be `None`, representing the dimension of the matrix we want to take
    /// a slice of. For example, for some 3d matrix a, `a[1,..,2]` has the indices
    /// `Some(1),None,Some(2)`.
    ///
    /// It is assumed that the slice only has one "wild-card" dimension and thus is 1 dimensional.
    ///
    /// Defined iff the defined indices are within their respective index domains.
    #[compatible(JsonInput)]
    UnsafeSlice(Metadata, Box<Expression>, Vec<Option<Expression>>),

    /// A safe matrix slice: `a[indices]`.
    ///
    /// See [`Expression::UnsafeSlice`].
    SafeSlice(Metadata, Box<Expression>, Vec<Option<Expression>>),

    /// `inDomain(x,domain)` iff `x` is in the domain `domain`.
    ///
    /// This cannot be constructed from Essence input, nor passed to a solver: this expression is
    /// mainly used during the conversion of `UnsafeIndex` and `UnsafeSlice` to `SafeIndex` and
    /// `SafeSlice` respectively.
    InDomain(Metadata, Box<Expression>, Domain),

    Scope(Metadata, Box<SubModel>),

    /// `|x|` - absolute value of `x`
    #[compatible(JsonInput)]
    Abs(Metadata, Box<Expression>),

    /// `sum(<vec_expr>)`
    #[compatible(JsonInput)]
    Sum(Metadata, Box<Expression>),

    /// `a * b * c * ...`
    #[compatible(JsonInput)]
    Product(Metadata, Vec<Expression>),

    /// `min(<vec_expr>)`
    #[compatible(JsonInput)]
    Min(Metadata, Box<Expression>),

    /// `max(<vec_expr>)`
    #[compatible(JsonInput)]
    Max(Metadata, Box<Expression>),

    /// `not(a)`
    #[compatible(JsonInput, SAT)]
    Not(Metadata, Box<Expression>),

    /// `or(<vec_expr>)`
    #[compatible(JsonInput, SAT)]
    Or(Metadata, Box<Expression>),

    /// `and(<vec_expr>)`
    #[compatible(JsonInput, SAT)]
    And(Metadata, Box<Expression>),

    /// Ensures that `a->b` (material implication).
    #[compatible(JsonInput)]
    Imply(Metadata, Box<Expression>, Box<Expression>),

    #[compatible(JsonInput)]
    Eq(Metadata, Box<Expression>, Box<Expression>),

    #[compatible(JsonInput)]
    Neq(Metadata, Box<Expression>, Box<Expression>),

    #[compatible(JsonInput)]
    Geq(Metadata, Box<Expression>, Box<Expression>),

    #[compatible(JsonInput)]
    Leq(Metadata, Box<Expression>, Box<Expression>),

    #[compatible(JsonInput)]
    Gt(Metadata, Box<Expression>, Box<Expression>),

    #[compatible(JsonInput)]
    Lt(Metadata, Box<Expression>, Box<Expression>),

    /// Division after preventing division by zero, usually with a bubble
    SafeDiv(Metadata, Box<Expression>, Box<Expression>),

    /// Division with a possibly undefined value (division by 0)
    #[compatible(JsonInput)]
    UnsafeDiv(Metadata, Box<Expression>, Box<Expression>),

    /// Modulo after preventing mod 0, usually with a bubble
    SafeMod(Metadata, Box<Expression>, Box<Expression>),

    /// Modulo with a possibly undefined value (mod 0)
    #[compatible(JsonInput)]
    UnsafeMod(Metadata, Box<Expression>, Box<Expression>),

    /// Negation: `-x`
    #[compatible(JsonInput)]
    Neg(Metadata, Box<Expression>),

    /// Unsafe power`x**y` (possibly undefined)
    ///
    /// Defined when (X!=0 \\/ Y!=0) /\ Y>=0
    #[compatible(JsonInput)]
    UnsafePow(Metadata, Box<Expression>, Box<Expression>),

    /// `UnsafePow` after preventing undefinedness
    SafePow(Metadata, Box<Expression>, Box<Expression>),

    /// `allDiff(<vec_expr>)`
    #[compatible(JsonInput)]
    AllDiff(Metadata, Box<Expression>),

    /// Binary subtraction operator
    ///
    /// This is a parser-level construct, and is immediately normalised to `Sum([a,-b])`.
    #[compatible(JsonInput)]
    Minus(Metadata, Box<Expression>, Box<Expression>),

    /// Ensures that x=|y| i.e. x is the absolute value of y.
    ///
    /// Low-level Minion constraint.
    ///
    /// # See also
    ///
    /// + [Minion documentation](https://minion-solver.readthedocs.io/en/stable/usage/constraints.html#abs)
    #[compatible(Minion)]
    FlatAbsEq(Metadata, Atom, Atom),

    /// Ensures that `alldiff([a,b,...])`.
    ///
    /// Low-level Minion constraint.
    ///
    /// # See also
    ///
    /// + [Minion documentation](https://minion-solver.readthedocs.io/en/stable/usage/constraints.html#alldiff)
    #[compatible(Minion)]
    FlatAllDiff(Metadata, Vec<Atom>),

    /// Ensures that sum(vec) >= x.
    ///
    /// Low-level Minion constraint.
    ///
    /// # See also
    ///
    /// + [Minion documentation](https://minion-solver.readthedocs.io/en/stable/usage/constraints.html#sumgeq)
    #[compatible(Minion)]
    FlatSumGeq(Metadata, Vec<Atom>, Atom),

    /// Ensures that sum(vec) <= x.
    ///
    /// Low-level Minion constraint.
    ///
    /// # See also
    ///
    /// + [Minion documentation](https://minion-solver.readthedocs.io/en/stable/usage/constraints.html#sumleq)
    #[compatible(Minion)]
    FlatSumLeq(Metadata, Vec<Atom>, Atom),

    /// `ineq(x,y,k)` ensures that x <= y + k.
    ///
    /// Low-level Minion constraint.
    ///
    /// # See also
    ///
    /// + [Minion documentation](https://minion-solver.readthedocs.io/en/stable/usage/constraints.html#ineq)
    #[compatible(Minion)]
    FlatIneq(Metadata, Atom, Atom, Literal),

    /// `w-literal(x,k)` ensures that x == k, where x is a variable and k a constant.
    ///
    /// Low-level Minion constraint.
    ///
    /// This is a low-level Minion constraint and you should probably use Eq instead. The main use
    /// of w-literal is to convert boolean variables to constraints so that they can be used inside
    /// watched-and and watched-or.
    ///
    /// # See also
    ///
    /// + [Minion documentation](https://minion-solver.readthedocs.io/en/stable/usage/constraints.html#minuseq)
    /// + `rules::minion::boolean_literal_to_wliteral`.
    #[compatible(Minion)]
    FlatWatchedLiteral(Metadata, Name, Literal),

    /// `weightedsumleq(cs,xs,total)` ensures that cs.xs <= total, where cs.xs is the scalar dot
    /// product of cs and xs.
    ///
    /// Low-level Minion constraint.
    ///
    /// Represents a weighted sum of the form `ax + by + cz + ...`
    ///
    /// # See also
    ///
    /// + [Minion
    /// documentation](https://minion-solver.readthedocs.io/en/stable/usage/constraints.html#weightedsumleq)
    FlatWeightedSumLeq(Metadata, Vec<Literal>, Vec<Atom>, Atom),

    /// `weightedsumgeq(cs,xs,total)` ensures that cs.xs >= total, where cs.xs is the scalar dot
    /// product of cs and xs.
    ///
    /// Low-level Minion constraint.
    ///
    /// Represents a weighted sum of the form `ax + by + cz + ...`
    ///
    /// # See also
    ///
    /// + [Minion
    /// documentation](https://minion-solver.readthedocs.io/en/stable/usage/constraints.html#weightedsumleq)
    FlatWeightedSumGeq(Metadata, Vec<Literal>, Vec<Atom>, Atom),

    /// Ensures that x =-y, where x and y are atoms.
    ///
    /// Low-level Minion constraint.
    ///
    /// # See also
    ///
    /// + [Minion documentation](https://minion-solver.readthedocs.io/en/stable/usage/constraints.html#minuseq)
    #[compatible(Minion)]
    FlatMinusEq(Metadata, Atom, Atom),

    /// Ensures that x*y=z.
    ///
    /// Low-level Minion constraint.
    ///
    /// # See also
    ///
    /// + [Minion documentation](https://minion-solver.readthedocs.io/en/stable/usage/constraints.html#product)
    #[compatible(Minion)]
    FlatProductEq(Metadata, Atom, Atom, Atom),

    /// Ensures that floor(x/y)=z. Always true when y=0.
    ///
    /// Low-level Minion constraint.
    ///
    /// # See also
    ///
    /// + [Minion documentation](https://minion-solver.readthedocs.io/en/stable/usage/constraints.html#div_undefzero)
    #[compatible(Minion)]
    MinionDivEqUndefZero(Metadata, Atom, Atom, Atom),

    /// Ensures that x%y=z. Always true when y=0.
    ///
    /// Low-level Minion constraint.
    ///
    /// # See also
    ///
    /// + [Minion documentation](https://minion-solver.readthedocs.io/en/stable/usage/constraints.html#mod_undefzero)
    #[compatible(Minion)]
    MinionModuloEqUndefZero(Metadata, Atom, Atom, Atom),

    /// Ensures that `x**y = z`.
    ///
    /// Low-level Minion constraint.
    ///
    /// This constraint is false when `y<0` except for `1**y=1` and `(-1)**y=z` (where z is 1 if y
    /// is odd and z is -1 if y is even).
    ///
    /// # See also
    ///
    /// + [Github comment about `pow` semantics](https://github.com/minion/minion/issues/40#issuecomment-2595914891)
    /// + [Minion documentation](https://minion-solver.readthedocs.io/en/stable/usage/constraints.html#pow)
    MinionPow(Metadata, Atom, Atom, Atom),

    /// `reify(constraint,r)` ensures that r=1 iff `constraint` is satisfied, where r is a 0/1
    /// variable.
    ///
    /// Low-level Minion constraint.
    ///
    /// # See also
    ///
    ///  + [Minion documentation](https://minion-solver.readthedocs.io/en/stable/usage/constraints.html#reify)
    #[compatible(Minion)]
    MinionReify(Metadata, Box<Expression>, Atom),

    /// `reifyimply(constraint,r)` ensures that `r->constraint`, where r is a 0/1 variable.
    /// variable.
    ///
    /// Low-level Minion constraint.
    ///
    /// # See also
    ///
    ///  + [Minion documentation](https://minion-solver.readthedocs.io/en/stable/usage/constraints.html#reifyimply)
    #[compatible(Minion)]
    MinionReifyImply(Metadata, Box<Expression>, Atom),

    /// `w-inintervalset(x, [a1,a2, b1,b2, … ])` ensures that the value of x belongs to one of the
    /// intervals {a1,…,a2}, {b1,…,b2} etc.
    ///
    /// The list of intervals must be given in numerical order.
    ///
    /// Low-level Minion constraint.
    ///
    /// # See also
    ///
    ///  + [Minion documentation](https://minion-solver.readthedocs.io/en/stable/usage/constraints.html#w-inintervalset)
    #[compatible(Minion)]
    MinionWInIntervalSet(Metadata, Atom, Vec<i32>),

    /// `element_one(vec, i, e)` specifies that `vec[i] = e`. This implies that i is
    /// in the range `[1..len(vec)]`.
    ///
    /// Low-level Minion constraint.
    ///
    /// # See also
    ///
    ///  + [Minion documentation](https://minion-solver.readthedocs.io/en/stable/usage/constraints.html#element_one)
    #[compatible(Minion)]
    MinionElementOne(Metadata, Vec<Atom>, Atom, Atom),

    /// Declaration of an auxiliary variable.
    ///
    /// As with Savile Row, we semantically distinguish this from `Eq`.
    #[compatible(Minion)]
    AuxDeclaration(Metadata, Name, Box<Expression>),
}

fn expr_vec_to_domain_i32(
    exprs: &[Expression],
    op: fn(i32, i32) -> Option<i32>,
    vars: &SymbolTable,
) -> Option<Domain> {
    let domains: Vec<Option<_>> = exprs.iter().map(|e| e.domain_of(vars)).collect();
    domains
        .into_iter()
        .reduce(|a, b| a.and_then(|x| b.and_then(|y| x.apply_i32(op, &y))))
        .flatten()
}
fn expr_vec_lit_to_domain_i32(
    e: &Expression,
    op: fn(i32, i32) -> Option<i32>,
    vars: &SymbolTable,
) -> Option<Domain> {
    let exprs = e.clone().unwrap_list()?;
    expr_vec_to_domain_i32(&exprs, op, vars)
}

// Returns none if unbounded
fn range_vec_bounds_i32(ranges: &Vec<Range<i32>>) -> Option<(i32, i32)> {
    let mut min = i32::MAX;
    let mut max = i32::MIN;
    for r in ranges {
        match r {
            Range::Single(i) => {
                if *i < min {
                    min = *i;
                }
                if *i > max {
                    max = *i;
                }
            }
            Range::Bounded(i, j) => {
                if *i < min {
                    min = *i;
                }
                if *j > max {
                    max = *j;
                }
            }
            Range::UnboundedR(_) | Range::UnboundedL(_) => return None,
        }
    }
    Some((min, max))
}

impl Expression {
    /// Returns the possible values of the expression, recursing to leaf expressions
    pub fn domain_of(&self, syms: &SymbolTable) -> Option<Domain> {
        let ret = match self {
            //todo
            Expression::AbstractLiteral(_, _) => None,
            Expression::DominanceRelation(_, _) => Some(Domain::BoolDomain),
            Expression::FromSolution(_, expr) => expr.domain_of(syms),
            Expression::Comprehension(_, comprehension) => comprehension.domain_of(),
            Expression::UnsafeIndex(_, matrix, _) | Expression::SafeIndex(_, matrix, _) => {
                match matrix.domain_of(syms)? {
                    Domain::DomainMatrix(elem_domain, _) => Some(*elem_domain),
                    Domain::DomainTuple(_) => None,
                    Domain::DomainRecord(_) => None,
                    _ => {
                        bug!("subject of an index operation should support indexing")
                    }
                }
            }
            Expression::UnsafeSlice(_, matrix, indices)
            | Expression::SafeSlice(_, matrix, indices) => {
                let sliced_dimension = indices.iter().position(Option::is_none);

                let Domain::DomainMatrix(elem_domain, index_domains) = matrix.domain_of(syms)?
                else {
                    bug!("subject of an index operation should be a matrix");
                };

                match sliced_dimension {
                    Some(dimension) => Some(Domain::DomainMatrix(
                        elem_domain,
                        vec![index_domains[dimension].clone()],
                    )),

                    // same as index
                    None => Some(*elem_domain),
                }
            }
            Expression::InDomain(_, _, _) => Some(Domain::BoolDomain),
            Expression::Atomic(_, Atom::Reference(name)) => Some(syms.resolve_domain(name)?),
            Expression::Atomic(_, Atom::Literal(Literal::Int(n))) => {
                Some(Domain::IntDomain(vec![Range::Single(*n)]))
            }
            Expression::Atomic(_, Atom::Literal(Literal::Bool(_))) => Some(Domain::BoolDomain),
            Expression::Atomic(_, Atom::Literal(Literal::AbstractLiteral(_))) => None,
            Expression::Scope(_, _) => Some(Domain::BoolDomain),
            Expression::Sum(_, e) => expr_vec_lit_to_domain_i32(e, |x, y| Some(x + y), syms),
            Expression::Product(_, exprs) => {
                expr_vec_to_domain_i32(exprs, |x, y| Some(x * y), syms)
            }
            Expression::Min(_, e) => {
                expr_vec_lit_to_domain_i32(e, |x, y| Some(if x < y { x } else { y }), syms)
            }
            Expression::Max(_, e) => {
                expr_vec_lit_to_domain_i32(e, |x, y| Some(if x > y { x } else { y }), syms)
            }
            Expression::UnsafeDiv(_, a, b) => a.domain_of(syms)?.apply_i32(
                // rust integer division is truncating; however, we want to always round down,
                // including for negative numbers.
                |x, y| {
                    if y != 0 {
                        Some((x as f32 / y as f32).floor() as i32)
                    } else {
                        None
                    }
                },
                &b.domain_of(syms)?,
            ),
            Expression::SafeDiv(_, a, b) => {
                // rust integer division is truncating; however, we want to always round down
                // including for negative numbers.
                let domain = a.domain_of(syms)?.apply_i32(
                    |x, y| {
                        if y != 0 {
                            Some((x as f32 / y as f32).floor() as i32)
                        } else {
                            None
                        }
                    },
                    &b.domain_of(syms)?,
                );

                match domain {
                    Some(Domain::IntDomain(ranges)) => {
                        let mut ranges = ranges;
                        ranges.push(Range::Single(0));
                        Some(Domain::IntDomain(ranges))
                    }
                    None => Some(Domain::IntDomain(vec![Range::Single(0)])),
                    _ => None,
                }
            }
            Expression::UnsafeMod(_, a, b) => a.domain_of(syms)?.apply_i32(
                |x, y| if y != 0 { Some(x % y) } else { None },
                &b.domain_of(syms)?,
            ),

            Expression::SafeMod(_, a, b) => {
                let domain = a.domain_of(syms)?.apply_i32(
                    |x, y| if y != 0 { Some(x % y) } else { None },
                    &b.domain_of(syms)?,
                );

                match domain {
                    Some(Domain::IntDomain(ranges)) => {
                        let mut ranges = ranges;
                        ranges.push(Range::Single(0));
                        Some(Domain::IntDomain(ranges))
                    }
                    None => Some(Domain::IntDomain(vec![Range::Single(0)])),
                    _ => None,
                }
            }

            Expression::SafePow(_, a, b) | Expression::UnsafePow(_, a, b) => {
                a.domain_of(syms)?.apply_i32(
                    |x, y| {
                        if (x != 0 || y != 0) && y >= 0 {
                            Some(x.pow(y as u32))
                        } else {
                            None
                        }
                    },
                    &b.domain_of(syms)?,
                )
            }

            Expression::Root(_, _) => None,
            Expression::Bubble(_, _, _) => None,
            Expression::AuxDeclaration(_, _, _) => Some(Domain::BoolDomain),
            Expression::And(_, _) => Some(Domain::BoolDomain),
            Expression::Not(_, _) => Some(Domain::BoolDomain),
            Expression::Or(_, _) => Some(Domain::BoolDomain),
            Expression::Imply(_, _, _) => Some(Domain::BoolDomain),
            Expression::Eq(_, _, _) => Some(Domain::BoolDomain),
            Expression::Neq(_, _, _) => Some(Domain::BoolDomain),
            Expression::Geq(_, _, _) => Some(Domain::BoolDomain),
            Expression::Leq(_, _, _) => Some(Domain::BoolDomain),
            Expression::Gt(_, _, _) => Some(Domain::BoolDomain),
            Expression::Lt(_, _, _) => Some(Domain::BoolDomain),
            Expression::FlatAbsEq(_, _, _) => Some(Domain::BoolDomain),
            Expression::FlatSumGeq(_, _, _) => Some(Domain::BoolDomain),
            Expression::FlatSumLeq(_, _, _) => Some(Domain::BoolDomain),
            Expression::MinionDivEqUndefZero(_, _, _, _) => Some(Domain::BoolDomain),
            Expression::MinionModuloEqUndefZero(_, _, _, _) => Some(Domain::BoolDomain),
            Expression::FlatIneq(_, _, _, _) => Some(Domain::BoolDomain),
            Expression::AllDiff(_, _) => Some(Domain::BoolDomain),
            Expression::FlatWatchedLiteral(_, _, _) => Some(Domain::BoolDomain),
            Expression::MinionReify(_, _, _) => Some(Domain::BoolDomain),
            Expression::MinionReifyImply(_, _, _) => Some(Domain::BoolDomain),
            Expression::MinionWInIntervalSet(_, _, _) => Some(Domain::BoolDomain),
            Expression::MinionElementOne(_, _, _, _) => Some(Domain::BoolDomain),
            Expression::Neg(_, x) => {
                let Some(Domain::IntDomain(mut ranges)) = x.domain_of(syms) else {
                    return None;
                };

                for range in ranges.iter_mut() {
                    *range = match range {
                        Range::Single(x) => Range::Single(-*x),
                        Range::Bounded(x, y) => Range::Bounded(-*y, -*x),
                        Range::UnboundedR(i) => Range::UnboundedL(-*i),
                        Range::UnboundedL(i) => Range::UnboundedR(-*i),
                    };
                }

                Some(Domain::IntDomain(ranges))
            }
            Expression::Minus(_, a, b) => a
                .domain_of(syms)?
                .apply_i32(|x, y| Some(x - y), &b.domain_of(syms)?),

            Expression::FlatAllDiff(_, _) => Some(Domain::BoolDomain),
            Expression::FlatMinusEq(_, _, _) => Some(Domain::BoolDomain),
            Expression::FlatProductEq(_, _, _, _) => Some(Domain::BoolDomain),
            Expression::FlatWeightedSumLeq(_, _, _, _) => Some(Domain::BoolDomain),
            Expression::FlatWeightedSumGeq(_, _, _, _) => Some(Domain::BoolDomain),
            Expression::Abs(_, a) => a
                .domain_of(syms)?
                .apply_i32(|a, _| Some(a.abs()), &a.domain_of(syms)?),
            Expression::MinionPow(_, _, _, _) => Some(Domain::BoolDomain),
        };
        match ret {
            // TODO: (flm8) the Minion bindings currently only support single ranges for domains, so we use the min/max bounds
            // Once they support a full domain as we define it, we can remove this conversion
            Some(Domain::IntDomain(ranges)) if ranges.len() > 1 => {
                let (min, max) = range_vec_bounds_i32(&ranges)?;
                Some(Domain::IntDomain(vec![Range::Bounded(min, max)]))
            }
            _ => ret,
        }
    }

    pub fn get_meta(&self) -> Metadata {
        let metas: VecDeque<Metadata> = self.children_bi();
        metas[0].clone()
    }

    pub fn set_meta(&self, meta: Metadata) {
        self.transform_bi(Arc::new(move |_| meta.clone()));
    }

    /// Checks whether this expression is safe.
    ///
    /// An expression is unsafe if can be undefined, or if any of its children can be undefined.
    ///
    /// Unsafe expressions are (typically) prefixed with Unsafe in our AST, and can be made
    /// safe through the use of bubble rules.
    pub fn is_safe(&self) -> bool {
        // TODO: memoise in Metadata
        for expr in self.universe() {
            match expr {
                Expression::UnsafeDiv(_, _, _)
                | Expression::UnsafeMod(_, _, _)
                | Expression::UnsafePow(_, _, _)
                | Expression::UnsafeIndex(_, _, _)
                | Expression::UnsafeSlice(_, _, _) => {
                    return false;
                }
                _ => {}
            }
        }
        true
    }

    pub fn return_type(&self) -> Option<ReturnType> {
        match self {
            Expression::AbstractLiteral(_, _) => None,
            Expression::UnsafeIndex(_, subject, _) | Expression::SafeIndex(_, subject, _) => {
                Some(subject.return_type()?)
            }
            Expression::UnsafeSlice(_, subject, _) | Expression::SafeSlice(_, subject, _) => {
                Some(ReturnType::Matrix(Box::new(subject.return_type()?)))
            }
            Expression::InDomain(_, _, _) => Some(ReturnType::Bool),
            Expression::Comprehension(_, _) => None,
            Expression::Root(_, _) => Some(ReturnType::Bool),
            Expression::DominanceRelation(_, _) => Some(ReturnType::Bool),
            Expression::FromSolution(_, expr) => expr.return_type(),
            Expression::Atomic(_, Atom::Literal(Literal::Int(_))) => Some(ReturnType::Int),
            Expression::Atomic(_, Atom::Literal(Literal::Bool(_))) => Some(ReturnType::Bool),
            Expression::Atomic(_, Atom::Literal(Literal::AbstractLiteral(_))) => None,
            Expression::Atomic(_, Atom::Reference(_)) => None,
            Expression::Scope(_, scope) => scope.return_type(),
            Expression::Abs(_, _) => Some(ReturnType::Int),
            Expression::Sum(_, _) => Some(ReturnType::Int),
            Expression::Product(_, _) => Some(ReturnType::Int),
            Expression::Min(_, _) => Some(ReturnType::Int),
            Expression::Max(_, _) => Some(ReturnType::Int),
            Expression::Not(_, _) => Some(ReturnType::Bool),
            Expression::Or(_, _) => Some(ReturnType::Bool),
            Expression::Imply(_, _, _) => Some(ReturnType::Bool),
            Expression::And(_, _) => Some(ReturnType::Bool),
            Expression::Eq(_, _, _) => Some(ReturnType::Bool),
            Expression::Neq(_, _, _) => Some(ReturnType::Bool),
            Expression::Geq(_, _, _) => Some(ReturnType::Bool),
            Expression::Leq(_, _, _) => Some(ReturnType::Bool),
            Expression::Gt(_, _, _) => Some(ReturnType::Bool),
            Expression::Lt(_, _, _) => Some(ReturnType::Bool),
            Expression::SafeDiv(_, _, _) => Some(ReturnType::Int),
            Expression::UnsafeDiv(_, _, _) => Some(ReturnType::Int),
            Expression::FlatAllDiff(_, _) => Some(ReturnType::Bool),
            Expression::FlatSumGeq(_, _, _) => Some(ReturnType::Bool),
            Expression::FlatSumLeq(_, _, _) => Some(ReturnType::Bool),
            Expression::MinionDivEqUndefZero(_, _, _, _) => Some(ReturnType::Bool),
            Expression::FlatIneq(_, _, _, _) => Some(ReturnType::Bool),
            Expression::AllDiff(_, _) => Some(ReturnType::Bool),
            Expression::Bubble(_, _, _) => None, // TODO: (flm8) should this be a bool?
            Expression::FlatWatchedLiteral(_, _, _) => Some(ReturnType::Bool),
            Expression::MinionReify(_, _, _) => Some(ReturnType::Bool),
            Expression::MinionReifyImply(_, _, _) => Some(ReturnType::Bool),
            Expression::MinionWInIntervalSet(_, _, _) => Some(ReturnType::Bool),
            Expression::MinionElementOne(_, _, _, _) => Some(ReturnType::Bool),
            Expression::AuxDeclaration(_, _, _) => Some(ReturnType::Bool),
            Expression::UnsafeMod(_, _, _) => Some(ReturnType::Int),
            Expression::SafeMod(_, _, _) => Some(ReturnType::Int),
            Expression::MinionModuloEqUndefZero(_, _, _, _) => Some(ReturnType::Bool),
            Expression::Neg(_, _) => Some(ReturnType::Int),
            Expression::UnsafePow(_, _, _) => Some(ReturnType::Int),
            Expression::SafePow(_, _, _) => Some(ReturnType::Int),
            Expression::Minus(_, _, _) => Some(ReturnType::Int),
            Expression::FlatAbsEq(_, _, _) => Some(ReturnType::Bool),
            Expression::FlatMinusEq(_, _, _) => Some(ReturnType::Bool),
            Expression::FlatProductEq(_, _, _, _) => Some(ReturnType::Bool),
            Expression::FlatWeightedSumLeq(_, _, _, _) => Some(ReturnType::Bool),
            Expression::FlatWeightedSumGeq(_, _, _, _) => Some(ReturnType::Bool),
            Expression::MinionPow(_, _, _, _) => Some(ReturnType::Bool),
        }
    }

    pub fn is_clean(&self) -> bool {
        let metadata = self.get_meta();
        metadata.clean
    }

    pub fn set_clean(&mut self, bool_value: bool) {
        let mut metadata = self.get_meta();
        metadata.clean = bool_value;
        self.set_meta(metadata);
    }

    /// True if the expression is an associative and commutative operator
    pub fn is_associative_commutative_operator(&self) -> bool {
        matches!(
            self,
            Expression::Sum(_, _)
                | Expression::Or(_, _)
                | Expression::And(_, _)
                | Expression::Product(_, _)
        )
    }

    /// True iff self and other are both atomic and identical.
    ///
    /// This method is useful to cheaply check equivalence. Assuming CSE is enabled, any unifiable
    /// expressions will be rewritten to a common variable. This is much cheaper than checking the
    /// entire subtrees of `self` and `other`.
    pub fn identical_atom_to(&self, other: &Expression) -> bool {
        let atom1: Result<&Atom, _> = self.try_into();
        let atom2: Result<&Atom, _> = other.try_into();

        if let (Ok(atom1), Ok(atom2)) = (atom1, atom2) {
            atom2 == atom1
        } else {
            false
        }
    }

    /// If the expression is a list, returns the inner expressions.
    ///
    /// A list is any a matrix with the domain `int(1..)`. This includes matrix literals without
    /// any explicitly specified domain.
    pub fn unwrap_list(self) -> Option<Vec<Expression>> {
        match self {
            Expression::AbstractLiteral(_, matrix @ AbstractLiteral::Matrix(_, _)) => {
                matrix.unwrap_list().cloned()
            }
            Expression::Atomic(
                _,
                Atom::Literal(Literal::AbstractLiteral(matrix @ AbstractLiteral::Matrix(_, _))),
            ) => matrix.unwrap_list().map(|elems| {
                elems
                    .clone()
                    .into_iter()
                    .map(|x: Literal| Expression::Atomic(Metadata::new(), Atom::Literal(x)))
                    .collect_vec()
            }),
            _ => None,
        }
    }

    /// If the expression is a matrix, gets it elements and index domain.
    ///
    /// **Consider using the safer [`Expression::unwrap_list`] instead.**
    ///
    /// It is generally undefined to edit the length of a matrix unless it is a list (as defined by
    /// [`Expression::unwrap_list`]). Users of this function should ensure that, if the matrix is
    /// reconstructed, the index domain and the number of elements in the matrix remain the same.
    pub fn unwrap_matrix_unchecked(self) -> Option<(Vec<Expression>, Domain)> {
        match self {
            Expression::AbstractLiteral(_, AbstractLiteral::Matrix(elems, domain)) => {
                Some((elems.clone(), domain))
            }
            Expression::Atomic(
                _,
                Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Matrix(elems, domain))),
            ) => Some((
                elems
                    .clone()
                    .into_iter()
                    .map(|x: Literal| Expression::Atomic(Metadata::new(), Atom::Literal(x)))
                    .collect_vec(),
                domain,
            )),

            _ => None,
        }
    }

    /// For a Root expression, extends the inner vec with the given vec.
    ///
    /// # Panics
    /// Panics if the expression is not Root.
    pub fn extend_root(self, exprs: Vec<Expression>) -> Expression {
        match self {
            Expression::Root(meta, mut children) => {
                children.extend(exprs);
                Expression::Root(meta, children)
            }
            _ => panic!("extend_root called on a non-Root expression"),
        }
    }

    /// Converts the expression to a literal, if possible.
    pub fn to_literal(self) -> Option<Literal> {
        match self {
            Expression::Atomic(_, Atom::Literal(lit)) => Some(lit),
            Expression::AbstractLiteral(_, abslit) => {
                Some(Literal::AbstractLiteral(abslit.clone().as_literals()?))
            }
            Expression::Neg(_, e) => {
                let Literal::Int(i) = e.to_literal()? else {
                    bug!("negated literal should be an int");
                };

                Some(Literal::Int(-i))
            }

            _ => None,
        }
    }
}

impl From<i32> for Expression {
    fn from(i: i32) -> Self {
        Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(i)))
    }
}

impl From<bool> for Expression {
    fn from(b: bool) -> Self {
        Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(b)))
    }
}

impl From<Atom> for Expression {
    fn from(value: Atom) -> Self {
        Expression::Atomic(Metadata::new(), value)
    }
}

impl From<Name> for Expression {
    fn from(name: Name) -> Self {
        Expression::Atomic(Metadata::new(), Atom::Reference(name))
    }
}

impl From<Box<Expression>> for Expression {
    fn from(val: Box<Expression>) -> Self {
        val.as_ref().clone()
    }
}

impl Display for Expression {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            Expression::AbstractLiteral(_, l) => l.fmt(f),
            Expression::Comprehension(_, c) => c.fmt(f),
            Expression::UnsafeIndex(_, e1, e2) | Expression::SafeIndex(_, e1, e2) => {
                write!(f, "{e1}{}", pretty_vec(e2))
            }
            Expression::UnsafeSlice(_, e1, es) | Expression::SafeSlice(_, e1, es) => {
                let args = es
                    .iter()
                    .map(|x| match x {
                        Some(x) => format!("{}", x),
                        None => "..".into(),
                    })
                    .join(",");

                write!(f, "{e1}[{args}]")
            }
            Expression::InDomain(_, e, domain) => {
                write!(f, "__inDomain({e},{domain})")
            }
            Expression::Root(_, exprs) => {
                write!(f, "{}", pretty_expressions_as_top_level(exprs))
            }
            Expression::DominanceRelation(_, expr) => write!(f, "DominanceRelation({})", expr),
            Expression::FromSolution(_, expr) => write!(f, "FromSolution({})", expr),
            Expression::Atomic(_, atom) => atom.fmt(f),
            Expression::Scope(_, submodel) => write!(f, "{{\n{submodel}\n}}"),
            Expression::Abs(_, a) => write!(f, "|{}|", a),
            Expression::Sum(_, e) => {
                write!(f, "Sum({e})")
            }
            Expression::Product(_, expressions) => {
                write!(f, "Product({})", pretty_vec(expressions))
            }
            Expression::Min(_, e) => {
                write!(f, "min({e})")
            }
            Expression::Max(_, e) => {
                write!(f, "max({e})")
            }
            Expression::Not(_, expr_box) => {
                write!(f, "Not({})", expr_box.clone())
            }
            Expression::Or(_, e) => {
                write!(f, "or({e})")
            }
            Expression::And(_, e) => {
                write!(f, "and({e})")
            }
            Expression::Imply(_, box1, box2) => {
                write!(f, "({}) -> ({})", box1, box2)
            }
            Expression::Eq(_, box1, box2) => {
                write!(f, "({} = {})", box1.clone(), box2.clone())
            }
            Expression::Neq(_, box1, box2) => {
                write!(f, "({} != {})", box1.clone(), box2.clone())
            }
            Expression::Geq(_, box1, box2) => {
                write!(f, "({} >= {})", box1.clone(), box2.clone())
            }
            Expression::Leq(_, box1, box2) => {
                write!(f, "({} <= {})", box1.clone(), box2.clone())
            }
            Expression::Gt(_, box1, box2) => {
                write!(f, "({} > {})", box1.clone(), box2.clone())
            }
            Expression::Lt(_, box1, box2) => {
                write!(f, "({} < {})", box1.clone(), box2.clone())
            }
            Expression::FlatSumGeq(_, box1, box2) => {
                write!(f, "SumGeq({}, {})", pretty_vec(box1), box2.clone())
            }
            Expression::FlatSumLeq(_, box1, box2) => {
                write!(f, "SumLeq({}, {})", pretty_vec(box1), box2.clone())
            }
            Expression::FlatIneq(_, box1, box2, box3) => write!(
                f,
                "Ineq({}, {}, {})",
                box1.clone(),
                box2.clone(),
                box3.clone()
            ),
            Expression::AllDiff(_, e) => {
                write!(f, "allDiff({e})")
            }
            Expression::Bubble(_, box1, box2) => {
                write!(f, "{{{} @ {}}}", box1.clone(), box2.clone())
            }
            Expression::SafeDiv(_, box1, box2) => {
                write!(f, "SafeDiv({}, {})", box1.clone(), box2.clone())
            }
            Expression::UnsafeDiv(_, box1, box2) => {
                write!(f, "UnsafeDiv({}, {})", box1.clone(), box2.clone())
            }
            Expression::UnsafePow(_, box1, box2) => {
                write!(f, "UnsafePow({}, {})", box1.clone(), box2.clone())
            }
            Expression::SafePow(_, box1, box2) => {
                write!(f, "SafePow({}, {})", box1.clone(), box2.clone())
            }
            Expression::MinionDivEqUndefZero(_, box1, box2, box3) => {
                write!(
                    f,
                    "DivEq({}, {}, {})",
                    box1.clone(),
                    box2.clone(),
                    box3.clone()
                )
            }
            Expression::MinionModuloEqUndefZero(_, box1, box2, box3) => {
                write!(
                    f,
                    "ModEq({}, {}, {})",
                    box1.clone(),
                    box2.clone(),
                    box3.clone()
                )
            }
            Expression::FlatWatchedLiteral(_, x, l) => {
                write!(f, "WatchedLiteral({},{})", x, l)
            }
            Expression::MinionReify(_, box1, box2) => {
                write!(f, "Reify({}, {})", box1.clone(), box2.clone())
            }
            Expression::MinionReifyImply(_, box1, box2) => {
                write!(f, "ReifyImply({}, {})", box1.clone(), box2.clone())
            }
            Expression::MinionWInIntervalSet(_, atom, intervals) => {
                let intervals = intervals.iter().join(",");
                write!(f, "__minion_w_inintervalset({atom},{intervals})")
            }
            Expression::AuxDeclaration(_, n, e) => {
                write!(f, "{} =aux {}", n, e.clone())
            }
            Expression::UnsafeMod(_, a, b) => {
                write!(f, "{} % {}", a.clone(), b.clone())
            }
            Expression::SafeMod(_, a, b) => {
                write!(f, "SafeMod({},{})", a.clone(), b.clone())
            }
            Expression::Neg(_, a) => {
                write!(f, "-({})", a.clone())
            }
            Expression::Minus(_, a, b) => {
                write!(f, "({} - {})", a.clone(), b.clone())
            }
            Expression::FlatAllDiff(_, es) => {
                write!(f, "__flat_alldiff({})", pretty_vec(es))
            }
            Expression::FlatAbsEq(_, a, b) => {
                write!(f, "AbsEq({},{})", a.clone(), b.clone())
            }
            Expression::FlatMinusEq(_, a, b) => {
                write!(f, "MinusEq({},{})", a.clone(), b.clone())
            }
            Expression::FlatProductEq(_, a, b, c) => {
                write!(
                    f,
                    "FlatProductEq({},{},{})",
                    a.clone(),
                    b.clone(),
                    c.clone()
                )
            }
            Expression::FlatWeightedSumLeq(_, cs, vs, total) => {
                write!(
                    f,
                    "FlatWeightedSumLeq({},{},{})",
                    pretty_vec(cs),
                    pretty_vec(vs),
                    total.clone()
                )
            }
            Expression::FlatWeightedSumGeq(_, cs, vs, total) => {
                write!(
                    f,
                    "FlatWeightedSumGeq({},{},{})",
                    pretty_vec(cs),
                    pretty_vec(vs),
                    total.clone()
                )
            }
            Expression::MinionPow(_, atom, atom1, atom2) => {
                write!(f, "MinionPow({},{},{})", atom, atom1, atom2)
            }
            Expression::MinionElementOne(_, atoms, atom, atom1) => {
                let atoms = atoms.iter().join(",");
                write!(f, "__minion_element_one([{atoms}],{atom},{atom1})")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use crate::{ast::declaration::Declaration, matrix_expr};

    use super::*;

    #[test]
    fn test_domain_of_constant_sum() {
        let c1 = Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(1)));
        let c2 = Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(2)));
        let sum = Expression::Sum(
            Metadata::new(),
            Box::new(matrix_expr![c1.clone(), c2.clone()]),
        );
        assert_eq!(
            sum.domain_of(&SymbolTable::new()),
            Some(Domain::IntDomain(vec![Range::Single(3)]))
        );
    }

    #[test]
    fn test_domain_of_constant_invalid_type() {
        let c1 = Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(1)));
        let c2 = Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(true)));
        let sum = Expression::Sum(
            Metadata::new(),
            Box::new(matrix_expr![c1.clone(), c2.clone()]),
        );
        assert_eq!(sum.domain_of(&SymbolTable::new()), None);
    }

    #[test]
    fn test_domain_of_empty_sum() {
        let sum = Expression::Sum(Metadata::new(), Box::new(matrix_expr![]));
        assert_eq!(sum.domain_of(&SymbolTable::new()), None);
    }

    #[test]
    fn test_domain_of_reference() {
        let reference = Expression::Atomic(Metadata::new(), Atom::Reference(Name::MachineName(0)));
        let mut vars = SymbolTable::new();
        vars.insert(Rc::new(Declaration::new_var(
            Name::MachineName(0),
            Domain::IntDomain(vec![Range::Single(1)]),
        )))
        .unwrap();
        assert_eq!(
            reference.domain_of(&vars),
            Some(Domain::IntDomain(vec![Range::Single(1)]))
        );
    }

    #[test]
    fn test_domain_of_reference_not_found() {
        let reference = Expression::Atomic(Metadata::new(), Atom::Reference(Name::MachineName(0)));
        assert_eq!(reference.domain_of(&SymbolTable::new()), None);
    }

    #[test]
    fn test_domain_of_reference_sum_single() {
        let reference = Expression::Atomic(Metadata::new(), Atom::Reference(Name::MachineName(0)));
        let mut vars = SymbolTable::new();
        vars.insert(Rc::new(Declaration::new_var(
            Name::MachineName(0),
            Domain::IntDomain(vec![Range::Single(1)]),
        )))
        .unwrap();
        let sum = Expression::Sum(
            Metadata::new(),
            Box::new(matrix_expr![reference.clone(), reference.clone()]),
        );
        assert_eq!(
            sum.domain_of(&vars),
            Some(Domain::IntDomain(vec![Range::Single(2)]))
        );
    }

    #[test]
    fn test_domain_of_reference_sum_bounded() {
        let reference = Expression::Atomic(Metadata::new(), Atom::Reference(Name::MachineName(0)));
        let mut vars = SymbolTable::new();
        vars.insert(Rc::new(Declaration::new_var(
            Name::MachineName(0),
            Domain::IntDomain(vec![Range::Bounded(1, 2)]),
        )));
        let sum = Expression::Sum(
            Metadata::new(),
            Box::new(matrix_expr![reference.clone(), reference.clone()]),
        );
        assert_eq!(
            sum.domain_of(&vars),
            Some(Domain::IntDomain(vec![Range::Bounded(2, 4)]))
        );
    }
}
