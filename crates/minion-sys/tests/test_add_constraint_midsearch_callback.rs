use std::collections::HashMap;
use std::sync::Mutex;

use minion_sys::ast::{Constant, Constraint, Model, Var, VarDomain, VarName};
use minion_sys::error::MinionError;

static SEEN_F_VALUES: Mutex<Vec<i32>> = Mutex::new(vec![]);
static POSTED_EXTRA_CONSTRAINT: Mutex<bool> = Mutex::new(false);
static SAW_IMMEDIATE_FAILURE_ON_POST: Mutex<bool> = Mutex::new(false);
static CALLBACK_ERROR: Mutex<Option<String>> = Mutex::new(None);

#[test]
#[allow(clippy::panic_in_result_fn)]
fn test_add_constraint_midsearch_from_callback() -> Result<(), MinionError> {
    {
        let mut seen = SEEN_F_VALUES.lock().unwrap();
        seen.clear();
    }
    {
        let mut posted = POSTED_EXTRA_CONSTRAINT.lock().unwrap();
        *posted = false;
    }
    {
        let mut saw_failure = SAW_IMMEDIATE_FAILURE_ON_POST.lock().unwrap();
        *saw_failure = false;
    }
    {
        let mut callback_error = CALLBACK_ERROR.lock().unwrap();
        *callback_error = None;
    }

    let mut model = Model::new();
    model
        .named_variables
        .add_var(String::from("f"), VarDomain::Bound(0, 2));

    minion_sys::run_minion(model, callback)?;

    let callback_error = CALLBACK_ERROR.lock().unwrap();
    assert!(
        callback_error.is_none(),
        "Callback failed: {}",
        callback_error.clone().unwrap_or_default()
    );

    let posted = POSTED_EXTRA_CONSTRAINT.lock().unwrap();
    assert!(*posted, "Expected callback to post a mid-search constraint");

    let saw_failure = SAW_IMMEDIATE_FAILURE_ON_POST.lock().unwrap();
    assert!(
        *saw_failure,
        "Expected immediate failure when posting f in {{2}} at solution f = 0"
    );

    let seen = SEEN_F_VALUES.lock().unwrap();
    assert_eq!(*seen, vec![0, 2]);

    Ok(())
}

fn callback(solution_set: HashMap<VarName, Constant>) -> bool {
    let f = match solution_set.get("f") {
        Some(Constant::Integer(v)) => *v,
        Some(other) => {
            *CALLBACK_ERROR.lock().unwrap() = Some(format!(
                "Expected integer value for f, got {other:?}"
            ));
            return false;
        }
        None => {
            *CALLBACK_ERROR.lock().unwrap() = Some("Callback solution set did not contain f".into());
            return false;
        }
    };

    SEEN_F_VALUES.lock().unwrap().push(f);

    let mut posted = POSTED_EXTRA_CONSTRAINT.lock().unwrap();
    if !*posted {
        if f != 0 {
            *CALLBACK_ERROR.lock().unwrap() = Some(format!(
                "Expected first solution to be f = 0, got {f}"
            ));
            return false;
        }

        // Domain is 0..2, so forcing f into {2} is equivalent to forcing f > 1.
        let posted_ok = match minion_sys::add_constraint_midsearch(&Constraint::WInset(
            Var::NameRef(String::from("f")),
            vec![Constant::Integer(2)],
        )) {
            Ok(ok) => ok,
            Err(err) => {
                *CALLBACK_ERROR.lock().unwrap() =
                    Some(format!("Failed to post mid-search constraint: {err}"));
                return false;
            }
        };

        if !posted_ok {
            // This is expected: at the first solution f = 0, posting f in {2}
            // immediately fails the current branch, then search should backtrack.
            *SAW_IMMEDIATE_FAILURE_ON_POST.lock().unwrap() = true;
        }

        *posted = true;
    }

    true
}
