//! Implementations of Uniplate and Biplate for common types
//!
//! This includes stdlib types as well as common collections
//!
//! Box types are excluded, and are inlined by the macro.

// NOTE (niklasdewally): my assumption is that we can do all this here, and that llvm will inline
// this and/or devirtualise the Box<dyn Fn()> when necessary to make this fast.
// https://users.rust-lang.org/t/why-box-dyn-fn-is-the-same-fast-as-normal-fn/96392

use im::Vector;
use std::collections::VecDeque;

use crate::biplate::*;
use crate::derive_iter;
use crate::derive_unplateable;
use crate::Tree::*;

derive_unplateable!(i8);
derive_unplateable!(i16);
derive_unplateable!(i32);
derive_unplateable!(i64);
derive_unplateable!(i128);
derive_unplateable!(u8);
derive_unplateable!(u16);
derive_unplateable!(u32);
derive_unplateable!(u64);
derive_unplateable!(u128);
derive_unplateable!(String);

/*****************************/
/*        Collections        */
/*****************************/

// Implement Biplate for collections by converting them to iterators.

derive_iter!(Vec);
derive_iter!(VecDeque);
derive_iter!(Vector);
