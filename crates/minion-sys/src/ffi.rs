#![allow(warnings)]

use std::ffi::CString;
use std::sync::atomic::{AtomicI32, Ordering};
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(test)]
mod tests {
    use std::ffi::{CString, c_char, c_void};
    use std::process::{Command, ExitStatus};

    use super::*;

    // solutions
    static X_VAL: AtomicI32 = AtomicI32::new(0);
    static Y_VAL: AtomicI32 = AtomicI32::new(0);
    static Z_VAL: AtomicI32 = AtomicI32::new(0);
    const MIDSEARCH_SCENARIO_ENV: &str = "MINION_MIDSEARCH_SCENARIO";
    const MIDSEARCH_CHILD_TEST: &str = "ffi::tests::midsearch_child_runner";

    #[derive(Clone, Copy, Debug)]
    enum MidsearchScenario {
        AddVarOnly,
        AddVarThenAddEqConstraint,
    }

    impl MidsearchScenario {
        fn as_env_value(self) -> &'static str {
            match self {
                MidsearchScenario::AddVarOnly => "add_var_only",
                MidsearchScenario::AddVarThenAddEqConstraint => "add_var_then_add_eq_constraint",
            }
        }

        fn from_env_value(value: &str) -> Option<Self> {
            match value {
                "add_var_only" => Some(MidsearchScenario::AddVarOnly),
                "add_var_then_add_eq_constraint" => {
                    Some(MidsearchScenario::AddVarThenAddEqConstraint)
                }
                _ => None,
            }
        }
    }

    #[derive(Debug)]
    struct MidsearchState {
        scenario: MidsearchScenario,
        instance: *mut ProbSpec_CSPInstance,
        callback_count: u32,
        added_var_ok: bool,
        looked_up_new_var_ok: bool,
        add_constraint_result: Option<bool>,
    }

    #[derive(Debug)]
    struct MidsearchOutcome {
        run_code: ReturnCodes,
        callback_count: u32,
        added_var_ok: bool,
        looked_up_new_var_ok: bool,
        add_constraint_result: Option<bool>,
    }

    #[derive(Default, Debug)]
    struct SpawnStats {
        ok: usize,
        nonzero: usize,
        signal_11: usize,
        signal_6: usize,
        other_signal: usize,
    }

    #[cfg(unix)]
    fn status_signal(status: &ExitStatus) -> Option<i32> {
        use std::os::unix::process::ExitStatusExt;
        status.signal()
    }

    #[cfg(not(unix))]
    fn status_signal(_: &ExitStatus) -> Option<i32> {
        None
    }

    fn classify_status(status: &ExitStatus, stats: &mut SpawnStats) {
        if status.success() {
            stats.ok += 1;
            return;
        }

        stats.nonzero += 1;

        match status_signal(status) {
            Some(11) => stats.signal_11 += 1,
            Some(6) => stats.signal_6 += 1,
            Some(_) => stats.other_signal += 1,
            None => {}
        }
    }

    unsafe fn build_two_var_instance(instance: *mut ProbSpec_CSPInstance) {
        let x_name = b"x\0";
        let y_name = b"y\0";

        assert!(newVar_ffi_safe(
            instance,
            x_name.as_ptr() as *mut c_char,
            VariableType_VAR_BOUND,
            0,
            1
        ));
        assert!(newVar_ffi_safe(
            instance,
            y_name.as_ptr() as *mut c_char,
            VariableType_VAR_BOUND,
            0,
            1
        ));

        let mut x_var = ProbSpec_Var {
            type_m: VariableType_VAR_CONSTANT,
            pos_m: 0,
        };
        let mut y_var = ProbSpec_Var {
            type_m: VariableType_VAR_CONSTANT,
            pos_m: 0,
        };
        assert!(getVarByName_safe(
            instance,
            x_name.as_ptr() as *mut c_char,
            &mut x_var
        ));
        assert!(getVarByName_safe(
            instance,
            y_name.as_ptr() as *mut c_char,
            &mut y_var
        ));

        printMatrix_addVar(instance, x_var);
        printMatrix_addVar(instance, y_var);

        let search_vars = vec_var_new();
        vec_var_push_back(search_vars, x_var);
        vec_var_push_back(search_vars, y_var);
        let search_order = searchOrder_new(search_vars, VarOrderEnum_ORDER_STATIC, false);
        instance_addSearchOrder(instance, search_order);
    }

    unsafe extern "C" fn midsearch_mutation_callback(
        ctx: *mut MinionContext,
        userdata: *mut c_void,
    ) -> bool {
        let state = &mut *(userdata as *mut MidsearchState);
        state.callback_count += 1;

        if state.callback_count > 1 {
            return false;
        }

        let dyn_name = b"dyn_mid\0";
        state.added_var_ok = newVar_ffi_safe(
            state.instance,
            dyn_name.as_ptr() as *mut c_char,
            VariableType_VAR_BOUND,
            0,
            1,
        );

        let mut dyn_var = ProbSpec_Var {
            type_m: VariableType_VAR_CONSTANT,
            pos_m: 0,
        };
        state.looked_up_new_var_ok = getVarByName_safe(
            state.instance,
            dyn_name.as_ptr() as *mut c_char,
            &mut dyn_var,
        );

        if matches!(state.scenario, MidsearchScenario::AddVarThenAddEqConstraint)
            && state.added_var_ok
            && state.looked_up_new_var_ok
        {
            let x_name = b"x\0";
            let mut x_var = ProbSpec_Var {
                type_m: VariableType_VAR_CONSTANT,
                pos_m: 0,
            };
            if getVarByName_safe(state.instance, x_name.as_ptr() as *mut c_char, &mut x_var) {
                let eq = constraint_new(ConstraintType_CT_EQ);
                constraint_addVar(eq, &mut dyn_var);
                constraint_addVar(eq, &mut x_var);
                state.add_constraint_result =
                    Some(instance_addConstraintMidsearch(ctx, state.instance, eq));
            } else {
                state.add_constraint_result = Some(false);
            }
        }

        false
    }

    unsafe fn run_midsearch_scenario_once(scenario: MidsearchScenario) -> MidsearchOutcome {
        let ctx = minion_newContext();
        let options = searchOptions_new();
        let args = searchMethod_new();
        let instance = instance_new();

        (*options).silent = true;
        (*options).print_solution = false;

        build_two_var_instance(instance);

        let mut state = MidsearchState {
            scenario,
            instance,
            callback_count: 0,
            added_var_ok: false,
            looked_up_new_var_ok: false,
            add_constraint_result: None,
        };

        let run_code = runMinion(
            ctx,
            options,
            args,
            instance,
            Some(midsearch_mutation_callback),
            (&mut state as *mut MidsearchState).cast::<c_void>(),
        );

        minion_freeContext(ctx);

        MidsearchOutcome {
            run_code,
            callback_count: state.callback_count,
            added_var_ok: state.added_var_ok,
            looked_up_new_var_ok: state.looked_up_new_var_ok,
            add_constraint_result: state.add_constraint_result,
        }
    }

    fn run_midsearch_child(scenario: MidsearchScenario) -> ExitStatus {
        let current_test_binary =
            std::env::current_exe().expect("could not find current test binary");

        Command::new(current_test_binary)
            .arg("--exact")
            .arg(MIDSEARCH_CHILD_TEST)
            .arg("--nocapture")
            .env(MIDSEARCH_SCENARIO_ENV, scenario.as_env_value())
            .status()
            .expect("could not execute child test")
    }

    pub extern "C" fn hello_from_rust(ctx: *mut MinionContext, _userdata: *mut c_void) -> bool {
        unsafe {
            X_VAL.store(printMatrix_getValue(ctx, 0) as _, Ordering::Relaxed);
            Y_VAL.store(printMatrix_getValue(ctx, 1) as _, Ordering::Relaxed);
            Z_VAL.store(printMatrix_getValue(ctx, 2) as _, Ordering::Relaxed);
            return true;
        }
    }

    #[test]
    fn xyz_raw() {
        // A simple constraints model, manually written using FFI functions.
        // Testing to see if it does not segfault.
        // Results can be manually inspected in the outputted minion logs.
        unsafe {
            // See https://rust-lang.github.io/rust-bindgen/cpp.html
            let ctx = minion_newContext();
            let options = searchOptions_new();
            let args = searchMethod_new();
            let instance = instance_new();

            let x_str = CString::new("x").expect("bad x");
            let y_str = CString::new("y").expect("bad y");
            let z_str = CString::new("z").expect("bad z");

            newVar_ffi(instance, x_str.as_ptr() as _, VariableType_VAR_BOUND, 1, 3);
            newVar_ffi(instance, y_str.as_ptr() as _, VariableType_VAR_BOUND, 2, 4);
            newVar_ffi(instance, z_str.as_ptr() as _, VariableType_VAR_BOUND, 1, 5);

            let x = getVarByName(instance, x_str.as_ptr() as _);
            let y = getVarByName(instance, y_str.as_ptr() as _);
            let z = getVarByName(instance, z_str.as_ptr() as _);

            // PRINT
            printMatrix_addVar(instance, x);
            printMatrix_addVar(instance, y);
            printMatrix_addVar(instance, z);

            // VARORDER
            let search_vars = vec_var_new();
            vec_var_push_back(search_vars as _, x);
            vec_var_push_back(search_vars as _, y);
            vec_var_push_back(search_vars as _, z);
            let search_order = searchOrder_new(search_vars as _, VarOrderEnum_ORDER_STATIC, false);
            instance_addSearchOrder(instance, search_order);

            // CONSTRAINTS
            let leq = constraint_new(ConstraintType_CT_LEQSUM);
            let geq = constraint_new(ConstraintType_CT_GEQSUM);
            let ineq = constraint_new(ConstraintType_CT_INEQ);

            let rhs_vars = vec_var_new();
            vec_var_push_back(rhs_vars, constantAsVar(4));

            // leq / geq : [var] [var]
            constraint_addList(leq, search_vars as _);
            constraint_addList(leq, rhs_vars as _);

            constraint_addList(geq, search_vars as _);
            constraint_addList(geq, rhs_vars as _);

            // ineq: [var] [var] [const]
            let x_vec = vec_var_new();
            vec_var_push_back(x_vec, x);

            let y_vec = vec_var_new();
            vec_var_push_back(y_vec, y);

            let const_vec = vec_int_new();
            vec_int_push_back(const_vec, -1);

            constraint_addList(ineq, x_vec as _);
            constraint_addList(ineq, y_vec as _);
            constraint_addConstantList(ineq, const_vec as _);

            instance_addConstraint(instance, leq);
            instance_addConstraint(instance, geq);
            instance_addConstraint(instance, ineq);

            let res = runMinion(
                ctx,
                options,
                args,
                instance,
                Some(hello_from_rust),
                std::ptr::null_mut(),
            );

            // does it get this far?
            assert_eq!(res, 0);

            // test if solutions are correct
            assert_eq!(X_VAL.load(Ordering::Relaxed), 1);
            assert_eq!(Y_VAL.load(Ordering::Relaxed), 2);
            assert_eq!(Z_VAL.load(Ordering::Relaxed), 1);

            minion_freeContext(ctx);
        }
    }

    #[test]
    fn midsearch_child_runner() {
        let Ok(scenario_raw) = std::env::var(MIDSEARCH_SCENARIO_ENV) else {
            return;
        };
        let scenario = MidsearchScenario::from_env_value(&scenario_raw)
            .expect("invalid value for MINION_MIDSEARCH_SCENARIO");

        let outcome = unsafe { run_midsearch_scenario_once(scenario) };

        assert_eq!(
            outcome.run_code, ReturnCodes_OK,
            "runMinion failed in child with outcome={outcome:#?}"
        );
        assert!(
            outcome.callback_count >= 1,
            "callback was never called; outcome={outcome:#?}"
        );
        assert!(
            outcome.added_var_ok,
            "newVar_ffi_safe failed in callback; outcome={outcome:#?}"
        );
        assert!(
            outcome.looked_up_new_var_ok,
            "getVarByName_safe for new variable failed; outcome={outcome:#?}"
        );

        if matches!(scenario, MidsearchScenario::AddVarThenAddEqConstraint) {
            assert_eq!(
                outcome.add_constraint_result,
                Some(true),
                "mid-search eq constraint using fresh variable failed; outcome={outcome:#?}"
            );
        }
    }

    #[test]
    #[ignore = "diagnostic stress test; runs child processes to detect crashes while mutating model mid-search"]
    fn midsearch_variable_addition_stress() {
        let scenarios = [
            MidsearchScenario::AddVarOnly,
            MidsearchScenario::AddVarThenAddEqConstraint,
        ];
        let iterations = 40usize;
        let mut any_failures = false;

        for scenario in scenarios {
            let mut stats = SpawnStats::default();

            for _ in 0..iterations {
                let status = run_midsearch_child(scenario);
                classify_status(&status, &mut stats);
            }

            eprintln!(
                "midsearch scenario={} iterations={} stats={stats:?}",
                scenario.as_env_value(),
                iterations
            );

            if stats.nonzero > 0 {
                any_failures = true;
            }
        }

        assert!(
            !any_failures,
            "at least one subprocess crashed/failed in mid-search variable-addition stress run"
        );
    }
}
