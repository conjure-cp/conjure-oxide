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

        unsafe fn solve_wrapper(model_proto: &[u8], callback_ptr: usize) -> Vec<u8>;
    }
    extern "Rust" {
        unsafe fn invoke_callback(callback_ptr: usize, response_proto: &[u8]) -> bool;
    }
}

#[cfg(not(no_ortools))]
unsafe fn invoke_callback(callback_ptr: usize, response_proto: &[u8]) -> bool {
    unsafe {
        let cb = &mut *(callback_ptr as *mut &mut dyn FnMut(&[u8]) -> bool);
        cb(response_proto)
    }
}

// Stub implementation when compiled without OR-Tools
#[cfg(no_ortools)]
mod stub;

#[cfg(no_ortools)]
pub use stub::OrToolsCpSat;
