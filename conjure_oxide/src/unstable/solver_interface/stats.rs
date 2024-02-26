//! Statistics about a solver run.
use super::private::Sealed;

pub trait Stats: Sealed {}

struct NoStats;
impl Sealed for NoStats {}
impl Stats for NoStats {}
