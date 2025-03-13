use crate::ast::Expression;
use std::fmt;

pub struct RuleTrace {
    pub initial_expression: Expression,
    pub rule_name: String,
    pub rule_priority: u16,
    pub rule_set_name: String,
    pub transformed_expression: Option<Expression>,
    pub new_variables_str: Option<String>,
    pub top_level_str: Option<String>,
}

pub enum TraceStruct {
    RuleTrace(RuleTrace),
    Model,
}

impl fmt::Display for RuleTrace {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}, ~~> {} [{}{}]",
            self.initial_expression, self.rule_name, self.rule_priority, self.rule_set_name,
        )?;

        if let Some(expr) = &self.transformed_expression {
            write!(f, " -> {}", expr)?;
        }
        if let Some(vars) = &self.new_variables_str {
            write!(f, " {}", vars)?;
        }
        if let Some(top) = &self.top_level_str {
            write!(f, " {}", top)?;
        }

        write!(f, " --")?;
        Ok(())
    }
}

#[derive(Clone, PartialEq)]
pub enum VerbosityLevel {
    Low,
    Medium,
    High,
}

pub trait Trace<F: MessageFormatter> {
    fn capture(&self, trace: TraceStruct);
}

pub trait MessageFormatter {
    fn format(&self, trace: TraceStruct) -> String;
}

pub struct HumanFormatter;

impl MessageFormatter for HumanFormatter {
    fn format(&self, trace: TraceStruct) -> String {
        match trace {
            TraceStruct::RuleTrace(rule_trace) => {
                format!("Formatted rule trace: {}", rule_trace)
            }
            _ => String::from("Unknown trace"),
        }
    }
}

pub enum Consumer<F: MessageFormatter> {
    StdoutConsumer(StdoutConsumer<F>),
    FileConsumer(FileConsumer<F>),
}

pub struct StdoutConsumer<F: MessageFormatter> {
    pub formatter: F,
    pub verbosity: VerbosityLevel,
}

pub struct FileConsumer<F: MessageFormatter> {
    pub formatter: F,
}

impl<F: MessageFormatter> Trace<F> for StdoutConsumer<F> {
    fn capture(&self, trace: TraceStruct) {
        let formatted_output = self.formatter.format(trace);
        println!("{}", formatted_output);
    }
}

impl<F: MessageFormatter> Trace<F> for FileConsumer<F> {
    fn capture(&self, trace: TraceStruct) {}
}

pub fn check_verbosity_level<F>(consumer: &Consumer<F>) -> VerbosityLevel
where
    F: MessageFormatter,
{
    match consumer {
        Consumer::StdoutConsumer(stdout_consumer) => stdout_consumer.verbosity.clone(),
        Consumer::FileConsumer(_) => VerbosityLevel::Medium,
    }
}

pub fn capture_trace<F>(consumer: &Consumer<F>, trace: TraceStruct)
where
    F: MessageFormatter,
{
    match consumer {
        Consumer::StdoutConsumer(stdout_consumer) => {
            stdout_consumer.capture(trace);
        }
        Consumer::FileConsumer(file_consumer) => {
            file_consumer.capture(trace);
        }
    }
}
