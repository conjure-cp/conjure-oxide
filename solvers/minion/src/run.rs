use std::{
    collections::HashMap,
    ffi::CString,
    sync::{Mutex, MutexGuard},
};

use crate::ffi;
use crate::{ast::*, error::*, scoped_ptr::Scoped};
use anyhow::anyhow;

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
///       true
///   }
///    
///   // Build and run the model.
///   let mut model = Model::new();
///
///   // ... omitted for brevity ...
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
///   let solution_set_1 = &(guard.get(0).unwrap());
///
///   let x1 = solution_set_1.get("x").unwrap();
///   let y1 = solution_set_1.get("y").unwrap();
///   let z1 = solution_set_1.get("z").unwrap();
/// #
/// # // TODO: this test would be better with an example with >1 solution.
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

    // Mutex poisoning is probably panic worthy.
    #[allow(clippy::unwrap_used)]
    let mut guard: MutexGuard<'_, Option<Vec<VarName>>> = PRINT_VARS.lock().unwrap();

    if guard.is_none() {
        return true;
    }

    let print_vars = match &mut *guard {
        Some(x) => x,
        None => unreachable!(),
    };

    if print_vars.is_empty() {
        return true;
    }

    // build nice solutions view to be used by callback
    let mut solutions: HashMap<VarName, Constant> = HashMap::new();

    for (i, var) in print_vars.iter().enumerate() {
        let solution_int: i32 = ffi::printMatrix_getValue(i as _);
        let solution: Constant = Constant::Integer(solution_int);
        solutions.insert(var.to_string(), solution);
    }

    #[allow(clippy::unwrap_used)]
    match *CALLBACK.lock().unwrap() {
        None => true,
        Some(func) => func(solutions),
    }
}

/// Run Minion on the given [Model].
///
/// The given [callback](Callback) is ran whenever a new solution set is found.

// Turn it into a warning for this function, cant unwarn it directly above callback wierdness
#[allow(clippy::unwrap_used)]
pub fn run_minion(model: Model, callback: Callback) -> Result<(), MinionError> {
    // Mutex poisoning is probably panic worthy.
    *CALLBACK.lock().unwrap() = Some(callback);

    unsafe {
        let options = Scoped::new(ffi::newSearchOptions(), |x| ffi::searchOptions_free(x as _));
        let args = Scoped::new(ffi::newSearchMethod(), |x| ffi::searchMethod_free(x as _));
        let instance = Scoped::new(ffi::newInstance(), |x| ffi::instance_free(x as _));

        convert_model_to_raw(&instance, &model)?;

        let res = ffi::runMinion(options.ptr, args.ptr, instance.ptr, Some(run_callback));
        match res {
            0 => Ok(()),
            x => Err(MinionError::from(RuntimeError::from(x))),
        }
    }
}

unsafe fn convert_model_to_raw(
    instance: &Scoped<ffi::ProbSpec_CSPInstance>,
    model: &Model,
) -> Result<(), MinionError> {
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

    let search_vars = Scoped::new(ffi::vec_var_new(), |x| ffi::vec_var_free(x as _));

    // store variables and the order they will be returned inside rust for later use.
    #[allow(clippy::unwrap_used)]
    let mut print_vars_guard = PRINT_VARS.lock().unwrap();
    *print_vars_guard = Some(vec![]);

    for var_name in model.named_variables.get_variable_order() {
        let c_str = CString::new(var_name.clone()).map_err(|_| {
            anyhow!(
                "Variable name {:?} contains a null character.",
                var_name.clone()
            )
        })?;

        let vartype = model
            .named_variables
            .get_vartype(var_name.clone())
            .ok_or(anyhow!("Could not get var type for {:?}", var_name.clone()))?;

        let (vartype_raw, domain_low, domain_high) = match vartype {
            VarDomain::Bound(a, b) => Ok((ffi::VariableType_VAR_BOUND, a, b)),
            x => Err(MinionError::NotImplemented(format!("{:?}", x))),
        }?;

        ffi::newVar_ffi(
            instance.ptr,
            c_str.as_ptr() as _,
            vartype_raw,
            domain_low,
            domain_high,
        );

        let var = ffi::getVarByName(instance.ptr, c_str.as_ptr() as _);

        ffi::printMatrix_addVar(instance.ptr, var);

        // add to the print vars stored in rust so to remember
        // the order for callback function.

        #[allow(clippy::unwrap_used)]
        (*print_vars_guard).as_mut().unwrap().push(var_name.clone());

        ffi::vec_var_push_back(search_vars.ptr, var);
    }

    let search_order = Scoped::new(
        ffi::newSearchOrder(search_vars.ptr, ffi::VarOrderEnum_ORDER_STATIC, false),
        |x| ffi::searchOrder_free(x as _),
    );

    ffi::instance_addSearchOrder(instance.ptr, search_order.ptr);

    /*********************************/
    /*        Add constraints        */
    /*********************************/

    for constraint in &model.constraints {
        // 1. get constraint type and create C++ constraint object
        // 2. run through arguments and add them to the constraint
        // 3. add constraint to instance

        let constraint_type = get_constraint_type(constraint)?;
        let raw_constraint = Scoped::new(ffi::newConstraintBlob(constraint_type), |x| {
            ffi::constraint_free(x as _)
        });

        constraint_add_args(instance.ptr, raw_constraint.ptr, constraint)?;
        ffi::instance_addConstraint(instance.ptr, raw_constraint.ptr);
    }

    Ok(())
}

unsafe fn get_constraint_type(constraint: &Constraint) -> Result<u32, MinionError> {
    match constraint {
        Constraint::SumGeq(_, _) => Ok(ffi::ConstraintType_CT_GEQSUM),
        Constraint::SumLeq(_, _) => Ok(ffi::ConstraintType_CT_LEQSUM),
        Constraint::Ineq(_, _, _) => Ok(ffi::ConstraintType_CT_INEQ),
        Constraint::Eq(_, _) => Ok(ffi::ConstraintType_CT_EQ),
        #[allow(unreachable_patterns)]
        x => Err(MinionError::NotImplemented(format!(
            "Constraint not implemented {:?}",
            x,
        ))),
    }
}

unsafe fn constraint_add_args(
    i: *mut ffi::ProbSpec_CSPInstance,
    r_constr: *mut ffi::ProbSpec_ConstraintBlob,
    constr: &Constraint,
) -> Result<(), MinionError> {
    match constr {
        Constraint::SumGeq(lhs_vars, rhs_var) => {
            read_vars(i, r_constr, lhs_vars)?;
            read_var(i, r_constr, rhs_var)?;
            Ok(())
        }
        Constraint::SumLeq(lhs_vars, rhs_var) => {
            read_vars(i, r_constr, lhs_vars)?;
            read_var(i, r_constr, rhs_var)?;
            Ok(())
        }
        Constraint::Ineq(var1, var2, c) => {
            read_var(i, r_constr, var1)?;
            read_var(i, r_constr, var2)?;
            read_const(r_constr, c)?;
            Ok(())
        }
        Constraint::Eq(var1, var2) => {
            read_var(i, r_constr, var1)?;
            read_var(i, r_constr, var2)?;
            Ok(())
        }
        #[allow(unreachable_patterns)]
        x => Err(MinionError::NotImplemented(format!("{:?}", x))),
    }
}

// DO NOT call manually - this assumes that all needed vars are already in the symbol table.
// TODO not happy with this just assuming the name is in the symbol table
unsafe fn read_vars(
    instance: *mut ffi::ProbSpec_CSPInstance,
    raw_constraint: *mut ffi::ProbSpec_ConstraintBlob,
    vars: &Vec<Var>,
) -> Result<(), MinionError> {
    let raw_vars = Scoped::new(ffi::vec_var_new(), |x| ffi::vec_var_free(x as _));
    for var in vars {
        let raw_var = match var {
            Var::NameRef(name) => {
                let c_str = CString::new(name.clone()).map_err(|_| {
                    anyhow!(
                        "Variable name {:?} contains a null character.",
                        name.clone()
                    )
                })?;
                ffi::getVarByName(instance, c_str.as_ptr() as _)
            }
            Var::ConstantAsVar(n) => ffi::constantAsVar(*n),
        };

        ffi::vec_var_push_back(raw_vars.ptr, raw_var);
    }

    ffi::constraint_addVarList(raw_constraint, raw_vars.ptr);

    Ok(())
}

unsafe fn read_var(
    instance: *mut ffi::ProbSpec_CSPInstance,
    raw_constraint: *mut ffi::ProbSpec_ConstraintBlob,
    var: &Var,
) -> Result<(), MinionError> {
    let raw_vars = Scoped::new(ffi::vec_var_new(), |x| ffi::vec_var_free(x as _));
    let raw_var = match var {
        Var::NameRef(name) => {
            let c_str = CString::new(name.clone()).map_err(|_| {
                anyhow!(
                    "Variable name {:?} contains a null character.",
                    name.clone()
                )
            })?;
            ffi::getVarByName(instance, c_str.as_ptr() as _)
        }
        Var::ConstantAsVar(n) => ffi::constantAsVar(*n),
    };

    ffi::vec_var_push_back(raw_vars.ptr, raw_var);
    ffi::constraint_addVarList(raw_constraint, raw_vars.ptr);

    Ok(())
}

unsafe fn read_const(
    raw_constraint: *mut ffi::ProbSpec_ConstraintBlob,
    constant: &Constant,
) -> Result<(), MinionError> {
    let raw_consts = Scoped::new(ffi::vec_int_new(), |x| ffi::vec_var_free(x as _));

    let val = match constant {
        Constant::Integer(n) => Ok(n),
        x => Err(MinionError::NotImplemented(format!("{:?}", x))),
    }?;

    ffi::vec_int_push_back(raw_consts.ptr, *val);
    ffi::constraint_addConstantList(raw_constraint, raw_consts.ptr);

    Ok(())
}
