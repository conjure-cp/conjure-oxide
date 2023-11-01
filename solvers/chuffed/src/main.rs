use chuffed_rs::wrappers::{create_vars, all_different_wrapper, branch_wrapper};
use chuffed_rs::bindings::{IntVar, ConLevel_CL_DEF, VarBranch_VAR_INORDER, VarBranch_VAR_MIN_MIN, vec, new_dummy_problem, get_idx, p_addVars, p_setcallback}; 

unsafe fn post_constraints(_n: i32) -> *mut vec<*mut IntVar> {
    // Create constant
    let n: i32 = _n;
    // Create some variables
    let x: *mut vec<*mut IntVar> = create_vars(n, 0, n, false);

    // Post some constraints
    all_different_wrapper(x, ConLevel_CL_DEF);

    // Post some branchings
    branch_wrapper(x as _, VarBranch_VAR_INORDER, VarBranch_VAR_MIN_MIN); 

    x
}

// Custom printing function for this problem
#[no_mangle]
pub unsafe extern "C" fn callback(x: *mut vec<*mut IntVar>) {
   print!("First output is: {:?}", get_idx(x, 0));
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() != 2 {
        println!("Invalid number of arguments");
        return;
    }

    let n: i32 = args[1].parse().expect("Invalid input");
    
    unsafe{
        let x = post_constraints(n);
        // make new dummy problem
        let p = new_dummy_problem();
        // Call problem.addvars()
        p_addVars(p, x);
        // Call problem.setcallback()
        p_setcallback(p, Some(callback));
    }
}
