mod adaptor;
mod convs;

pub use adaptor::OrToolsCpSat;

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/operations_research.sat.rs"));
}

#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("src/solver/adaptors/ortools-cpsat/wrapper.hpp");

        fn solve_wrapper(model_proto: &[u8]) -> Vec<u8>;
    }
}
