use std::fmt;

use rustsat::instances::Cnf;
#[cfg(all(feature = "sat-cadical", not(target_family = "wasm")))]
use rustsat::solvers::{ControlSignal, Terminate};
use rustsat::solvers::{Solve, SolveIncremental, SolverResult};
use rustsat::types::{Assignment, Clause, Lit, Var};

use crate::solver::SolverError;
use crate::solver::adaptors::SolveTimeBudget;

/// SAT backends compiled into this build.
/// The default is CaDiCaL when available, otherwise BatSat.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum SatBackend {
    #[cfg(all(feature = "sat-cadical", not(target_family = "wasm")))]
    #[default]
    Cadical,
    #[cfg(feature = "sat-batsat")]
    #[cfg_attr(
        not(all(feature = "sat-cadical", not(target_family = "wasm"))),
        default
    )]
    Batsat,
}

impl fmt::Display for SatBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(all(feature = "sat-cadical", not(target_family = "wasm")))]
            Self::Cadical => f.write_str("cadical"),
            #[cfg(feature = "sat-batsat")]
            Self::Batsat => f.write_str("batsat"),
        }
    }
}

pub(super) enum SatSolver {
    #[cfg(all(feature = "sat-cadical", not(target_family = "wasm")))]
    Cadical(rustsat_cadical::CaDiCaL<'static, 'static>),
    #[cfg(feature = "sat-batsat")]
    Batsat(Box<rustsat_batsat::BasicSolver>),
}

// Keep CNF encoding and solution enumeration shared, dispatching only solver operations.
macro_rules! dispatch {
    ($self:expr, $solver:ident => $body:expr) => {
        match $self {
            #[cfg(all(feature = "sat-cadical", not(target_family = "wasm")))]
            SatSolver::Cadical($solver) => $body,
            #[cfg(feature = "sat-batsat")]
            SatSolver::Batsat($solver) => $body,
        }
    };
}

impl SatSolver {
    pub(super) fn new(backend: SatBackend) -> Self {
        match backend {
            #[cfg(all(feature = "sat-cadical", not(target_family = "wasm")))]
            SatBackend::Cadical => Self::Cadical(Default::default()),
            #[cfg(feature = "sat-batsat")]
            SatBackend::Batsat => Self::Batsat(Default::default()),
        }
    }

    pub(super) fn set_seed(&mut self, seed: u32) -> Result<(), SolverError> {
        match self {
            #[cfg(all(feature = "sat-cadical", not(target_family = "wasm")))]
            Self::Cadical(solver) => {
                let seed = i32::try_from(seed).map_err(|_| {
                    SolverError::Runtime(format!(
                        "solver seed {seed} exceeds CaDiCaL's maximum supported value ({})",
                        i32::MAX
                    ))
                })?;
                solver.set_option("seed", seed).map_err(|err| {
                    SolverError::Runtime(format!("Failed setting CaDiCaL solver seed: {err}"))
                })
            }
            #[cfg(feature = "sat-batsat")]
            Self::Batsat(_) if seed != 0 => Err(SolverError::Runtime(
                "BatSat does not support configuring the solver seed through this adaptor".into(),
            )),
            #[cfg(feature = "sat-batsat")]
            Self::Batsat(_) => Ok(()),
        }
    }

    pub(super) fn set_budget(&mut self, budget: SolveTimeBudget) {
        match self {
            #[cfg(all(feature = "sat-cadical", not(target_family = "wasm")))]
            Self::Cadical(solver) => solver.attach_terminator(move || {
                if budget.expired() {
                    ControlSignal::Terminate
                } else {
                    ControlSignal::Continue
                }
            }),
            #[cfg(feature = "sat-batsat")]
            Self::Batsat(solver) => solver
                .batsat_mut()
                .cb_mut()
                .set_stop(move || budget.expired()),
        }
    }

    pub(super) fn reserve(&mut self, var: Var) -> anyhow::Result<()> {
        dispatch!(self, solver => solver.reserve(var))
    }

    pub(super) fn add_clause(&mut self, clause: Clause) -> anyhow::Result<()> {
        dispatch!(self, solver => solver.add_clause(clause))
    }

    pub(super) fn add_cnf(&mut self, cnf: Cnf) -> anyhow::Result<()> {
        dispatch!(self, solver => solver.add_cnf(cnf))
    }

    pub(super) fn solve_assumps(&mut self, assumptions: &[Lit]) -> anyhow::Result<SolverResult> {
        dispatch!(self, solver => solver.solve_assumps(assumptions))
    }

    pub(super) fn full_solution(&self) -> anyhow::Result<Assignment> {
        dispatch!(self, solver => solver.full_solution())
    }
}
