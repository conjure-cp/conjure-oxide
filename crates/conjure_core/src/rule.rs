use crate::ast::Expression;
use crate::solvers::Solver;

#[derive(Debug)]
pub enum RuleKind {
    Horizontal,
    Vertical,
    SolverSpecific(Solver),
}

pub enum RuleApplicationError {
    RuleNotApplicable,
}

pub type RuleApplicationResult = Result<Expression, RuleApplicationError>;

#[derive(Debug)]
pub struct Rule {
    pub name: String,
    pub kind: RuleKind,
    pub application: fn(Expression) -> RuleApplicationResult,
}

impl Rule {
    pub fn apply(self, expr: Expression) -> RuleApplicationResult {
        (self.application)(expr)
    }
}
