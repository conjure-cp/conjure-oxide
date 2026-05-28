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
mod stub;

#[cfg(no_ortools)]
pub use stub::OrToolsCpSat;
