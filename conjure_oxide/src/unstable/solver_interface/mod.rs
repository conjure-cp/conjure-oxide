use std::collections::HashMap;
use std::fmt::Debug;

use conjure_core::ast::Model;

struct Init;
struct HasModel;
struct HasRun;
struct ExecutionSuccess;
struct ExecutionFailure;

// TODO: seal trait.
trait SolverState {}
impl SolverState for Init {}
impl SolverState for HasModel {}
impl SolverState for ExecutionSuccess {}
impl SolverState for ExecutionFailure {}

type SolverError = String;

// TODO: this will use constant when it exists
type Callback = fn(bindings: HashMap<String,String>) -> bool;

// TODO: seal trait?
trait SolverAdaptor {
    type Model: Clone;
    type Solution; 
    type ExecutionError;
    type TranslationError: Debug;

    // TODO: this should be able to go to multiple states.
    // Adaptor implementers must call the user provided callback whenever a solution is found.
    fn run_solver(&mut self, model: Self::Model, callback: Callback) -> Result<ExecutionSuccess,ExecutionFailure>;
    fn load_model(&mut self, model: Model)  -> Result<Self::Model, Self::TranslationError>;
    fn init_solver(&mut self) {}
}


struct Solver<A:SolverAdaptor,State:SolverState = Init> {
    state: std::marker::PhantomData<State>,
    adaptor: A,
    model: Option<A::Model>,
}

impl<A: SolverAdaptor> Solver<A,Init> {
    pub fn load_model(mut self,model: Model) -> Result<Solver<A,HasModel>,SolverError> {
        let solver_model = &mut self.adaptor.load_model(model).unwrap();
        Ok(Solver {
            state: std::marker::PhantomData::<HasModel>,
            adaptor: self.adaptor,
            model: Some(solver_model.clone()),
        })
    }
}

impl <A:SolverAdaptor> Solver<A,HasModel> {

    pub fn solve(mut self, callback: Callback) -> Result<ExecutionSuccess,ExecutionFailure> {
        #[allow(clippy::unwrap_used)]
        self.adaptor.run_solver(self.model.unwrap(),callback)
    }
}

impl<T: SolverAdaptor> Solver<T> {
    pub fn new(solver_adaptor: T) -> Solver<T> {
        let mut solver = Solver {
            state: std::marker::PhantomData::<Init>,
            adaptor: solver_adaptor,
            model: None,
        };

        solver.adaptor.init_solver();
        solver
    }
}
