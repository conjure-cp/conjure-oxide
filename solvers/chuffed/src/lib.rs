pub mod bindings {
    #![allow(warnings)]
    include!(concat!(env!("OUT_DIR"), "/chuffed_bindings.rs"));
}

pub mod wrappers {
    use crate::bindings::{
        all_different, branch_IntVar, createVar, createVars, int_plus, make_vec_intvar,
        output_vars1, var_sym_break, vec, ConLevel, IntVar, ValBranch, VarBranch,
    };
    use core::ptr;

    // The signature of createVar is below for reference.
    // createVar(x: *mut *mut IntVar, min: ::std::os::raw::c_int, max: ::std::os::raw::c_int, el: bool)
    pub fn create_var(min: i32, max: i32, el: bool) -> *mut IntVar {
        let mut ptr: *mut IntVar = ptr::null_mut();

        unsafe {
            createVar(&mut ptr, min, max, el);
            ptr
        }
    }

    // createVars void createVars(vec<IntVar*>& x, int n, int min, int max, bool el)
    pub fn create_vars(n: i32, min: i32, max: i32, el: bool) -> *mut vec<*mut IntVar> {
        let ptr: *mut vec<*mut IntVar> = unsafe { make_vec_intvar() };

        unsafe {
            createVars(ptr, n, min, max, el);
            ptr
        }
    }

    // void all_different(vec<IntVar*>& x, ConLevel cl)
    pub unsafe fn all_different_wrapper(x: *mut vec<*mut IntVar>, cl: ConLevel) {
        unsafe {
            all_different(x, cl);
        }
    }

    // void branch(vec<Branching*> x, VarBranch var_branch, ValBranch val_branch);
    pub unsafe fn branch_wrapper(
        x: *mut vec<*mut IntVar>,
        var_branch: VarBranch,
        val_branch: ValBranch,
    ) {
        unsafe {
            branch_IntVar(x, var_branch, val_branch);
        }
    }

    pub unsafe fn output_vars_wrapper(x: *mut vec<*mut IntVar>) {
        unsafe {
            // output_vars1 takes in an vec<IntVar*> instead of branching
            output_vars1(x);
        }
    }

    pub unsafe fn var_sym_break_wrapper(x: *mut vec<*mut IntVar>) {
        unsafe {
            var_sym_break(x);
        }
    }

    pub unsafe fn int_plus_wrapper(x: *mut IntVar, y: *mut IntVar, z: *mut IntVar) {
        unsafe {
            int_plus(x, y, z);
        }
    }
}
