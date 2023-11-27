use crate::ast::{DecisionVariable, Domain, Expression, Model, Name};
use std::collections::HashMap;
use String;

type CNF = Vec<Vec<i32>>;
type Error = String;

/// Loop over variable names and assign indices to them for the CNF format. If a variable is not boolean, raise error.
fn make_indices(vars: &HashMap<Name, DecisionVariable>) -> Result<HashMap<&Name, i32>, Error> {
    let mut ans: HashMap<&Name, i32> = HashMap::new();
    let mut next_index = 1;

    for name in vars.keys() {
        let variable: &DecisionVariable = vars.get(name).unwrap();
        match variable.domain {
            Domain::BoolDomain => {
                ans.insert(name, next_index);
                next_index += 1;
            }
            _ => {
                return Err(String::from(
                    "Could not convert (Model -> CNF):\
                 all variables must belong to domain BoolDomain!",
                ))
            }
        }
    }

    return Ok(ans);
}

/// Convert a single expression into a row of the CNF format
fn convert_expression(
    expression: &Expression,
    symbol_table: &HashMap<&Name, i32>,
) -> Result<Vec<i32>, Error> {
    let mut ans: Vec<i32> = vec![];

    match expression {
        Expression::Reference(name) => match symbol_table.get(&name) {
            // If it's a variable, just get its index
            None => {
                let str_name = match name {
                    Name::UserName(s_name) => s_name.clone(),
                    Name::MachineName(i_name) => i_name.to_string(),
                };
                return Err(format!("Variable with name {str_name} not found!"));
            }
            Some(ind) => {
                ans.push(*ind);
            }
        },
        Expression::Not(var_box) => {
            let expr = var_box.as_ref();
            match expr {
                // Expression inside the Not()
                Expression::Reference(_) => {
                    // If it's a variable, get its index by calling
                    // convert_expression again, and add a -
                    let ret = convert_expression(expr, symbol_table)?;
                    let ind = *ret.first().unwrap();
                    ans.push(-ind);
                }
                _ => {
                    return Err(String::from(
                        "Expected Model to be in CNF form,\
                 expression inside Not must always be a Reference!",
                    ))
                }
            }
        }
        Expression::Or(expressions) => {
            // If it's an Or, we just need to convert expressions inside it and flatten the result
            for expr in expressions {
                let ret = convert_expression(expr, symbol_table)?;
                for ind in ret {
                    ans.push(ind);
                }
            }
        }
        _ => {
            return Err(String::from(
                "Expected Model to be in CNF form,\
        only Reference, Not(Reference) and Or(...) allowed!",
            ))
        }
    }

    return Ok(ans);
}

/// Expects Model to be in the Conjunctive Normal Form:
/// - All variables must be boolean
/// - Expressions must be Reference, Not(Reference), or Or(Reference1, Not(Reference2), ...)
impl TryFrom<Model> for CNF {
    type Error = Error;
    fn try_from(conjure_model: Model) -> Result<Self, Self::Error> {
        let mut ans: Vec<Vec<i32>> = vec![];
        let constraints = conjure_model.constraints;
        let variables = conjure_model.variables;

        let names_to_indices: HashMap<&Name, i32> = make_indices(&variables)?;

        for expression in constraints.iter() {
            match convert_expression(expression, &names_to_indices) {
                Ok(row) => ans.push(row),
                Err(msg) => return Err(format!("Could not convert (Model -> CNF): {msg}")),
            }
        }

        return Ok(ans);
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::Domain::BoolDomain;
    use crate::ast::Expression::{Not, Or, Reference};
    use crate::ast::Name;
    use crate::ast::{DecisionVariable, Model};
    use crate::solvers::kissat::CNF;
    use std::collections::{HashMap, HashSet};
    use std::fmt::Debug;
    use std::hash::Hash;
    use std::ops::Deref;

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
        let var = DecisionVariable { domain: BoolDomain };
        let name = Name::MachineName(1);

        let reference = Reference(name.clone());

        let vars: HashMap<Name, DecisionVariable> = HashMap::from([(name.clone(), var)]);
        let expressions = vec![reference];

        let model = Model {
            variables: vars,
            constraints: expressions,
        };

        let converted = CNF::try_from(model);
        if let Ok(ans) = converted {
            assert_eq!(ans, vec![vec![1]]);
        }
    }

    #[test]
    fn test_single_not() {
        let var = DecisionVariable { domain: BoolDomain };
        let name = Name::MachineName(1);

        let reference = Reference(name.clone());
        let not = Not(Box::from(reference));

        let vars: HashMap<Name, DecisionVariable> = HashMap::from([(name.clone(), var)]);
        let expressions = vec![not];

        let model = Model {
            variables: vars,
            constraints: expressions,
        };

        let converted = CNF::try_from(model);
        if let Ok(ans) = converted {
            assert_eq!(ans, vec![vec![-1]]);
        }
    }

    #[test]
    fn test_single_or() {
        let var1 = DecisionVariable { domain: BoolDomain };
        let name1 = Name::MachineName(1);
        let var2 = DecisionVariable { domain: BoolDomain };
        let name2 = Name::MachineName(2);

        let ref1 = Reference(name1.clone());
        let ref2 = Reference(name2.clone());
        let or = Or(vec![ref1, ref2]);

        let vars: HashMap<Name, DecisionVariable> =
            HashMap::from([(name1.clone(), var1), (name2.clone(), var2)]);
        let expressions = vec![or];

        let model = Model {
            variables: vars,
            constraints: expressions,
        };

        let converted = CNF::try_from(model);
        if let Ok(ans) = converted {
            assert_eq!(ans, vec![vec![1, 2]]);
        }
    }

    #[test]
    fn test_multiple_vars() {
        let var1 = DecisionVariable { domain: BoolDomain };
        let name1 = Name::MachineName(1);
        let var2 = DecisionVariable { domain: BoolDomain };
        let name2 = Name::MachineName(2);

        let ref1 = Reference(name1.clone());
        let ref2 = Reference(name2.clone());

        let vars: HashMap<Name, DecisionVariable> =
            HashMap::from([(name1.clone(), var1), (name2.clone(), var2)]);
        let expressions = vec![ref1, ref2];

        let model = Model {
            variables: vars,
            constraints: expressions,
        };

        let converted = CNF::try_from(model);
        if let Ok(ans) = converted {
            let corr = vec![vec![1], vec![2]];
            assert_eq_any_order(&ans, &corr);
        }
    }

    #[test]
    fn test_var_and_not() {
        let var1 = DecisionVariable { domain: BoolDomain };
        let name1 = Name::MachineName(1);
        let var2 = DecisionVariable { domain: BoolDomain };
        let name2 = Name::MachineName(2);

        let ref1 = Reference(name1.clone());
        let ref2 = Reference(name2.clone());
        let not2 = Not(Box::from(ref2));

        let vars: HashMap<Name, DecisionVariable> =
            HashMap::from([(name1.clone(), var1), (name2.clone(), var2)]);
        let expressions = vec![ref1, not2];

        let model = Model {
            variables: vars,
            constraints: expressions,
        };

        let converted = CNF::try_from(model);
        if let Ok(ans) = converted {
            let corr = vec![vec![1], vec![-2]];
            assert_eq_any_order(&ans, &corr);
        }
    }

    #[test]
    fn test_or_not() {
        let var1 = DecisionVariable { domain: BoolDomain };
        let name1 = Name::MachineName(1);
        let var2 = DecisionVariable { domain: BoolDomain };
        let name2 = Name::MachineName(2);

        let ref1 = Reference(name1.clone());
        let ref2 = Reference(name2.clone());
        let not2 = Not(Box::from(ref2));
        let or = Or(vec![ref1, not2]);

        let vars: HashMap<Name, DecisionVariable> =
            HashMap::from([(name1.clone(), var1), (name2.clone(), var2)]);
        let expressions = vec![or];

        let model = Model {
            variables: vars,
            constraints: expressions,
        };

        let converted = CNF::try_from(model);
        if let Ok(ans) = converted {
            let corr = vec![vec![1, -2]];
            assert_eq_any_order(&ans, &corr);
        }
    }

    #[test]
    fn test_multiple_ors() {
        let var1 = DecisionVariable { domain: BoolDomain };
        let name1 = Name::MachineName(1);
        let var2 = DecisionVariable { domain: BoolDomain };
        let name2 = Name::MachineName(2);
        let var3 = DecisionVariable { domain: BoolDomain };
        let name3 = Name::MachineName(3);
        let var4 = DecisionVariable { domain: BoolDomain };
        let name4 = Name::MachineName(4);

        let ref1 = Reference(name1.clone());
        let ref2 = Reference(name2.clone());
        let ref3 = Reference(name3.clone());
        let ref4 = Reference(name4.clone());

        let or1 = Or(vec![ref1, ref2]);
        let or2 = Or(vec![ref3, ref4]);

        let vars: HashMap<Name, DecisionVariable> =
            HashMap::from([(name1.clone(), var1), (name2.clone(), var2)]);
        let expressions = vec![or1, or2];

        let model = Model {
            variables: vars,
            constraints: expressions,
        };

        let converted = CNF::try_from(model);
        if let Ok(ans) = converted {
            let corr = vec![vec![1, 2], vec![3, 4]];
            assert_eq_any_order(&ans, &corr);
        }
    }

    #[test]
    fn test_nested_ors() {
        let var1 = DecisionVariable { domain: BoolDomain };
        let name1 = Name::MachineName(1);
        let var2 = DecisionVariable { domain: BoolDomain };
        let name2 = Name::MachineName(2);
        let var3 = DecisionVariable { domain: BoolDomain };
        let name3 = Name::MachineName(3);
        let var4 = DecisionVariable { domain: BoolDomain };
        let name4 = Name::MachineName(4);

        let ref1 = Reference(name1.clone());
        let ref2 = Reference(name2.clone());
        let ref3 = Reference(name3.clone());
        let ref4 = Reference(name4.clone());

        let or1 = Or(vec![ref1, ref2]);
        let or2 = Or(vec![ref3, ref4]);
        let or = Or(vec![or1, or2]);

        let vars: HashMap<Name, DecisionVariable> =
            HashMap::from([(name1.clone(), var1), (name2.clone(), var2)]);
        let expressions = vec![or];

        let model = Model {
            variables: vars,
            constraints: expressions,
        };

        let converted = CNF::try_from(model);
        if let Ok(ans) = converted {
            let corr = vec![vec![1, 2, 3, 4]];
            assert_eq_any_order(&ans, &corr);
        }
    }

    #[test]
    fn test_invalid() {}
}
