use conjure_oxide::conjure_core::solver::{SolverAdaptor, SolveError};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use conjure_oxide::conjure_core::essence::model::EssenceModel;

//custom solver adaptor for solving using savile row
pub struct SRSolverAdaptor;

impl SolverAdaptor for SRSolverAdaptor {
  
     fn load_essence_model(&self, essence_file: &Path) -> Result<EssenceModel, SolveError> {
        Err(SolveError::from("TODO"))
    }

    // Transform the Essence model into Essence Prime format
    fn to_essence_prime(&self, model: EssenceModel) -> Result<EssenceModel, SolveError> {
        Err(SolveError::from("TODO"))
    }

    // Solve the Essence Prime model using the specified solver
    fn solve(&self, model: &EssenceModel, output_dir: &Path, solver_name: &str) -> Result<Vec<HashMap<String, String>>, SolveError> {
        Err(SolveError::from("TODO"))
    }
}