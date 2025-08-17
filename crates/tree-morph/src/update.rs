use crate::commands::Commands;
use uniplate::Uniplate;

/// Represents the effects of a successful rule application.
///
/// Contains the new subtree and any side-effects. This type is not intended to be constructed
/// directly, but rather created by the engine to pass to the user-defined selector functions.
pub struct Update<T, M>
where
    T: Uniplate,
{
    pub(crate) new_subtree: T,
    pub(crate) commands: Commands<T, M>,
}

impl<T: Uniplate, M> Update<T, M> {
    /// The new subtree to be inserted as a result of applying this [`Update`]
    pub fn new_subtree(&self) -> &T {
        &self.new_subtree
    }
}
