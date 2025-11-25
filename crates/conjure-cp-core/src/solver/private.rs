// Used to limit calling trait functions outside this module.
#[doc(hidden)]
pub struct Internal;

// https://predr.ag/blog/definitive-guide-to-sealed-traits-in-rust/#the-trick-for-sealing-traits
// Make traits unimplementable from outside of this module.
#[doc(hidden)]
pub trait Sealed {}
