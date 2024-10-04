#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The rule does not apply to the current node, but other rules might.
    NotApplicable,

    /// The current node and its descendants up to a given depth should be ignored.
    /// With a depth of `0`, only the current node is ignored.
    ///
    /// No rules are attempted on the ignored nodes.
    Ignore(u32),

    /// The current node and all its descendants should be ignored.
    /// Equivalent to `Ignore(Infinity)`
    ///
    /// No rules are attempted on the ignored nodes.
    Prune,
}
