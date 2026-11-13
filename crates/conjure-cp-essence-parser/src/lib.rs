// Required by conjure-cp-core macros used in this crate.
use defile as _;

pub mod diagnostics;
pub mod errors;
pub mod parser;

pub use diagnostics::error_detection::collect_errors::detect_errors;
pub use errors::{FatalParseError, RecoverableParseError};
pub use parser::*;
