use crate::ast::Domain::BoolDomain;
use crate::ast::Expression::{And, Not, Or, Reference};
use crate::ast::{DecisionVariable, Domain, Expression, Model, Name};
use std::collections::HashMap;
use String;

type Error = String;

struct CNF {
    pub clauses: Vec<Vec<i32>>,
    variables: HashMap<Name, i32>,
    next_ind: i32,
}

impl CNF {
    pub fn new() -> CNF {
        CNF {
            clauses: Vec::new(),
            variables: HashMap::new(),
            next_ind: 1,
        }
    }

    pub fn get_variables(&self) -> Vec<&Name> {
        let mut ans: Vec<&Name> = Vec::new();

        for key in self.variables.keys() {
            ans.push(key);
        }

        return ans;
    }

    pub fn get_index(&self, var: &Name) -> Option<i32> {
        return match self.variables.get(var) {
            None => None,
            Some(idx) => Some(*idx),
        };
    }

    pub fn get_name(&self, ind: i32) -> Option<&Name> {
        for key in self.variables.keys() {
            let idx = self.get_index(key)?;
            if idx == ind {
                return Some(key);
            }
        }
        return None;
    }

    pub fn add_variable(&mut self, var: &Name) {
        self.variables.insert(var.clone(), self.next_ind);
        self.next_ind += 1;
    }

    pub fn has_variable<T: HasVariable>(&self, value: T) -> bool {
        value.has_variable(self)
    }

    pub fn add_clause(&mut self, vec: &Vec<i32>) -> Result<(), Error> {
        for idx in vec {
            if !self.has_variable(idx.abs()) {
                return Err(format!("Variable with index {idx} not found!"));
            }
        }
        self.clauses.push(vec.clone());
        return Ok(());
    }

    pub fn add_expression(&mut self, expr: &Expression) -> Result<(), Error> {
        for row in self.handle_expression(expr)? {
            self.add_clause(&row)?;
        }
        return Ok(());
    }

    pub fn as_expression(&self) -> Result<Expression, Error> {
        let mut expr_clauses: Vec<Expression> = Vec::new();

        for clause in &self.clauses {
            expr_clauses.push(self.clause_to_expression(clause)?);
        }

        // ToDo (gs248) We should probably flatten the result first. x -> And( [ Or([x]) ] ) is a bit silly.
        // Also, this functionality may not be needed altogether
        return Ok(And(expr_clauses));
    }

    fn clause_to_expression(&self, clause: &Vec<i32>) -> Result<Expression, Error> {
        let mut ans: Vec<Expression> = Vec::new();

        for idx in clause {
            match self.get_name(idx.abs()) {
                None => return Err(format!("Could not find variable with index {idx}")),
                Some(name) => {
                    if *idx > 0 {
                        ans.push(Reference(name.clone()))
                    } else {
                        ans.push(Not(Box::from(Reference(name.clone()))))
                    }
                }
            }
        }

        return Ok(Or(ans));
    }

    fn get_reference_index(&self, name: &Name) -> Result<i32, Error> {
        return match self.get_index(name) {
            None => {
                let str_name = match name {
                    Name::UserName(s_name) => s_name.clone(),
                    Name::MachineName(i_name) => i_name.to_string(),
                };
                Err(format!("Variable with name {str_name} not found!"))
            }
            Some(ind) => Ok(ind),
        };
    }

    fn handle_reference(&self, name: &Name) -> Result<Vec<i32>, Error> {
        return Ok(vec![self.get_reference_index(name)?]);
    }

    fn handle_not(&self, expr_box: &Box<Expression>) -> Result<Vec<i32>, Error> {
        let expr = expr_box.as_ref();
        return match expr {
            // Expression inside the Not()
            Reference(name) => Ok(vec![-self.get_reference_index(name)?]),
            _ => Err(String::from(
                "Expected Model to be in CNF form,\
                 expression inside Not must always be a Reference!",
            )),
        };
    }

    fn handle_or(&self, expressions: &Vec<Expression>) -> Result<Vec<i32>, Error> {
        let mut ans: Vec<i32> = Vec::new();

        for expr in expressions {
            let ret = self.handle_flat_expression(expr)?;
            for ind in ret {
                ans.push(ind);
            }
        }

        return Ok(ans);
    }

    /// Convert a single Reference, Not or Or into a row of the CNF format
    fn handle_flat_expression(&self, expression: &Expression) -> Result<Vec<i32>, Error> {
        return match expression {
            Reference(name) => self.handle_reference(name),
            Not(var_box) => self.handle_not(var_box),
            Or(expressions) => self.handle_or(expressions),
            _ => Err(String::from(
                "Expected Model to be in CNF form,\
        only Reference, Not(Reference) and Or(...) allowed!",
            )),
        };
    }

    fn handle_and(&self, expressions: &Vec<Expression>) -> Result<Vec<Vec<i32>>, Error> {
        let mut ans: Vec<Vec<i32>> = Vec::new();

        for expression in expressions {
            match expression {
                And(expressions) => {
                    return Err(String::from("Nested And expressions not allowed!"));
                }
                _ => {
                    ans.push(self.handle_flat_expression(expression)?);
                }
            }
        }

        return Ok(ans);
    }

    fn handle_expression(&self, expression: &Expression) -> Result<Vec<Vec<i32>>, Error> {
        return match expression {
            And(expressions) => self.handle_and(expressions),
            _ => Ok(vec![self.handle_flat_expression(expression)?]),
        };
    }
}

trait HasVariable {
    fn has_variable(self, cnf: &CNF) -> bool;
}

impl HasVariable for i32 {
    fn has_variable(self, cnf: &CNF) -> bool {
        return cnf.get_name(self).is_some();
    }
}

impl HasVariable for &Name {
    fn has_variable(self, cnf: &CNF) -> bool {
        return cnf.get_index(self).is_some();
    }
}

/// Expects Model to be in the Conjunctive Normal Form:
/// - All variables must be boolean
/// - Expressions must be Reference, Not(Reference), or Or(Reference1, Not(Reference2), ...)
impl TryFrom<Model> for CNF {
    type Error = Error;

    fn try_from(model: Model) -> Result<CNF, Error> {
        let mut ans: CNF = CNF::new();

        for var in model.variables.keys() {
            // Check that domain has the correct type
            let decision_var = model.variables.get(var).unwrap();
            if decision_var.domain != BoolDomain {
                return Err(format!(
                    "Unexpected domain in variable {decision_var}! Only BoolDomain is allowed."
                ));
            }

            ans.add_variable(var);
        }

        for expr in model.constraints {
            ans.add_expression(&expr)?;
        }

        return Ok(ans);
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::Domain::{BoolDomain, IntDomain};
    use crate::ast::Expression::{And, Not, Or, Reference};
    use crate::ast::{DecisionVariable, Model};
    use crate::ast::{Expression, Name};
    use crate::solvers::kissat::{Error, CNF};
    use std::collections::HashSet;
    use std::fmt::Debug;
    use std::hash::Hash;

    fn to_set<T: Eq + Hash + Debug + Clone>(a: &Vec<T>) -> HashSet<T> {
        let mut a_set: HashSet<T> = HashSet::new();
        for el in a {
            a_set.insert(el.clone());
        }
        return a_set;
    }

    fn assert_eq_any_order<T: Eq + Hash + Debug + Clone>(a: &Vec<Vec<T>>, b: &Vec<Vec<T>>) {
        assert_eq!(a.len(), b.len());

        let mut a_rows: Vec<HashSet<T>> = Vec::new();
        for row in a {
            let hash_row = to_set(row);
            a_rows.push(hash_row);
        }

        let mut b_rows: Vec<HashSet<T>> = Vec::new();
        for row in b {
            let hash_row = to_set(row);
            b_rows.push(hash_row);
        }

        println!("{:?},{:?}", a_rows, b_rows);
        for row in a_rows {
            assert!(b_rows.contains(&row));
        }
    }

    #[test]
    fn test_single_var() {
        // x -> [[1]]

        let mut model: Model = Model::new();

        let x: Name = Name::UserName(String::from('x'));
        model.add_variable(x.clone(), DecisionVariable { domain: BoolDomain });
        model.add_constraint(Expression::Reference(x.clone()));

        let res: Result<CNF, Error> = CNF::try_from(model);
        assert!(res.is_ok());

        let cnf = res.unwrap();

        assert_eq!(cnf.get_index(&x), Some(1));
        assert!(cnf.get_name(1).is_some());
        assert_eq!(cnf.get_name(1).unwrap(), &x);

        assert_eq!(cnf.clauses, vec![vec![1]]);

        assert_eq!(
            cnf.as_expression(),
            Ok(And(vec![Or(vec![Reference(x.clone())])]))
        )
    }

    #[test]
    fn test_single_not() {
        // Not(x) -> [[-1]]

        let mut model: Model = Model::new();

        let x: Name = Name::UserName(String::from('x'));
        model.add_variable(x.clone(), DecisionVariable { domain: BoolDomain });
        model.add_constraint(Not(Box::from(Reference(x.clone()))));

        let cnf: CNF = CNF::try_from(model).unwrap();
        assert_eq!(cnf.get_index(&x), Some(1));
        assert_eq!(cnf.clauses, vec![vec![-1]]);

        assert_eq!(
            cnf.as_expression(),
            Ok(And(vec![Or(vec![Not(Box::from(Reference(x.clone())))])]))
        )
    }

    #[test]
    fn test_single_or() {
        // Or(x, y) -> [[1, 2]]

        let mut model: Model = Model::new();

        let x: Name = Name::UserName(String::from('x'));
        let y: Name = Name::UserName(String::from('y'));

        model.add_variable(x.clone(), DecisionVariable { domain: BoolDomain });
        model.add_variable(y.clone(), DecisionVariable { domain: BoolDomain });

        model.add_constraint(Or(vec![Reference(x.clone()), Reference(y.clone())]));

        let cnf: CNF = CNF::try_from(model).unwrap();

        let xi = cnf.get_index(&x).unwrap();
        let yi = cnf.get_index(&y).unwrap();
        assert_eq_any_order(&cnf.clauses, &vec![vec![xi, yi]]);

        assert_eq!(
            cnf.as_expression(),
            Ok(And(vec![Or(vec![
                Reference(x.clone()),
                Reference(y.clone())
            ])]))
        )
    }

    #[test]
    fn test_or_not() {
        // Or(x, Not(y)) -> [[1, -2]]

        let mut model: Model = Model::new();

        let x: Name = Name::UserName(String::from('x'));
        let y: Name = Name::UserName(String::from('y'));

        model.add_variable(x.clone(), DecisionVariable { domain: BoolDomain });
        model.add_variable(y.clone(), DecisionVariable { domain: BoolDomain });

        model.add_constraint(Or(vec![
            Reference(x.clone()),
            Not(Box::from(Reference(y.clone()))),
        ]));

        let cnf: CNF = CNF::try_from(model).unwrap();

        let xi = cnf.get_index(&x).unwrap();
        let yi = cnf.get_index(&y).unwrap();
        assert_eq_any_order(&cnf.clauses, &vec![vec![xi, -yi]]);

        assert_eq!(
            cnf.as_expression(),
            Ok(And(vec![Or(vec![
                Reference(x.clone()),
                Not(Box::from(Reference(y.clone())))
            ])]))
        )
    }

    #[test]
    fn test_multiple() {
        // [x, y] - equivalent to And(x, y) -> [[1], [2]]

        let mut model: Model = Model::new();

        let x: Name = Name::UserName(String::from('x'));
        let y: Name = Name::UserName(String::from('y'));

        model.add_variable(x.clone(), DecisionVariable { domain: BoolDomain });
        model.add_variable(y.clone(), DecisionVariable { domain: BoolDomain });

        model.add_constraint(Reference(x.clone()));
        model.add_constraint(Reference(y.clone()));

        let cnf: CNF = CNF::try_from(model).unwrap();

        let xi = cnf.get_index(&x).unwrap();
        let yi = cnf.get_index(&y).unwrap();
        assert_eq_any_order(&cnf.clauses, &vec![vec![xi], vec![yi]]);

        assert_eq!(
            cnf.as_expression(),
            Ok(And(vec![
                Or(vec![Reference(x.clone())]),
                Or(vec![Reference(y.clone())])
            ]))
        )
    }

    #[test]
    fn test_and() {
        // And(x, y) -> [[1], [2]]

        let mut model: Model = Model::new();

        let x: Name = Name::UserName(String::from('x'));
        let y: Name = Name::UserName(String::from('y'));

        model.add_variable(x.clone(), DecisionVariable { domain: BoolDomain });
        model.add_variable(y.clone(), DecisionVariable { domain: BoolDomain });

        model.add_constraint(And(vec![Reference(x.clone()), Reference(y.clone())]));

        let cnf: CNF = CNF::try_from(model).unwrap();

        let xi = cnf.get_index(&x).unwrap();
        let yi = cnf.get_index(&y).unwrap();
        assert_eq_any_order(&cnf.clauses, &vec![vec![xi], vec![yi]]);

        assert_eq!(
            cnf.as_expression(),
            Ok(And(vec![
                Or(vec![Reference(x.clone())]),
                Or(vec![Reference(y.clone())])
            ]))
        )
    }

    #[test]
    fn test_nested_ors() {
        // Or(x, Or(y, z)) -> [[1, 2, 3]]

        let mut model: Model = Model::new();

        let x: Name = Name::UserName(String::from('x'));
        let y: Name = Name::UserName(String::from('y'));
        let z: Name = Name::UserName(String::from('z'));

        model.add_variable(x.clone(), DecisionVariable { domain: BoolDomain });
        model.add_variable(y.clone(), DecisionVariable { domain: BoolDomain });
        model.add_variable(z.clone(), DecisionVariable { domain: BoolDomain });

        model.add_constraint(Or(vec![
            Reference(x.clone()),
            Or(vec![Reference(y.clone()), Reference(z.clone())]),
        ]));

        let cnf: CNF = CNF::try_from(model).unwrap();

        let xi = cnf.get_index(&x).unwrap();
        let yi = cnf.get_index(&y).unwrap();
        let zi = cnf.get_index(&z).unwrap();
        assert_eq_any_order(&cnf.clauses, &vec![vec![xi, yi, zi]]);

        assert_eq!(
            cnf.as_expression(),
            Ok(And(vec![Or(vec![
                Reference(x.clone()),
                Reference(y.clone()),
                Reference(z.clone())
            ])]))
        )
    }

    #[test]
    fn test_int() {
        // y is an IntDomain - only booleans should be allowed

        let mut model: Model = Model::new();

        let x: Name = Name::UserName(String::from('x'));
        let y: Name = Name::UserName(String::from('y'));

        model.add_variable(x.clone(), DecisionVariable { domain: BoolDomain });
        model.add_variable(
            y.clone(),
            DecisionVariable {
                domain: IntDomain(vec![]),
            },
        );

        model.add_constraint(Reference(x.clone()));
        model.add_constraint(Reference(y.clone()));

        let cnf: Result<CNF, Error> = CNF::try_from(model);
        assert!(cnf.is_err());
    }

    #[test]
    fn test_eq() {
        // Eq(x, y) - this operation is not allowed

        let mut model: Model = Model::new();

        let x: Name = Name::UserName(String::from('x'));
        let y: Name = Name::UserName(String::from('y'));

        model.add_variable(x.clone(), DecisionVariable { domain: BoolDomain });
        model.add_variable(y.clone(), DecisionVariable { domain: BoolDomain });

        model.add_constraint(Expression::Eq(
            Box::from(Reference(x.clone())),
            Box::from(Reference(y.clone())),
        ));

        let mut model: Model = Model::new();

        let x: Name = Name::UserName(String::from('x'));
        let y: Name = Name::UserName(String::from('y'));

        model.add_variable(x.clone(), DecisionVariable { domain: BoolDomain });
        model.add_variable(y.clone(), DecisionVariable { domain: BoolDomain });
    }
}
