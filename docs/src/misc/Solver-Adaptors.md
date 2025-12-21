<!-- TODO: This doesnt seem complete -->
# Why Use The `SolverAdaptor` Trait?

One of the steps in generating solutions is to pass a constraint problem to a constraint solver. However, the process for passing problems to, and getting solutions from a constraint solver is different for each solver. The simplest way to do this is to write a very large amount of code to 'call' the solvers and get solutions, tailored to each of the solver libraries. This would be long, tedious, and quite redundant.

The better way to do this would be to implement some type of shared behaviour. Doing so would allow the implementation of code in a solver-agnostic manner as long as it can rely on whatever trait is used to implement the aforementioned shared behaviour. Furthermore, this means that the code to pass problems and get solutions is encapsulated away from the rest of the program, so that it can implement whatever is needed for the solvers. Rust is a particularly good language for this, as it allows the implementation of Traits on structs that have already been defined, as opposed to the interface system in object oriented languages like Java, and C#. 

In order to do this programatically, we use crates which contain the infrastructure for using different constraint solvers to solve problems. How these crates use the constraint solvers (generally written in other languages) does not matter as long as the `SolverAdaptor` trait is implemented for some struct which can be initialized and then used to call the solvers. 

# Development using this infrastructure

## Adding a Solver Backend using the `SolverAdaptor` Trait

## Using `SolverAdaptor` in Conjure Oxide

# Currently used SAT Adaptor Backend

## Background

### Background Information on Implementation

RustSAT contains bindings for a number of common SAT solvers such as *kissat* written by Armin Biere and *Minisat*, which is the one being used by this instance of the solver. While *Minisat* is the solver being used, the variation of SAT Solver is irrelevant. This is because the functionality for calling the solver (when it is eventually called) is implemented in shared behaviour using a Rust `Trait`. For example, the `rustsat::solvers::Solve` trait is used to call the `solve()` on any solver supported by `solver`. 

```rust
use rustsat_minisat::core::Minisat;
use rustsat::solvers::{Solve, SolverResult};

    [...]

let res = solver.solve().unwrap();
```

The preceding code snippet is from this implementation. It uses *Minisat*, but it can use any of the other solvers by simply using the relevant identifier and `use` item to import the desired solver.

RustSAT also uses the `SatInstance` type as an abstract syntax tree and a variable manager in one. This is a rather complex way of saying that all syntax trees, variables, literals etc. must be associated with an instance of `SatInstance` in order to be used by the solver.

### Conjure Oxide

Conjure Oxide stores all variables in the `SymbolTable` struct — including decision variables (`finds`) and other variables (`lettings`).

The constraints themselves are stored in an instance of the recursively defined `Expression` enum, which also represents the constraints themselves. The top-level expression stores other instances of `Expression`, each of which stores either other constraints or atoms — which are indivisible constraints. This allows us to recursively store any arbitrarily large constraint tree.

### Getting Decision Variables and Constraints

Preparing the Conjure Oxide model for use by Rust requires the use of several things.

In order to get the decision variables, this implementation of the `SolverAdaptor` trait clones the symbol table associated with the current runtime of Conjure Oxide and turns the clone into an iterator, which it uses to iterate over all variables. While iterating, it checks that the domain of each of them is boolean. *If* a non-boolean symbol is found, the function will return an error. If not, it will add all of them to a vector.

Each of the variables in the symbol table has a name associated with them and, *ideally*, a reference in the constraints. At this point, however, there is no way to predict whether or not these variables appear in the constraints, and so it is important to consider both possibilities.

For this reason, a `HashMap` is initialized with the names as keys and literals as values. Now, the program has to intelligently "copy" the expression tree into a `SatInstance`. In order to do this, the model associated with the current runtime is cloned, then a vector of all constraints in the top-level expression is extracted from the model clone using the `.as_submodel().constraints()` function chain and cloning. This process consumes the clone of the model.

The next step in the process is to actually do the conversion, which is described in the next section.


## Converting to SatInstance

### Copying the Model

The next step in the process is to intelligently copy the expression tree into a `SatInstance`. This is done by passing a vector of constraints *and* a hashmap of decision variables into the `handle_cnf` function, located in `solver/adaptors/rustsat/convs.rs` in the `conjure_core` crate. The `handle_cnf` function, along with others in this file, recursively traverses the entire expression tree while updating a shared `SatInstance`.

At this point, the following are in scope: a vector of all clauses, a map from variable names to literals, and a `SatInstance`, which is initialized within `handle_cnf`. These must remain in scope throughout the model conversion process. This is particularly important due to Rust’s strict ownership and memory safety guarantees — any data that goes out of scope is immediately destroyed. For instance, if an expression is boxed (as is the case with some variants of the `Expression` enum), the heap-allocated memory will be freed when the box is dropped.

The next step is to iterate over each item in the constraint vector. The handling logic must remain flexible, as not all constraints are disjunctions. Each constraint is passed to `handle_disjn`, which handles this logic accordingly.

The `handle_disjn` function verifies that the given constraint is a disjunction. Currently, it panics for all other cases (there is a hole in the match), using a standard `panic!` rather than Conjure Oxide’s `bug!` macro. This may be revised in the future to support other expression types, such as constants. When a disjunction is found, it is converted into a vector of constraints—expected to be atoms. There is no longer a need to check that all constraints are disjunctions at this point. However, we do need to ensure that all constraints at this level are atomic. To do this, every constraint in the resulting "disjunction vector" is passed to the `handle_lit` function.

### Processing Literals

The `handle_lit` function matches on the constraint passed to it. For anything that is not an `Expression::Not` or `Expression::Atomic`, the function currently panics. As with the previous case, this is a `panic!`, not a call to `bug!`, since the behavior is still under development. Future versions may expand the match arms or change the fallback case to call `bug!`.

- **Case: `Expression::Atomic`**  
  In this case, the function calls `handle_atom` with a polarity of `true`. `handle_atom` looks up a literal from the variable map using the name stored in the `Atom`, and then adds the corresponding RustSAT literal to the `SatInstance`. This completes the "copying" process.

- **Case: `Expression::Not`**  
  This case delegates to `handle_not`, which unpacks the boxed `Atom` inside the `Not` expression. It then calls `handle_atom` with a negative polarity.
