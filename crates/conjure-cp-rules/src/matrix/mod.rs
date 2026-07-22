mod bubble;
mod comparison;
pub(crate) mod components;
mod flatten;
mod indexed_flatten;
mod matrix_to_list;
mod remove_dimension;
pub(crate) use components::{
    try_index_matrix_components, try_lower_const_unsafe_index_matrix_components,
};
