use crate::ast::Expression;
use std::fmt;

pub struct RuleTrace<'a> {
    pub initial_expression: Expression,
    pub rule_name: String,
    pub rule_sets: Vec<(&'a str, u16)>,
    pub transformed_expression: Expression,
    pub new_variables_str: String,
    pub top_level_str: String,
}
impl fmt::Display for RuleTrace<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}, \n   ~~> {} ({:?}) \n{} \n{}\n{}--\n",
            self.initial_expression,
            self.rule_name,
            self.rule_sets,
            self.transformed_expression,
            self.new_variables_str,
            self.top_level_str
        )?;

        Ok(())
    }
}
pub enum VerbosityLevel {
    Low,
    Medium,
    High,
}
pub trait Trace<F: MessageFormatter> {
    fn capture(&self, rule_trace: &RuleTrace);
}

pub trait MessageFormatter {
    fn format(&self, rule_trace: &RuleTrace) -> String;
}

pub struct HumanFormatter;
impl MessageFormatter for HumanFormatter {
    fn format(&self, rule_trace: &RuleTrace) -> String {
        format!("Formatted rule trace: {}", rule_trace)
    }
}

pub struct StdoutConsumer<F: MessageFormatter> {
    pub formatter: F,
}

impl<F: MessageFormatter> Trace<F> for StdoutConsumer<F> {
    fn capture(&self, rule_trace: &RuleTrace) {
        let formatted_output = self.formatter.format(rule_trace);
        println!("{}", formatted_output);
    }
}
