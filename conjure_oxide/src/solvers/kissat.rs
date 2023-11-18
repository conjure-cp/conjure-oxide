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
