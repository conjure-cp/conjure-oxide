use crate::Model;
use crate::solver::{SolverError, SolverResult};

pub(super) fn model_to_cp_sat(model: Model) -> SolverResult<Model> {
    if model.constraints().is_empty() {
        return Err(SolverError::ModelInvalid(
            "cannot load empty model into ortools-cpsat adaptor".to_owned(),
        ));
    }
    Ok(model)
}
