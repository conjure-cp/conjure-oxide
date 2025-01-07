use std::fmt::{Display, Formatter};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use enum_compatability_macro::document_compatibility;
use uniplate::derive::Uniplate;
use uniplate::{Biplate, Uniplate as _};

use crate::ast::literals::Literal;
use crate::ast::pretty::pretty_vec;
use crate::ast::symbol_table::{Name, SymbolTable};
use crate::ast::Atom;
use crate::ast::ReturnType;
use crate::metadata::Metadata;

use super::{Domain, Range};

/// Represents different types of expressions used to define rules and constraints in the model.
///
/// The `Expression` enum includes operations, constants, and variable references
/// used to build rules and conditions for the model.
#[document_compatibility]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Uniplate)]
#[uniplate(walk_into=[Atom])]
#[biplate(to=Literal)]
#[biplate(to=Metadata)]
#[biplate(to=Atom)]
#[biplate(to=Name)]
#[biplate(to=Vec<Expression>)]
pub enum Expression {
    /// An expression representing "A is valid as long as B is true"
    /// Turns into a conjunction when it reaches a boolean context
    Bubble(Metadata, Box<Expression>, Box<Expression>),

    Atomic(Metadata, Atom),

    #[compatible(JsonInput)]
    Sum(Metadata, Vec<Expression>),

    #[compatible(JsonInput)]
    Product(Metadata, Vec<Expression>),

    #[compatible(JsonInput)]
    Min(Metadata, Vec<Expression>),

    #[compatible(JsonInput)]
    Max(Metadata, Vec<Expression>),

    #[compatible(JsonInput, SAT)]
    Not(Metadata, Box<Expression>),

    #[compatible(JsonInput, SAT)]
    Or(Metadata, Vec<Expression>),

    #[compatible(JsonInput, SAT)]
    And(Metadata, Vec<Expression>),

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

    #[compatible(JsonInput)]
    AllDiff(Metadata, Vec<Expression>),

    /// Binary subtraction operator
    ///
    /// This is a parser-level construct, and is immediately normalised to `Sum([a,-b])`.
    #[compatible(JsonInput)]
    Minus(Metadata, Box<Expression>, Box<Expression>),

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

fn range_vec_bounds_i32(ranges: &Vec<Range<i32>>) -> (i32, i32) {
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
        }
    }
    (min, max)
}

impl Expression {
    /// Returns the possible values of the expression, recursing to leaf expressions
    pub fn domain_of(&self, vars: &SymbolTable) -> Option<Domain> {
        let ret = match self {
            Expression::Atomic(_, Atom::Reference(name)) => Some(vars.get(name)?.domain.clone()),
            Expression::Atomic(_, Atom::Literal(Literal::Int(n))) => {
                Some(Domain::IntDomain(vec![Range::Single(*n)]))
            }
            Expression::Atomic(_, Atom::Literal(Literal::Bool(_))) => Some(Domain::BoolDomain),
            Expression::Sum(_, exprs) => expr_vec_to_domain_i32(exprs, |x, y| Some(x + y), vars),
            Expression::Product(_, exprs) => {
                expr_vec_to_domain_i32(exprs, |x, y| Some(x * y), vars)
            }
            Expression::Min(_, exprs) => {
                expr_vec_to_domain_i32(exprs, |x, y| Some(if x < y { x } else { y }), vars)
            }
            Expression::Max(_, exprs) => {
                expr_vec_to_domain_i32(exprs, |x, y| Some(if x > y { x } else { y }), vars)
            }
            Expression::UnsafeDiv(_, a, b) => a.domain_of(vars)?.apply_i32(
                |x, y| if y != 0 { Some(x / y) } else { None },
                &b.domain_of(vars)?,
            ),
            Expression::SafeDiv(_, a, b) => {
                let domain = a.domain_of(vars)?.apply_i32(
                    |x, y| if y != 0 { Some(x / y) } else { None },
                    &b.domain_of(vars)?,
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
            Expression::UnsafeMod(_, a, b) => a.domain_of(vars)?.apply_i32(
                |x, y| if y != 0 { Some(x % y) } else { None },
                &b.domain_of(vars)?,
            ),

            Expression::SafeMod(_, a, b) => {
                let domain = a.domain_of(vars)?.apply_i32(
                    |x, y| if y != 0 { Some(x % y) } else { None },
                    &b.domain_of(vars)?,
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

            Expression::Bubble(_, _, _) => None,
            Expression::AuxDeclaration(_, _, _) => Some(Domain::BoolDomain),
            Expression::And(_, _) => Some(Domain::BoolDomain),
            Expression::Not(_, _) => Some(Domain::BoolDomain),
            Expression::Or(_, _) => Some(Domain::BoolDomain),
            Expression::Eq(_, _, _) => Some(Domain::BoolDomain),
            Expression::Neq(_, _, _) => Some(Domain::BoolDomain),
            Expression::Geq(_, _, _) => Some(Domain::BoolDomain),
            Expression::Leq(_, _, _) => Some(Domain::BoolDomain),
            Expression::Gt(_, _, _) => Some(Domain::BoolDomain),
            Expression::Lt(_, _, _) => Some(Domain::BoolDomain),
            Expression::FlatSumGeq(_, _, _) => Some(Domain::BoolDomain),
            Expression::FlatSumLeq(_, _, _) => Some(Domain::BoolDomain),
            Expression::MinionDivEqUndefZero(_, _, _, _) => Some(Domain::BoolDomain),
            Expression::MinionModuloEqUndefZero(_, _, _, _) => Some(Domain::BoolDomain),
            Expression::FlatIneq(_, _, _, _) => Some(Domain::BoolDomain),
            Expression::AllDiff(_, _) => Some(Domain::BoolDomain),
            Expression::FlatWatchedLiteral(_, _, _) => Some(Domain::BoolDomain),
            Expression::MinionReify(_, _, _) => Some(Domain::BoolDomain),
            Expression::Neg(_, x) => {
                let Some(Domain::IntDomain(mut ranges)) = x.domain_of(vars) else {
                    return None;
                };

                for range in ranges.iter_mut() {
                    *range = match range {
                        Range::Single(x) => Range::Single(-*x),
                        Range::Bounded(x, y) => Range::Bounded(-*y, -*x),
                    };
                }

                Some(Domain::IntDomain(ranges))
            }
            Expression::Minus(_, a, b) => a
                .domain_of(vars)?
                .apply_i32(|x, y| Some(x - y), &b.domain_of(vars)?),

            Expression::FlatMinusEq(_, _, _) => Some(Domain::BoolDomain),
            Expression::FlatProductEq(_, _, _, _) => Some(Domain::BoolDomain),
            Expression::FlatWeightedSumLeq(_, _, _, _) => Some(Domain::BoolDomain),
            Expression::FlatWeightedSumGeq(_, _, _, _) => Some(Domain::BoolDomain),
            // #[allow(unreachable_patterns)]
            // _ => bug!("Cannot calculate domain of {:?}", self),
        };
        match ret {
            // TODO: (flm8) the Minion bindings currently only support single ranges for domains, so we use the min/max bounds
            // Once they support a full domain as we define it, we can remove this conversion
            Some(Domain::IntDomain(ranges)) if ranges.len() > 1 => {
                let (min, max) = range_vec_bounds_i32(&ranges);
                Some(Domain::IntDomain(vec![Range::Bounded(min, max)]))
            }
            _ => ret,
        }
    }

    pub fn get_meta(&self) -> Metadata {
        <Expression as Biplate<Metadata>>::children_bi(self)[0].clone()
    }

    pub fn set_meta(&self, meta: Metadata) {
        <Expression as Biplate<Metadata>>::transform_bi(self, Arc::new(move |_| meta.clone()));
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
                Expression::UnsafeDiv(_, _, _) | Expression::UnsafeMod(_, _, _) => {
                    return false;
                }
                _ => {}
            }
        }
        true
    }

    pub fn return_type(&self) -> Option<ReturnType> {
        match self {
            Expression::Atomic(_, Atom::Literal(Literal::Int(_))) => Some(ReturnType::Int),
            Expression::Atomic(_, Atom::Literal(Literal::Bool(_))) => Some(ReturnType::Bool),
            Expression::Atomic(_, Atom::Reference(_)) => None,
            Expression::Sum(_, _) => Some(ReturnType::Int),
            Expression::Product(_, _) => Some(ReturnType::Int),
            Expression::Min(_, _) => Some(ReturnType::Int),
            Expression::Max(_, _) => Some(ReturnType::Int),
            Expression::Not(_, _) => Some(ReturnType::Bool),
            Expression::Or(_, _) => Some(ReturnType::Bool),
            Expression::And(_, _) => Some(ReturnType::Bool),
            Expression::Eq(_, _, _) => Some(ReturnType::Bool),
            Expression::Neq(_, _, _) => Some(ReturnType::Bool),
            Expression::Geq(_, _, _) => Some(ReturnType::Bool),
            Expression::Leq(_, _, _) => Some(ReturnType::Bool),
            Expression::Gt(_, _, _) => Some(ReturnType::Bool),
            Expression::Lt(_, _, _) => Some(ReturnType::Bool),
            Expression::SafeDiv(_, _, _) => Some(ReturnType::Int),
            Expression::UnsafeDiv(_, _, _) => Some(ReturnType::Int),
            Expression::FlatSumGeq(_, _, _) => Some(ReturnType::Bool),
            Expression::FlatSumLeq(_, _, _) => Some(ReturnType::Bool),
            Expression::MinionDivEqUndefZero(_, _, _, _) => Some(ReturnType::Bool),
            Expression::FlatIneq(_, _, _, _) => Some(ReturnType::Bool),
            Expression::AllDiff(_, _) => Some(ReturnType::Bool),
            Expression::Bubble(_, _, _) => None, // TODO: (flm8) should this be a bool?
            Expression::FlatWatchedLiteral(_, _, _) => Some(ReturnType::Bool),
            Expression::MinionReify(_, _, _) => Some(ReturnType::Bool),
            Expression::AuxDeclaration(_, _, _) => Some(ReturnType::Bool),
            Expression::UnsafeMod(_, _, _) => Some(ReturnType::Int),
            Expression::SafeMod(_, _, _) => Some(ReturnType::Int),
            Expression::MinionModuloEqUndefZero(_, _, _, _) => Some(ReturnType::Bool),
            Expression::Neg(_, _) => Some(ReturnType::Int),
            Expression::Minus(_, _, _) => Some(ReturnType::Int),
            Expression::FlatMinusEq(_, _, _) => Some(ReturnType::Bool),
            Expression::FlatProductEq(_, _, _, _) => Some(ReturnType::Bool),
            Expression::FlatWeightedSumLeq(_, _, _, _) => Some(ReturnType::Bool),
            Expression::FlatWeightedSumGeq(_, _, _, _) => Some(ReturnType::Bool),
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
impl Display for Expression {
    // TODO: (flm8) this will change once we implement a parser (two-way conversion)
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            Expression::Atomic(_, atom) => atom.fmt(f),
            Expression::Sum(_, expressions) => {
                write!(f, "Sum({})", pretty_vec(expressions))
            }
            Expression::Product(_, expressions) => {
                write!(f, "Product({})", pretty_vec(expressions))
            }
            Expression::Min(_, expressions) => {
                write!(f, "Min({})", pretty_vec(expressions))
            }
            Expression::Max(_, expressions) => {
                write!(f, "Max({})", pretty_vec(expressions))
            }
            Expression::Not(_, expr_box) => {
                write!(f, "Not({})", expr_box.clone())
            }
            Expression::Or(_, expressions) => {
                write!(f, "Or({})", pretty_vec(expressions))
            }
            Expression::And(_, expressions) => {
                write!(f, "And({})", pretty_vec(expressions))
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
            Expression::AllDiff(_, expressions) => {
                write!(f, "AllDiff({})", pretty_vec(expressions))
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
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::DecisionVariable;

    use super::*;

    #[test]
    fn test_domain_of_constant_sum() {
        let c1 = Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(1)));
        let c2 = Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(2)));
        let sum = Expression::Sum(Metadata::new(), vec![c1.clone(), c2.clone()]);
        assert_eq!(
            sum.domain_of(&SymbolTable::new()),
            Some(Domain::IntDomain(vec![Range::Single(3)]))
        );
    }

    #[test]
    fn test_domain_of_constant_invalid_type() {
        let c1 = Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(1)));
        let c2 = Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(true)));
        let sum = Expression::Sum(Metadata::new(), vec![c1.clone(), c2.clone()]);
        assert_eq!(sum.domain_of(&SymbolTable::new()), None);
    }

    #[test]
    fn test_domain_of_empty_sum() {
        let sum = Expression::Sum(Metadata::new(), vec![]);
        assert_eq!(sum.domain_of(&SymbolTable::new()), None);
    }

    #[test]
    fn test_domain_of_reference() {
        let reference = Expression::Atomic(Metadata::new(), Atom::Reference(Name::MachineName(0)));
        let mut vars = SymbolTable::new();
        vars.insert(
            Name::MachineName(0),
            DecisionVariable::new(Domain::IntDomain(vec![Range::Single(1)])),
        );
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
        vars.insert(
            Name::MachineName(0),
            DecisionVariable::new(Domain::IntDomain(vec![Range::Single(1)])),
        );
        let sum = Expression::Sum(Metadata::new(), vec![reference.clone(), reference.clone()]);
        assert_eq!(
            sum.domain_of(&vars),
            Some(Domain::IntDomain(vec![Range::Single(2)]))
        );
    }

    #[test]
    fn test_domain_of_reference_sum_bounded() {
        let reference = Expression::Atomic(Metadata::new(), Atom::Reference(Name::MachineName(0)));
        let mut vars = SymbolTable::new();
        vars.insert(
            Name::MachineName(0),
            DecisionVariable::new(Domain::IntDomain(vec![Range::Bounded(1, 2)])),
        );
        let sum = Expression::Sum(Metadata::new(), vec![reference.clone(), reference.clone()]);
        assert_eq!(
            sum.domain_of(&vars),
            Some(Domain::IntDomain(vec![Range::Bounded(2, 4)]))
        );
    }
}
