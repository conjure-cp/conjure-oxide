//! Canonical names and legacy aliases for global constraint operators.
//!
//! The `is_*` predicates accept legacy spellings for backwards compatibility with
//! older Essence models. We may remove those aliases once downstream models migrate.

/// Canonical Essence name for the all-different global constraint.
pub const ALL_DIFFERENT: &str = "allDifferent";

/// Canonical Essence name for the all-different-except global constraint.
pub const ALL_DIFFERENT_EXCEPT: &str = "allDifferentExcept";

/// Canonical Essence name for the global cardinality constraint.
pub const GLOBAL_CARDINALITY: &str = "globalCardinality";

/// Canonical Essence name for at-least global cardinality.
pub const AT_LEAST: &str = "atLeast";

/// Canonical Essence name for at-most global cardinality.
pub const AT_MOST: &str = "atMost";

/// Canonical Essence name for element-id lookup.
pub const ELEMENT_ID: &str = "elementId";

pub fn is_all_different_operator(operator: &str) -> bool {
    matches!(operator, "allDifferent" | "allDiff")
}

pub fn is_all_different_except_operator(operator: &str) -> bool {
    matches!(
        operator,
        "allDifferentExcept" | "alldifferent_except" | "alldiff_except" | "allDiffExcept"
    )
}

pub fn is_at_least_operator(operator: &str) -> bool {
    matches!(operator, "atLeast" | "atleast")
}

pub fn is_at_most_operator(operator: &str) -> bool {
    matches!(operator, "atMost" | "atmost")
}

pub fn is_global_cardinality_operator(operator: &str) -> bool {
    matches!(operator, "globalCardinality" | "gcc")
}
