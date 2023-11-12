use std::{
    collections::HashMap,
    ffi::CString,
    sync::{Mutex, MutexGuard},
};

use crate::{ast::*, error::*, raw_bindings::*, scoped_ptr::Scoped};

// TODO: allow passing of options.

/// Callback function used to capture results from minion as they are generated.
/// Should return `true` if search is to continue, `false` otherwise.
///
/// Consider using a global mutex (or other static variable) to use these results
/// elsewhere.
///
/// For example:
///
/// ```
///   use minion_rs::ast::*;
///   use minion_rs::run_minion;
///   use std::{
///       collections::HashMap,
///       sync::{Mutex, MutexGuard},
///   };
///
///   // More elaborate data-structures are possible, but for sake of example store
///   // a vector of solution sets.
///   static ALL_SOLUTIONS: Mutex<Vec<HashMap<VarName,Constant>>>  = Mutex::new(vec![]);
///   
///   fn callback(solutions: HashMap<VarName,Constant>) -> bool {
///       let mut guard = ALL_SOLUTIONS.lock().unwrap();
///       guard.push(solutions);
///       return true;
///   }
///    
///   // Build and run the model.
///   let mut model = Model::new();
///
///   // ... omitted for brevity ...
/// #
/// # model
/// #     .named_variables
/// #     .add_var("x".to_owned(), VarDomain::Bound(1, 3));
/// # model
/// #     .named_variables
/// #     .add_var("y".to_owned(), VarDomain::Bound(2, 4));
/// # model
/// #     .named_variables
/// #     .add_var("z".to_owned(), VarDomain::Bound(1, 5));
/// #
/// # let leq = Constraint::SumLeq(
/// #     vec![
/// #         Var::NameRef("x".to_owned()),
/// #         Var::NameRef("y".to_owned()),
/// #         Var::NameRef("z".to_owned()),
/// #     ],
/// #     Var::ConstantAsVar(4),
/// # );
/// #
/// # let geq = Constraint::SumGeq(
/// #     vec![
/// #         Var::NameRef("x".to_owned()),
/// #         Var::NameRef("y".to_owned()),
/// #         Var::NameRef("z".to_owned()),
/// #     ],
/// #     Var::ConstantAsVar(4),
/// # );
/// #
/// # let ineq = Constraint::Ineq(
/// #     Var::NameRef("x".to_owned()),
/// #     Var::NameRef("y".to_owned()),
/// #     Constant::Integer(-1),
/// # );
/// #
/// # model.constraints.push(leq);
/// # model.constraints.push(geq);
/// # model.constraints.push(ineq);
///  
///   let res = run_minion(model, callback);
///   res.expect("Error occurred");
///
///   // Get solutions
///   let guard = ALL_SOLUTIONS.lock().unwrap();
///
///   let solution_set_1 = &(*guard.get(0).unwrap());
///
///   let x1 = solution_set_1.get("x").unwrap();
///   let y1 = solution_set_1.get("y").unwrap();
///   let z1 = solution_set_1.get("z").unwrap();
/// #
/// # // FIXME: this test would be better with an example with >1 solution.
/// # assert_eq!(guard.len(),1);
/// # assert_eq!(*x1,Constant::Integer(1));
/// # assert_eq!(*y1,Constant::Integer(2));
/// # assert_eq!(*z1,Constant::Integer(1));
/// ```
pub type Callback = fn(solution_set: HashMap<VarName, Constant>) -> bool;

// Use globals to pass things between run_minion and the callback function.
// Minion is (currently) single threaded anyways so the Mutexs' don't matter.

// the current callback function
static CALLBACK: Mutex<Option<Callback>> = Mutex::new(None);

// the variables we want to return, and their ordering in the print matrix
static PRINT_VARS: Mutex<Option<Vec<VarName>>> = Mutex::new(None);

#[no_mangle]
unsafe extern "C" fn run_callback() -> bool {
    // get printvars from static PRINT_VARS if they exist.
    // if not, return true and continue search.

    let mut guard: MutexGuard<'_, Option<Vec<VarName>>> = PRINT_VARS.lock().unwrap();

    println!("here 0");
    if guard.is_none() {
        return true;
    }

    let print_vars = match &mut *guard {
        Some(x) => x,
        None => unreachable!(),
    };

    println!("here 1 printvars: {:?}", print_vars);
    if print_vars.len() == 0 {
        return true;
    }

    // build nice solutions view to be used by callback
    let mut solutions: HashMap<VarName, Constant> = HashMap::new();

    for (i, var) in print_vars.iter().enumerate() {
        let solution_int: i32 = printMatrix_getValue(i as _).try_into().unwrap();
        let solution: Constant = Constant::Integer(solution_int);
        solutions.insert(var.to_string(), solution);
    }

    println!("here {:?}", solutions);
    match *CALLBACK.lock().unwrap() {
        None => true,
        Some(func) => func(solutions),
    }
}

/// Run Minion on the given [Model].
///
/// The given [callback](Callback) is ran whenever a new solution set is found.
pub fn run_minion(model: Model, callback: Callback) -> Result<(), RuntimeError> {
    *CALLBACK.lock().unwrap() = Some(callback);

    unsafe {
        let options = Scoped::new(newSearchOptions(), |x| searchOptions_free(x as _));
        let args = Scoped::new(newSearchMethod(), |x| searchMethod_free(x as _));
        let instance = Scoped::new(convert_model_to_raw(&model), |x| instance_free(x as _));
        let res = runMinion(options.ptr, args.ptr, instance.ptr, Some(run_callback));

        match res {
            0 => Ok(()),
            x => Err(RuntimeError::from(x)),
        }
    }
}

/// Callee owns the returned instance
unsafe fn convert_model_to_raw(model: &Model) -> *mut ProbSpec_CSPInstance {
    // This is managed in scope by the callee
    let instance = newInstance();

    /*******************************/
    /*        Add variables        */
    /*******************************/

    /*
     * Add variables to:
     * 1. symbol table
     * 2. print matrix
     * 3. search vars
     *
     * These are all done in the order saved in the SymbolTable.
     */

    let search_vars = Scoped::new(vec_var_new(), |x| vec_var_free(x as _));

    // store variables and the order they will be returned inside rust for later use.
    let mut print_vars_guard = PRINT_VARS.lock().unwrap();
    *print_vars_guard = Some(vec![]);

    for var_name in model.named_variables.get_variable_order() {
        //TODO: make this return Result
        let c_str = CString::new(var_name.clone()).expect("");

        let vartype = model
            .named_variables
            .get_vartype(var_name.clone())
            .expect("");

        let (vartype_raw, domain_low, domain_high) = match vartype {
            VarDomain::Bound(a, b) => (VariableType_VAR_BOUND, a, b),
            _ => panic!("NOT IMPLEMENTED"),
        };

        newVar_ffi(
            instance,
            c_str.as_ptr() as _,
            vartype_raw,
            domain_low,
            domain_high,
        );

        let var = getVarByName(instance, c_str.as_ptr() as _);

        printMatrix_addVar(instance, var);

        // add to the print vars stored in rust so to remember
        // the order for callback function.
        (*print_vars_guard).as_mut().unwrap().push(var_name.clone());

        vec_var_push_back(search_vars.ptr, var);
    }

    let search_order = Scoped::new(
        newSearchOrder(search_vars.ptr, VarOrderEnum_ORDER_STATIC, false),
        |x| searchOrder_free(x as _),
    );

    // this and other instance_ functions does not move so my use of ptrs are ok
    // TODO (nd60): document this
    instance_addSearchOrder(instance, search_order.ptr);

    /*********************************/
    /*        Add constraints        */
    /*********************************/

    for constraint in &model.constraints {
        // 1. get constraint type and create C++ constraint object
        // 2. run through arguments and add them to the constraint
        // 3. add constraint to instance

        let constraint_type = get_constraint_type(constraint);
        let raw_constraint = Scoped::new(newConstraintBlob(constraint_type), |x| {
            constraint_free(x as _)
        });

        constraint_add_args(instance, raw_constraint.ptr, constraint);
        instance_addConstraint(instance, raw_constraint.ptr);
    }

    return instance;
}

unsafe fn get_constraint_type(constraint: &Constraint) -> u32 {
    match constraint {
        Constraint::SumGeq(_, _) => ConstraintType_CT_GEQSUM,
        Constraint::SumLeq(_, _) => ConstraintType_CT_LEQSUM,
        Constraint::Ineq(_, _, _) => ConstraintType_CT_INEQ,
        #[allow(unreachable_patterns)]
        _ => panic!("NOT IMPLEMENTED"),
    }
}

unsafe fn constraint_add_args(
    i: *mut ProbSpec_CSPInstance,
    r_constr: *mut ProbSpec_ConstraintBlob,
    constr: &Constraint,
) {
    match constr {
        Constraint::SumGeq(lhs_vars, rhs_var) => {
            read_vars(i, r_constr, &lhs_vars);
            read_var(i, r_constr, rhs_var)
        }
        Constraint::SumLeq(lhs_vars, rhs_var) => {
            read_vars(i, r_constr, &lhs_vars);
            read_var(i, r_constr, rhs_var)
        }
        Constraint::Ineq(var1, var2, c) => {
            read_var(i, r_constr, &var1);
            read_var(i, r_constr, &var2);
            read_const(r_constr, c)
        }
        #[allow(unreachable_patterns)]
        _ => panic!("NOT IMPLEMENTED"),
    };
}

// DO NOT call manually - this assumes that all needed vars are already in the symbol table.
// TODO not happy with this just assuming the name is in the symbol table
unsafe fn read_vars(
    instance: *mut ProbSpec_CSPInstance,
    raw_constraint: *mut ProbSpec_ConstraintBlob,
    vars: &Vec<Var>,
) {
    let raw_vars = Scoped::new(vec_var_new(), |x| vec_var_free(x as _));
    for var in vars {
        // TODO: could easily break and segfault and die and so on
        let raw_var = match var {
            Var::NameRef(name) => {
                let c_str = CString::new(name.clone()).expect("");
                getVarByName(instance, c_str.as_ptr() as _)
            }
            Var::ConstantAsVar(n) => constantAsVar(*n),
        };

        vec_var_push_back(raw_vars.ptr, raw_var);
    }

    constraint_addVarList(raw_constraint, raw_vars.ptr);
}

unsafe fn read_var(
    instance: *mut ProbSpec_CSPInstance,
    raw_constraint: *mut ProbSpec_ConstraintBlob,
    var: &Var,
) {
    let raw_vars = Scoped::new(vec_var_new(), |x| vec_var_free(x as _));
    let raw_var = match var {
        Var::NameRef(name) => {
            let c_str = CString::new(name.clone()).expect("");
            getVarByName(instance, c_str.as_ptr() as _)
        }
        Var::ConstantAsVar(n) => constantAsVar(*n),
    };

    vec_var_push_back(raw_vars.ptr, raw_var);
    constraint_addVarList(raw_constraint, raw_vars.ptr);
}

unsafe fn read_const(raw_constraint: *mut ProbSpec_ConstraintBlob, constant: &Constant) {
    let raw_consts = Scoped::new(vec_int_new(), |x| vec_var_free(x as _));

    let val = match constant {
        Constant::Integer(n) => n,
        _ => panic!("NOT IMPLEMENTED"),
    };

    vec_int_push_back(raw_consts.ptr, *val);
    constraint_addConstantList(raw_constraint, raw_consts.ptr);
}
