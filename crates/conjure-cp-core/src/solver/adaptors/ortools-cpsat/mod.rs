#[cfg(not(no_ortools))]
mod adaptor;
#[cfg(not(no_ortools))]
mod convs;

#[cfg(not(no_ortools))]
pub use adaptor::OrToolsCpSat;

#[cfg(not(no_ortools))]
pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/operations_research.sat.rs"));
}

#[cfg(not(no_ortools))]
#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("src/solver/adaptors/ortools-cpsat/wrapper.hpp");

        fn solve_wrapper(model_proto: &[u8]) -> Vec<u8>;
    }
}

// Stub implementation when compiled without OR-Tools
#[cfg(no_ortools)]
pub struct OrToolsCpSat {
    __non_constructable: crate::solver::private::Internal,
}

#[cfg(no_ortools)]
impl crate::solver::private::Sealed for OrToolsCpSat {}

#[cfg(no_ortools)]
impl OrToolsCpSat {
    pub const IS_AVAILABLE: bool = false;

    pub fn new() -> Self {
        Self {
            __non_constructable: crate::solver::private::Internal,
        }
    }
}

#[cfg(no_ortools)]
impl Default for OrToolsCpSat {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(no_ortools)]
impl crate::solver::SolverAdaptor for OrToolsCpSat {
    fn load_model(
        &mut self,
        _model: crate::Model,
        _: crate::solver::private::Internal,
    ) -> Result<(), crate::solver::SolverError> {
        Err(crate::solver::SolverError::Runtime(
            "OR-Tools CP-SAT solver support was not compiled in this build because the Google OR-Tools library was missing at build time".to_owned()
        ))
    }

    fn solve(
        &mut self,
        _callback: crate::solver::SolverCallback,
        _: crate::solver::private::Internal,
    ) -> Result<crate::solver::SolveSuccess, crate::solver::SolverError> {
        Err(crate::solver::SolverError::Runtime(
            "OR-Tools CP-SAT solver support was not compiled in this build because the Google OR-Tools library was missing at build time".to_owned()
        ))
    }

    fn solve_mut(
        &mut self,
        _callback: crate::solver::SolverMutCallback,
        _: crate::solver::private::Internal,
    ) -> Result<crate::solver::SolveSuccess, crate::solver::SolverError> {
        Err(crate::solver::SolverError::Runtime(
            "OR-Tools CP-SAT solver support was not compiled in this build because the Google OR-Tools library was missing at build time".to_owned()
        ))
    }

    fn get_family(&self) -> crate::settings::SolverFamily {
        crate::settings::SolverFamily::OrToolsCpSat
    }

    fn get_name(&self) -> &'static str {
        "ortools-cpsat"
    }

    fn write_solver_input_file(&self, _writer: &mut Box<dyn std::io::Write>) -> Result<(), std::io::Error> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "OR-Tools support not compiled"
        ))
    }
}
