/// Returns the default rule sets, excluding solver specific ones.
pub fn get_default_rule_sets() -> Vec<String> {
    vec![
        "Base".to_string(),
        "Constant".to_string(),
        "Bubble".to_string(),
    ]
}
