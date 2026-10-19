mod apply;
mod explicit;
mod generator;
mod horizontal;
mod packed;

pub use explicit::SequenceExplicit;
pub use packed::SequencePacked;

pub(crate) use apply::apply_at_position;
