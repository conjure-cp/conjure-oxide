mod bubble;
mod comparison;
mod components;
mod flatten;
mod indexed_flatten;
mod remove_dimension;
mod to_list;
pub use components::MatrixComponents;
pub(crate) use components::{
    try_index_matrix_components, try_lower_const_unsafe_index_matrix_components,
};
