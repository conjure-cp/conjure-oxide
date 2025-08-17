#![allow(warnings)]

use std::ffi::CString;
use std::sync::atomic::{AtomicI32, Ordering};
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(test)]
mod tests {
    use std::ffi::CString;

    use super::*;

    // solutions
    static X_VAL: AtomicI32 = AtomicI32::new(0);
    static Y_VAL: AtomicI32 = AtomicI32::new(0);
    static Z_VAL: AtomicI32 = AtomicI32::new(0);

    #[unsafe(no_mangle)]
    pub extern "C" fn hello_from_rust() -> bool {
        unsafe {
            X_VAL.store(printMatrix_getValue(0) as _, Ordering::Relaxed);
            Y_VAL.store(printMatrix_getValue(1) as _, Ordering::Relaxed);
            Z_VAL.store(printMatrix_getValue(2) as _, Ordering::Relaxed);
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

            let res = runMinion(options, args, instance, Some(hello_from_rust));

            // does it get this far?
            assert_eq!(res, 0);

            // test if solutions are correct
            assert_eq!(X_VAL.load(Ordering::Relaxed), 1);
            assert_eq!(Y_VAL.load(Ordering::Relaxed), 2);
            assert_eq!(Z_VAL.load(Ordering::Relaxed), 1);
        }
    }
}
