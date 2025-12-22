// There are three letter x, y, and z, which are each integers between 0 and 1 inclusive, where its binary and accepts every binary triple except all zeros and all ones

use std::collections::HashMap;
use std::sync::Mutex;

use minion_sys::ast::{Constant, Constraint, Model, Var, VarDomain, VarName};
use minion_sys::error::MinionError;
use minion_sys::get_from_table;

#[test]
#[allow(clippy::panic_in_result_fn)]
fn test_negative_table_constraint() -> Result<(), MinionError> {
    let mut model = Model::new();

    // Declares variables (x, y, z), explicitly setting the integers to be between 0 and 1 
    model
        .named_variables
        .add_var(String::from("x"), VarDomain::Bound(0, 1));
    model
        .named_variables
        .add_var(String::from("y"), VarDomain::Bound(0, 1));
    model
        .named_variables
        .add_var(String::from("z"), VarDomain::Bound(0, 1));

    // Defines the forbidden table data (a list of tuples)
    let forbidden_table_data = vec![
        vec![Constant::Integer(1), Constant::Integer(1), Constant::Integer(1)],
        vec![Constant::Integer(0), Constant::Integer(0), Constant::Integer(0)],
    ];
    // Builds the Table constraint 
    let vars = vec![
        Var::NameRef(String::from("x")),
        Var::NameRef(String::from("y")),
        Var::NameRef(String::from("z")),
    ];
    
    model
        .constraints
        .push(Constraint::NegativeTable(vars, forbidden_table_data));

    // Runs the solver via the Minion interface
    minion_sys::run_minion(model, callback)?;

    // Asserts that we found exactly 3 solutions (one for each row in the table)
    let guard = SOLS_COUNTER.lock().unwrap();
    assert_eq!(*guard, 6);
    assert_ne!(get_from_table("Nodes".into()), None);
    Ok(())
}

// Global thread safe counter to store the number of solutions found
static SOLS_COUNTER: Mutex<u32> = Mutex::new(0);
// Callback function increments the counter for every solution
fn callback(_: HashMap<VarName, Constant>) -> bool {
    #[allow(clippy::unwrap_used)]
    let mut guard = SOLS_COUNTER.lock().unwrap();
    *guard += 1;
    true
}