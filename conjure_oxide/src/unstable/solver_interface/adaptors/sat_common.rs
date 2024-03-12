//! Common code for SAT adaptors.
//! Primarily, this is CNF related code.

// (nd60, march 24) - i basically copied all this from @gskorokod's SAT implemention for the old
// solver interface.
use crate::{
    ast as conjure_ast, unstable::solver_interface::SolverError,
    unstable::solver_interface::SolverError::*,
};
use conjure_core::metadata::Metadata;
use std::collections::HashMap;
use thiserror::Error;

/// A representation of a model in CNF.
///
/// Expects Model to be in the Conjunctive Normal Form:
///
/// - All variables must be boolean
/// - Expressions must be `Reference`, `Not(Reference)`, or `Or(Reference1, Not(Reference2), ...)`
/// - The top level And() may contain nested Or()s. Any other nested expressions are not allowed.
#[derive(Debug, Clone)]
pub struct CNFModel {
    pub clauses: Vec<Vec<i32>>,
    variables: HashMap<conjure_ast::Name, i32>,
    next_ind: i32,
}
impl CNFModel {
    pub fn new() -> CNFModel {
        CNFModel {
            clauses: Vec::new(),
            variables: HashMap::new(),
            next_ind: 1,
        }
    }

    pub fn from_conjure(conjure_model: conjure_ast::Model) -> Result<CNFModel, SolverError> {
        let mut ans: CNFModel = CNFModel::new();

        for var in conjure_model.variables.keys() {
            // Check that domain has the correct type
            let decision_var = match conjure_model.variables.get(var) {
                None => {
                    return Err(ModelInvalid(format!("variable {:?} not found", var)));
                }
                Some(var) => var,
            };

            if decision_var.domain != conjure_ast::Domain::BoolDomain {
                return Err(ModelFeatureNotSupported(format!(
                    "variable {:?} is not BoolDomain",
                    decision_var
                )));
            }

            ans.add_variable(var);
        }

        for expr in conjure_model.get_constraints_vec() {
            match ans.add_expression(&expr) {
                Ok(_) => {}
                Err(error) => {
                    let message = format!("{:?}", error);
                    return Err(ModelFeatureNotSupported(message));
                }
            }
        }

        Ok(ans)
    }

    /// Gets all the Conjure variables in the CNF.
    #[allow(dead_code)] // It will be used once we actually run kissat
    pub fn get_variables(&self) -> Vec<&conjure_ast::Name> {
        let mut ans: Vec<&conjure_ast::Name> = Vec::new();

        for key in self.variables.keys() {
            ans.push(key);
        }

        ans
    }

    /// Gets the index of a Conjure variable.
    pub fn get_index(&self, var: &conjure_ast::Name) -> Option<i32> {
        return self.variables.get(var).copied();
    }

    /// Gets a Conjure variable by index.
    pub fn get_name(&self, ind: i32) -> Option<&conjure_ast::Name> {
        for key in self.variables.keys() {
            let idx = self.get_index(key)?;
            if idx == ind {
                return Some(key);
            }
        }

        None
    }

    /// Adds a new Conjure variable to the CNF representation.
    pub fn add_variable(&mut self, var: &conjure_ast::Name) {
        self.variables.insert(var.clone(), self.next_ind);
        self.next_ind += 1;
    }

    /**
     * Check if a Conjure variable or index is present in the CNF
     */
    pub fn has_variable<T: HasVariable>(&self, value: T) -> bool {
        value.has_variable(self)
    }

    /**
     * Add a new clause to the CNF. Must be a vector of indices in CNF format
     */
    pub fn add_clause(&mut self, vec: &Vec<i32>) -> Result<(), CNFError> {
        for idx in vec {
            if !self.has_variable(idx.abs()) {
                return Err(CNFError::ClauseIndexNotFound(*idx));
            }
        }
        self.clauses.push(vec.clone());
        Ok(())
    }

    /**
     * Add a new Conjure expression to the CNF. Must be a logical expression in CNF form
     */
    pub fn add_expression(&mut self, expr: &conjure_ast::Expression) -> Result<(), CNFError> {
        for row in self.handle_expression(expr)? {
            self.add_clause(&row)?;
        }
        Ok(())
    }

    /**
     * Convert the CNF to a Conjure expression
     */
    #[allow(dead_code)] // It will be used once we actually run kissat
    pub fn as_expression(&self) -> Result<conjure_ast::Expression, CNFError> {
        let mut expr_clauses: Vec<conjure_ast::Expression> = Vec::new();

        for clause in &self.clauses {
            expr_clauses.push(self.clause_to_expression(clause)?);
        }

        Ok(conjure_ast::Expression::And(Metadata::new(), expr_clauses))
    }

    /**
     * Convert a single clause to a Conjure expression
     */
    fn clause_to_expression(&self, clause: &Vec<i32>) -> Result<conjure_ast::Expression, CNFError> {
        let mut ans: Vec<conjure_ast::Expression> = Vec::new();

        for idx in clause {
            match self.get_name(idx.abs()) {
                None => return Err(CNFError::ClauseIndexNotFound(*idx)),
                Some(name) => {
                    if *idx > 0 {
                        ans.push(conjure_ast::Expression::Reference(
                            Metadata::new(),
                            name.clone(),
                        ));
                    } else {
                        let expression: conjure_ast::Expression =
                            conjure_ast::Expression::Reference(Metadata::new(), name.clone());
                        ans.push(conjure_ast::Expression::Not(
                            Metadata::new(),
                            Box::from(expression),
                        ))
                    }
                }
            }
        }

        Ok(conjure_ast::Expression::Or(Metadata::new(), ans))
    }

    /**
     * Get the index for a Conjure Reference or return an error
     * @see get_index
     * @see conjure_ast::Expression::Reference
     */
    fn get_reference_index(&self, name: &conjure_ast::Name) -> Result<i32, CNFError> {
        match self.get_index(name) {
            None => Err(CNFError::VariableNameNotFound(name.clone())),
            Some(ind) => Ok(ind),
        }
    }

    /**
     * Convert the contents of a single Reference to a row of the CNF format
     * @see get_reference_index
     * @see conjure_ast::Expression::Reference
     */
    fn handle_reference(&self, name: &conjure_ast::Name) -> Result<Vec<i32>, CNFError> {
        Ok(vec![self.get_reference_index(name)?])
    }

    /**
     * Convert the contents of a single Not() to CNF
     */
    fn handle_not(&self, expr: &conjure_ast::Expression) -> Result<Vec<i32>, CNFError> {
        match expr {
            // Expression inside the Not()
            conjure_ast::Expression::Reference(_metadata, name) => {
                Ok(vec![-self.get_reference_index(name)?])
            }
            _ => Err(CNFError::UnexpectedExpressionInsideNot(expr.clone())),
        }
    }

    /**
     * Convert the contents of a single Or() to a row of the CNF format
     */
    fn handle_or(&self, expressions: &Vec<conjure_ast::Expression>) -> Result<Vec<i32>, CNFError> {
        let mut ans: Vec<i32> = Vec::new();

        for expr in expressions {
            let ret = self.handle_flat_expression(expr)?;
            for ind in ret {
                ans.push(ind);
            }
        }

        Ok(ans)
    }

    /**
     * Convert a single Reference, `Not` or `Or` into a clause of the CNF format
     */
    fn handle_flat_expression(
        &self,
        expression: &conjure_ast::Expression,
    ) -> Result<Vec<i32>, CNFError> {
        match expression {
            conjure_ast::Expression::Reference(_metadata, name) => self.handle_reference(name),
            conjure_ast::Expression::Not(_metadata, var_box) => self.handle_not(var_box),
            conjure_ast::Expression::Or(_metadata, expressions) => self.handle_or(expressions),
            _ => Err(CNFError::UnexpectedExpression(expression.clone())),
        }
    }

    /**
     * Convert a single And() into a vector of clauses in the CNF format
     */
    fn handle_and(
        &self,
        expressions: &Vec<conjure_ast::Expression>,
    ) -> Result<Vec<Vec<i32>>, CNFError> {
        let mut ans: Vec<Vec<i32>> = Vec::new();

        for expression in expressions {
            match expression {
                conjure_ast::Expression::And(_metadata, _expressions) => {
                    return Err(CNFError::NestedAnd(expression.clone()));
                }
                _ => {
                    ans.push(self.handle_flat_expression(expression)?);
                }
            }
        }

        Ok(ans)
    }

    /**
     * Convert a single Conjure expression into a vector of clauses of the CNF format
     */
    fn handle_expression(
        &self,
        expression: &conjure_ast::Expression,
    ) -> Result<Vec<Vec<i32>>, CNFError> {
        match expression {
            conjure_ast::Expression::And(_metadata, expressions) => self.handle_and(expressions),
            _ => Ok(vec![self.handle_flat_expression(expression)?]),
        }
    }
}

#[derive(Error, Debug)]
pub enum CNFError {
    #[error("Variable with name `{0}` not found")]
    VariableNameNotFound(conjure_ast::Name),

    #[error("Clause with index `{0}` not found")]
    ClauseIndexNotFound(i32),

    #[error("Unexpected Expression `{0}` inside Not(). Only Not(Reference) allowed!")]
    UnexpectedExpressionInsideNot(conjure_ast::Expression),

    #[error(
        "Unexpected Expression `{0}` found. Only Reference, Not(Reference) and Or(...) allowed!"
    )]
    UnexpectedExpression(conjure_ast::Expression),

    #[error("Unexpected nested And: {0}")]
    NestedAnd(conjure_ast::Expression),
}

/// Helper trait for checking if a variable is present in the CNF polymorphically (i32 or conjure_ast::Name)
trait HasVariable {
    fn has_variable(self, cnf: &CNFModel) -> bool;
}

impl HasVariable for i32 {
    fn has_variable(self, cnf: &CNFModel) -> bool {
        return cnf.get_name(self).is_some();
    }
}

impl HasVariable for &conjure_ast::Name {
    fn has_variable(self, cnf: &CNFModel) -> bool {
        cnf.get_index(self).is_some()
    }
}

#[cfg(test)]
mod tests {
    use conjure_core::metadata::Metadata;

    use super::CNFModel;
    use crate::ast::Domain::{BoolDomain, IntDomain};
    use crate::ast::Expression::{And, Not, Or, Reference};
    use crate::ast::{DecisionVariable, Model};
    use crate::ast::{Expression, Name};
    use crate::unstable::solver_interface::SolverError;
    use crate::utils::testing::assert_eq_any_order;

    #[test]
    fn test_single_var() {
        // x -> [[1]]

        let mut model: Model = Model::default();

        let x: Name = Name::UserName(String::from('x'));
        model.add_variable(x.clone(), DecisionVariable { domain: BoolDomain });
        model.add_constraint(Reference(Metadata::new(), x.clone()));

        let res: Result<CNFModel, SolverError> = CNFModel::from_conjure(model);
        assert!(res.is_ok());

        let cnf = res.unwrap();

        assert_eq!(cnf.get_index(&x), Some(1));
        assert!(cnf.get_name(1).is_some());
        assert_eq!(cnf.get_name(1).unwrap(), &x);

        assert_eq!(cnf.clauses, vec![vec![1]]);
    }

    #[test]
    fn test_single_not() {
        // Not(x) -> [[-1]]

        let mut model: Model = Model::default();

        let x: Name = Name::UserName(String::from('x'));
        model.add_variable(x.clone(), DecisionVariable { domain: BoolDomain });
        model.add_constraint(Not(
            Metadata::new(),
            Box::from(Reference(Metadata::new(), x.clone())),
        ));

        let cnf: CNFModel = CNFModel::from_conjure(model).unwrap();
        assert_eq!(cnf.get_index(&x), Some(1));
        assert_eq!(cnf.clauses, vec![vec![-1]]);

        assert_eq!(
            cnf.as_expression().unwrap(),
            And(
                Metadata::new(),
                vec![Or(
                    Metadata::new(),
                    vec![Not(
                        Metadata::new(),
                        Box::from(Reference(Metadata::new(), x.clone()))
                    )]
                )]
            )
        )
    }

    #[test]
    fn test_single_or() {
        // Or(x, y) -> [[1, 2]]

        let mut model: Model = Model::default();

        let x: Name = Name::UserName(String::from('x'));
        let y: Name = Name::UserName(String::from('y'));

        model.add_variable(x.clone(), DecisionVariable { domain: BoolDomain });
        model.add_variable(y.clone(), DecisionVariable { domain: BoolDomain });

        model.add_constraint(Or(
            Metadata::new(),
            vec![
                Reference(Metadata::new(), x.clone()),
                Reference(Metadata::new(), y.clone()),
            ],
        ));

        let cnf: CNFModel = CNFModel::from_conjure(model).unwrap();

        let xi = cnf.get_index(&x).unwrap();
        let yi = cnf.get_index(&y).unwrap();
        assert_eq_any_order(&cnf.clauses, &vec![vec![xi, yi]]);

        assert_eq!(
            cnf.as_expression().unwrap(),
            And(
                Metadata::new(),
                vec![Or(
                    Metadata::new(),
                    vec![
                        Reference(Metadata::new(), x.clone()),
                        Reference(Metadata::new(), y.clone())
                    ]
                )]
            )
        )
    }

    #[test]
    fn test_or_not() {
        // Or(x, Not(y)) -> [[1, -2]]

        let mut model: Model = Model::default();

        let x: Name = Name::UserName(String::from('x'));
        let y: Name = Name::UserName(String::from('y'));

        model.add_variable(x.clone(), DecisionVariable { domain: BoolDomain });
        model.add_variable(y.clone(), DecisionVariable { domain: BoolDomain });

        model.add_constraint(Or(
            Metadata::new(),
            vec![
                Reference(Metadata::new(), x.clone()),
                Not(
                    Metadata::new(),
                    Box::from(Reference(Metadata::new(), y.clone())),
                ),
            ],
        ));

        let cnf: CNFModel = CNFModel::from_conjure(model).unwrap();

        let xi = cnf.get_index(&x).unwrap();
        let yi = cnf.get_index(&y).unwrap();
        assert_eq_any_order(&cnf.clauses, &vec![vec![xi, -yi]]);

        assert_eq!(
            cnf.as_expression().unwrap(),
            And(
                Metadata::new(),
                vec![Or(
                    Metadata::new(),
                    vec![
                        Reference(Metadata::new(), x.clone()),
                        Not(
                            Metadata::new(),
                            Box::from(Reference(Metadata::new(), y.clone()))
                        )
                    ]
                )]
            )
        )
    }

    #[test]
    fn test_multiple() {
        // [x, y] - equivalent to And(x, y) -> [[1], [2]]

        let mut model: Model = Model::default();

        let x: Name = Name::UserName(String::from('x'));
        let y: Name = Name::UserName(String::from('y'));

        model.add_variable(x.clone(), DecisionVariable { domain: BoolDomain });
        model.add_variable(y.clone(), DecisionVariable { domain: BoolDomain });

        model.add_constraint(Reference(Metadata::new(), x.clone()));
        model.add_constraint(Reference(Metadata::new(), y.clone()));

        let cnf: CNFModel = CNFModel::from_conjure(model).unwrap();

        let xi = cnf.get_index(&x).unwrap();
        let yi = cnf.get_index(&y).unwrap();
        assert_eq_any_order(&cnf.clauses, &vec![vec![xi], vec![yi]]);

        assert_eq!(
            cnf.as_expression().unwrap(),
            And(
                Metadata::new(),
                vec![
                    Or(Metadata::new(), vec![Reference(Metadata::new(), x.clone())]),
                    Or(Metadata::new(), vec![Reference(Metadata::new(), y.clone())])
                ]
            )
        )
    }

    #[test]
    fn test_and() {
        // And(x, y) -> [[1], [2]]

        let mut model: Model = Model::default();

        let x: Name = Name::UserName(String::from('x'));
        let y: Name = Name::UserName(String::from('y'));

        model.add_variable(x.clone(), DecisionVariable { domain: BoolDomain });
        model.add_variable(y.clone(), DecisionVariable { domain: BoolDomain });

        model.add_constraint(And(
            Metadata::new(),
            vec![
                Reference(Metadata::new(), x.clone()),
                Reference(Metadata::new(), y.clone()),
            ],
        ));

        let cnf: CNFModel = CNFModel::from_conjure(model).unwrap();

        let xi = cnf.get_index(&x).unwrap();
        let yi = cnf.get_index(&y).unwrap();
        assert_eq_any_order(&cnf.clauses, &vec![vec![xi], vec![yi]]);

        assert_eq!(
            cnf.as_expression().unwrap(),
            And(
                Metadata::new(),
                vec![
                    Or(Metadata::new(), vec![Reference(Metadata::new(), x.clone())]),
                    Or(Metadata::new(), vec![Reference(Metadata::new(), y.clone())])
                ]
            )
        )
    }

    #[test]
    fn test_nested_ors() {
        // Or(x, Or(y, z)) -> [[1, 2, 3]]

        let mut model: Model = Model::default();

        let x: Name = Name::UserName(String::from('x'));
        let y: Name = Name::UserName(String::from('y'));
        let z: Name = Name::UserName(String::from('z'));

        model.add_variable(x.clone(), DecisionVariable { domain: BoolDomain });
        model.add_variable(y.clone(), DecisionVariable { domain: BoolDomain });
        model.add_variable(z.clone(), DecisionVariable { domain: BoolDomain });

        model.add_constraint(Or(
            Metadata::new(),
            vec![
                Reference(Metadata::new(), x.clone()),
                Or(
                    Metadata::new(),
                    vec![
                        Reference(Metadata::new(), y.clone()),
                        Reference(Metadata::new(), z.clone()),
                    ],
                ),
            ],
        ));

        let cnf: CNFModel = CNFModel::from_conjure(model).unwrap();

        let xi = cnf.get_index(&x).unwrap();
        let yi = cnf.get_index(&y).unwrap();
        let zi = cnf.get_index(&z).unwrap();
        assert_eq_any_order(&cnf.clauses, &vec![vec![xi, yi, zi]]);

        assert_eq!(
            cnf.as_expression().unwrap(),
            And(
                Metadata::new(),
                vec![Or(
                    Metadata::new(),
                    vec![
                        Reference(Metadata::new(), x.clone()),
                        Reference(Metadata::new(), y.clone()),
                        Reference(Metadata::new(), z.clone())
                    ]
                )]
            )
        )
    }

    #[test]
    fn test_int() {
        // y is an IntDomain - only booleans should be allowed

        let mut model: Model = Model::default();

        let x: Name = Name::UserName(String::from('x'));
        let y: Name = Name::UserName(String::from('y'));

        model.add_variable(x.clone(), DecisionVariable { domain: BoolDomain });
        model.add_variable(
            y.clone(),
            DecisionVariable {
                domain: IntDomain(vec![]),
            },
        );

        model.add_constraint(Reference(Metadata::new(), x.clone()));
        model.add_constraint(Reference(Metadata::new(), y.clone()));

        let cnf: Result<CNFModel, SolverError> = CNFModel::from_conjure(model);
        assert!(cnf.is_err());
    }

    #[test]
    fn test_eq() {
        // Eq(x, y) - this operation is not allowed

        let mut model: Model = Model::default();

        let x: Name = Name::UserName(String::from('x'));
        let y: Name = Name::UserName(String::from('y'));

        model.add_variable(x.clone(), DecisionVariable { domain: BoolDomain });
        model.add_variable(y.clone(), DecisionVariable { domain: BoolDomain });

        model.add_constraint(Expression::Eq(
            Metadata::new(),
            Box::from(Reference(Metadata::new(), x.clone())),
            Box::from(Reference(Metadata::new(), y.clone())),
        ));

        let cnf: Result<CNFModel, SolverError> = CNFModel::from_conjure(model);
        assert!(cnf.is_err());
    }
}
