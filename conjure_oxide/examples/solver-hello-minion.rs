/// This example shows how to run a basic model top to bottom with Minion, with a focus on
/// demonstrating how the solver interface works.
///
/// The model is `conjure_oxide/tests/integration/basic/div/05/div-05.essence`
use conjure_core::{
    ast::{Literal, Name},
    solver::{adaptors::Minion, states::ExecutionSuccess},
};
use conjure_oxide::defaults::get_default_rule_sets;
use itertools::Itertools;
use std::collections::HashMap;

#[allow(clippy::unwrap_used)]
pub fn main() {
    use conjure_core::solver::SolverFamily;
    use conjure_core::{parse::get_example_model, rule_engine::resolve_rule_sets};
    use conjure_core::{
        rule_engine::rewrite_model,
        solver::{adaptors, Solver},
    };
    use std::sync::{Arc, Mutex};

    // Load an example model and rewrite it with conjure oxide.
    let model = get_example_model("div-05").unwrap();
    println!("Input model: \n {} \n", model.constraints);

    // TODO: We will have a nicer way to do this in the future
    let rule_sets = resolve_rule_sets(SolverFamily::Minion, &get_default_rule_sets()).unwrap();

    let model = rewrite_model(&model, &rule_sets).unwrap();
    println!("Rewritten model: \n {} \n", model.constraints);

    // To tell the `Solver` type what solver to use, you pass it a `SolverAdaptor`.
    // Here we use Minion.

    let solver = Solver::new(adaptors::Minion::new());

    // This API has a specific order:
    //
    // 1. Load a model
    // 2. Solve the model
    // 3. Read execution statistics
    //
    // If you skip a step, you get a compiler error!
    //
    // Solver has two type variables. One is the solver adaptor, the other is a state. This state
    // represents which step we are on. Certain methods are only available in certain states.

    // 1. Load a model
    // ===============
    //
    // Here, the solver takes in a subset of our model types and converts it into its own
    // representation. If it sees features it doesn't support, it will fail!.
    //
    // TRY: deleting this line! What compiler errors appear?
    // TRY: this takes the same `conjure_core::ast::Model` type as the rest of the program.
    //      what happens if we pass it a non re-written model?

    let solver = solver.load_model(model).unwrap();

    // 2. Solve
    // ========
    //
    //
    // To solve a model, we need to provide a callback function to be run whenever the solver has
    // found a solution. This takes a `HashMap<Name,Literal>`, representing a single solution, as
    // input.  The return value tells the solver whether to continue or not.
    //
    // We need this for the following:
    //
    //  1. To get solutions out of the solver
    //  2. To terminate the solver (e.g. if we only want 1 solution).
    //
    //
    // Concurrency
    // -----------
    //
    // The solver interface is designed to allow adaptors to use multiple-threads / processes if
    // necessary. Therefore, the callback type requires all variables inside it to have a static
    // lifetime and to implement Send (i.e. the variable can be safely shared between theads).

    // Here we will count solutions as well as returning the results.

    // We use Arc<Mutex<i32>> to create multiple pointers to a thread-safe mutable counter.
    let counter_ptr = Arc::new(Mutex::new(0));
    let counter_ptr_2 = counter_ptr.clone();

    // Doing the same for our list of solutions
    let all_solutions_ptr = Arc::new(Mutex::<Vec<HashMap<Name, Literal>>>::new(vec![]));
    let all_solutions_ptr_2 = all_solutions_ptr.clone();

    // Using the move |x| ... closure syntax, we give ownership of one of these pointers to the
    // solver. We still own the second pointer, which we use to get the counter out later!

    let result = solver.solve(Box::new(move |sols| {
        // add to counter
        let mut counter = (*counter_ptr_2).lock().unwrap();
        *counter += 1;

        // add to solutions
        let mut all_solutions = (*all_solutions_ptr_2).lock().unwrap();
        (*all_solutions).push(sols);
        true
    }));

    // Did the solver run successfully?
    let solver: Solver<Minion, ExecutionSuccess> = match result {
        Ok(s) => s,
        Err(e) => {
            panic!("Error! {:?}", e);
        }
    };

    // Read our counter.
    let counter = (*counter_ptr).lock().unwrap();
    println!("Num solutions: {}\n", counter);

    // Read solutions, print 3
    let all_sols = (*all_solutions_ptr).lock().unwrap();
    for (i, sols) in all_sols.iter().enumerate() {
        if i > 2 {
            println!("... and {} more", *counter - i);
            break;
        }
        println!("Solution {}:", i + 1);
        for (k, v) in sols.iter().sorted_by_key(|x| x.0) {
            println!("  {} = {}", k, v);
        }
        println!()
    }
    println!();

    // 3. Stats
    // Now that we have run the solver, we have access to the stats!
    // we can turn these into JSON for easy processing.
    //
    // TRY: what happens if we call solver.stats() when we haven't run the solver yet?
    let stats_json = serde_json::to_string_pretty(&solver.stats()).unwrap();
    println!("Solver stats: \n{}", stats_json);
}
