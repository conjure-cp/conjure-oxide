use anyhow::{bail, Error};

use crate::ast::Expression;
use crate::error::{self, Error, Result};
use crate::{solvers::Solver, Model};

pub fn rewrite_to_solver(model: Model, solver: Solver) -> Result<Model> {
    todo!();
}
enum RuleKind {
    Horizontal,
    Vertical,
    SolverSpecific(Solver),
}

// TODO: possibly a nice macro for this:
// #[rule(kind=Horizontal)]
// fn this_rule(expr: Expression) -> Result<Expression> {...}
//
// rule.name=this_rule by default, but is overridable

pub struct Rule {
    name: String,
    kind: RuleKind,
    //guard: fn(Expression) -> bool,
    application: fn(Expression) -> Result<Expression>,
}

impl Rule {
    fn apply(self, expr: Expression) -> Result<Expression> {
        // if !self.guard(expr) {
        //     return Err(Error::RuleNotApplicable(self));
        // }
        (self.application)(expr)
    }
}
