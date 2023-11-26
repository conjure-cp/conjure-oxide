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

enum RuleApplicationError {
    RuleNotApplicable,
}

type RuleApplicationResult = Result<Expression, RuleApplicationError>;

// TODO: possibly a nice macro for this:
// #[rule(kind=Horizontal, name=this_rule)]
// fn this_rule(expr: Expression) -> Result<Expression> {...}
//
// rule.name=this_rule by default, but is overridable

pub struct Rule {
    name: String,
    kind: RuleKind,
    application: fn(Expression) -> RuleApplicationResult,
}

impl Rule {
    fn apply(self, expr: Expression) -> Result<Expression> {
        (self.application)(expr)
    }
}
