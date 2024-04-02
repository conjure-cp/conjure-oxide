use schemars::JsonSchema;
use serde::Serialize;
use serde_with::skip_serializing_none;

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
