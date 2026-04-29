mod adaptor;
mod convs;

pub use adaptor::OrToolsCpSat;

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/operations_research.sat.rs"));
}
