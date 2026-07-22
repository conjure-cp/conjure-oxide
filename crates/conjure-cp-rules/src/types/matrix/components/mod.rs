mod representation;
mod vertical;

pub use representation::MatrixComponents;
pub(crate) use vertical::{
    try_index_matrix_components, try_lower_const_unsafe_index_matrix_components,
};
