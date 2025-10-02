//! based on this minion test file:
//! https://github.com/minion/minion/blob/main/test_instances/test_watchedor_reifyimply_1.minion
//!
//! ```text
//! #TEST SOLCOUNT 7
//! # Recursive test
//! MINION 3
//!
//! **VARIABLES**
//! BOOL a
//! BOOL b
//! BOOL c
//!
//! **CONSTRAINTS**
//!
//! reifyimply(watched-or({w-inset(a,[1]),w-inset(b,[0])}), c)
//!
//! **EOF**
//! ```

use std::collections::HashMap;
use std::sync::Mutex;

use minion_sys::ast::{Constant, Constraint, Model, Var, VarDomain, VarName};
use minion_sys::error::MinionError;
use minion_sys::get_from_table;
#[test]
#[allow(clippy::panic_in_result_fn)]
fn test_watchedor_reifyimply_1() -> Result<(), MinionError> {
    let mut model = Model::new();
    model
        .named_variables
        .add_var(String::from("a"), VarDomain::Bool);
    model
        .named_variables
        .add_var(String::from("b"), VarDomain::Bool);
    model
        .named_variables
        .add_var(String::from("c"), VarDomain::Bool);

    model.constraints.push(Constraint::ReifyImply(
        Box::new(Constraint::WatchedOr(vec![
            Constraint::WInset(Var::NameRef(String::from("a")), vec![Constant::Bool(true)]),
            Constraint::WInset(Var::NameRef(String::from("b")), vec![Constant::Bool(false)]),
        ])),
        Var::NameRef(String::from("c")),
    ));

    minion_sys::run_minion(model, callback)?;

    let guard = SOLS_COUNTER.lock().unwrap();
    assert_eq!(*guard, 7);
    assert_ne!(get_from_table("Nodes".into()), None);
    Ok(())
}

static SOLS_COUNTER: Mutex<i32> = Mutex::new(0);
fn callback(_: HashMap<VarName, Constant>) -> bool {
    #[allow(clippy::unwrap_used)]
    let mut guard = SOLS_COUNTER.lock().unwrap();
    *guard += 1;
    true
}
