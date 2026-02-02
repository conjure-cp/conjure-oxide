use schemars::JsonSchema;
use serde::Serialize;
use serde_with::skip_serializing_none;

/// Represents the statistical data collected during the model rewriting process.
///
/// The `RewriterStats` struct is used to track various metrics and statistics related to the rewriting
/// of a model using a set of rules. These statistics can be used for performance monitoring, debugging,
/// and optimization purposes. The structure supports optional fields, allowing selective tracking of data
/// without requiring all fields to be set.
///
/// The struct uses the following features:
/// - `#[skip_serializing_none]`: Skips serializing fields that have a value of `None`, resulting in cleaner JSON output.
/// - `#[serde(rename_all = "camelCase")]`: Uses camelCase for all serialized field names, adhering to common JSON naming conventions.
///
/// # Fields
/// - `is_optimization_enabled`:
///   - Type: `Option<bool>`
///   - Indicates whether optimizations were enabled during the rewriting process.
///   - If `Some(true)`, it means optimizations were enabled.
///   - If `Some(false)`, optimizations were explicitly disabled.
///   - If `None`, the status of optimizations is unknown or not tracked.
///
/// - `rewriter_run_time`:
///   - Type: `Option<std::time::Duration>`
///   - The total runtime duration of the rewriter in the current session.
///   - If set, it indicates the amount of time spent on rewriting, measured as a `Duration`.
///   - If `None`, the runtime is either unknown or not tracked.
///
/// - `rewriter_rule_application_attempts`:
///   - Type: `Option<usize>`
///   - The number of rule application attempts made during the rewriting process.
///   - An attempt is counted each time a rule is evaluated, regardless of whether it was successfully applied.
///   - If `None`, this metric is not tracked or not applicable for the current session.
///
/// - `rewriter_rule_applications`:
///   - Type: `Option<usize>`
///   - The number of successful rule applications during the rewriting process.
///   - A successful application means the rule was successfully applied to transform the expression or constraint.
///   - If `None`, this metric is not tracked or not applicable for the current session.
///
/// # Example
///
/// let stats = RewriterStats {
///     is_optimization_enabled: Some(true),
///     rewriter_run_time: Some(std::time::Duration::new(2, 0)),
///     rewriter_rule_application_attempts: Some(15),
///     rewriter_rule_applications: Some(10),
/// };
///
/// // Serialize the stats to JSON
/// let serialized_stats = serde_json::to_string(&stats).unwrap();
/// println!("Serialized Stats: {}", serialized_stats);
///
///
/// # Usage Notes
/// - This struct is intended to be used in contexts where tracking the performance and behavior of rule-based
///   rewriting systems is necessary. It is designed to be easily serialized and deserialized to/from JSON, making it
///   suitable for logging, analytics, and reporting purposes.
///
/// # See Also
/// - [`serde_with::skip_serializing_none`]: For skipping `None` values during serialization.
/// - [`std::time::Duration`]: For measuring and representing time intervals.
#[skip_serializing_none]
#[derive(Default, Serialize, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct RewriterStats {
    pub is_optimization_enabled: Option<bool>,
    pub rewriter_run_time: Option<std::time::Duration>,
    pub rewriter_rule_application_attempts: Option<usize>,
    pub rewriter_rule_applications: Option<usize>,
}

impl RewriterStats {
    pub fn new() -> Self {
        Self {
            is_optimization_enabled: None,
            rewriter_run_time: None,
            rewriter_rule_application_attempts: None,
            rewriter_rule_applications: None,
        }
    }
}
