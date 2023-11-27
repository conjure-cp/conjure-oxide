use thiserror::Error;

use super::Solver;

#[derive(Error, Debug)]
pub enum SolverError {
    #[error("not supported in solver `{0}`: `{1}`.")]
    NotSupported(Solver, String),

    #[error("invalid instance for solver `{0}`:`{1}`")]
    InvalidInstance(Solver, String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
