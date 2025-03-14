use crate::ast::Expression;
use serde_json;
use std::{fmt, fs::OpenOptions, io::Write};

#[derive(serde::Serialize)] // added for serialisation to JSON using serde
/// represents the trace of a rule application
pub struct RuleTrace {
    pub initial_expression: Expression,
    pub rule_name: String,
    pub rule_priority: u16,
    pub rule_set_name: String,
    pub transformed_expression: Option<Expression>,
    pub new_variables_str: Option<String>,
    pub top_level_str: Option<String>,
}

/// represents different types of traces
pub enum TraceStruct {
    RuleTrace(RuleTrace),
    Model,
}

/// provides a human readable representation of the RuleTrace struct
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

/// represents the level of detail in the trace
#[derive(Clone, PartialEq)]
pub enum VerbosityLevel {
    Low,
    Medium,
    High,
}
/// defines trait for formatting traces
pub trait Trace<F: MessageFormatter> {
    fn capture(&self, trace: TraceStruct);
}

pub trait MessageFormatter {
    fn format(&self, trace: TraceStruct) -> String;
}

pub struct HumanFormatter;

// human-readable formatter implementing the MessageFormatter trait
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

pub struct JsonFormatter;

// JSON formatter implementing the MessageFormatter trait
impl MessageFormatter for JsonFormatter {
    fn format(&self, trace: TraceStruct) -> String {
        match trace {
            TraceStruct::RuleTrace(rule_trace) => serde_json::to_string(&rule_trace).unwrap(),
            _ => String::from("Unknown trace"),
        }
    }
}

// represents the different types of consumers
// one consumer writes to the console, the other writes to a file
// a Consumer recieves a TraceStruct, processes its data
// according to the consumer type, and sends it to the appropriate destination
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
    pub verbosity: VerbosityLevel,
    pub file_path: String, // path to file where the trace will be written
}

// implementation of the Trace trait for the StdoutConsumer struct
// provides an implementation for the capture method, which
// formats the trace and prints it to the console
impl<F: MessageFormatter> Trace<F> for StdoutConsumer<F> {
    fn capture(&self, trace: TraceStruct) {
        let formatted_output = self.formatter.format(trace);
        println!("{}", formatted_output);
    }
}

// implementation of the Trace trait for the FileConsumer struct
impl<F: MessageFormatter> Trace<F> for FileConsumer<F> {
    fn capture(&self, trace: TraceStruct) {
        let formatted_output = self.formatter.format(trace);
        let mut file = OpenOptions::new()
            .append(true) // overwrite the file if it already exists
            .create(true)
            .open(&self.file_path)
            .unwrap();
        writeln!(file, "{}", formatted_output).unwrap();
        // could do better error handling with Ok(()) ? or expect()
    }
}

// which returns the verbosity level of the consumer
pub fn check_verbosity_level<F>(consumer: &Consumer<F>) -> VerbosityLevel
where
    F: MessageFormatter,
{
    match consumer {
        Consumer::StdoutConsumer(stdout_consumer) => stdout_consumer.verbosity.clone(),
        Consumer::FileConsumer(file_consumer) => file_consumer.verbosity.clone(),
    }
}

// provides an implementation for the capture method, which
// sends the trace to the appropriate consumer
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
