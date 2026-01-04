mod attrs;
mod domain;
mod error;
mod ground;
mod range;
mod unresolved;

pub use attrs::{FuncAttr, JectivityAttr, PartialityAttr, SetAttr, MSetAttr};
pub use domain::{Domain, DomainPtr, HasDomain, Int};
pub use error::DomainOpError;
pub use ground::{GroundDomain, RecordEntryGround};
pub use range::Range;
pub use unresolved::{IntVal, RecordEntry, UnresolvedDomain};
