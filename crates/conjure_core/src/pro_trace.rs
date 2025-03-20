use crate::ast::Expression;
use serde_json;
use std::fs;
use std::path::PathBuf;
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
            "{}, \n~~> {} [{};{}]",
            self.initial_expression, self.rule_name, self.rule_priority, self.rule_set_name,
        )?;

        if let Some(expr) = &self.transformed_expression {
            write!(f, "\n{}", expr)?;
        }
        if let Some(vars) = &self.new_variables_str {
            write!(f, " {}", vars)?;
        }
        if let Some(top) = &self.top_level_str {
            write!(f, " {}", top)?;
        }

        write!(f, "\n--\n")?;
        Ok(())
    }
}

/// represents the level of detail in the trace
#[derive(clap::ValueEnum, serde::Serialize, Clone, PartialEq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum VerbosityLevel {
    #[default]
    Low,
    Medium,
    High,
}
/// defines trait for formatting traces
pub trait Trace {
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
                if (rule_trace.transformed_expression.is_some()) {
                    format!("Successful Tranformation: \n{}", rule_trace)
                } else {
                    format!("Unsuccessful Tranformation: \n{}", rule_trace)
                }
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
            TraceStruct::RuleTrace(rule_trace) => {
                serde_json::to_string_pretty(&rule_trace).unwrap()
            }
            _ => String::from("Unknown trace"),
        }
    }
}

// represents the different types of consumers
// one consumer writes to the console, the other writes to a file
// a Consumer recieves a TraceStruct, processes its data
// according to the consumer type, and sends it to the appropriate destination
pub enum Consumer {
    StdoutConsumer(StdoutConsumer),
    FileConsumer(FileConsumer),
}

pub struct StdoutConsumer {
    pub formatter: Box<dyn MessageFormatter>,
    pub verbosity: VerbosityLevel,
}

pub struct FileConsumer {
    pub formatter: Box<dyn MessageFormatter>,
    pub verbosity: VerbosityLevel,
    pub file_path: String, // path to file where the trace will be written
}

// implementation of the Trace trait for the StdoutConsumer struct
// provides an implementation for the capture method, which
// formats the trace and prints it to the console
// impl<F: MessageFormatter> Trace<F> for StdoutConsumer {
//     fn capture(&self, trace: TraceStruct) {
//         let formatted_output = self.formatter.format(trace);
//         println!("{}", formatted_output);
//     }
// }
impl Trace for StdoutConsumer {
    fn capture(&self, trace: TraceStruct) {
        let formatted_output = self.formatter.format(trace);
        println!("{}", formatted_output);
    }
}

// implementation of the Trace trait for the FileConsumer struct
// impl<F: MessageFormatter> Trace<F> for FileConsumer {
//     fn capture(&self, trace: TraceStruct) {
//         let formatted_output = self.formatter.format(trace);
//         let mut file = OpenOptions::new()
//             .append(true)
//             .create(true)
//             .open(&self.file_path)
//             .unwrap();
//         writeln!(file, "{}", formatted_output).unwrap();
//         // could do better error handling with Ok(()) ? or expect()
//     }
// }
impl Trace for FileConsumer {
    fn capture(&self, trace: TraceStruct) {
        let formatted_output = self.formatter.format(trace);
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.file_path)
            .unwrap();
        writeln!(file, "{}", formatted_output).unwrap();
    }
}

// which returns the verbosity level of the consumer
pub fn check_verbosity_level(consumer: &Consumer) -> VerbosityLevel {
    match consumer {
        Consumer::StdoutConsumer(stdout_consumer) => stdout_consumer.verbosity.clone(),
        Consumer::FileConsumer(file_consumer) => file_consumer.verbosity.clone(),
    }
}

// provides an implementation for the capture method, which
// sends the trace to the appropriate consumer
pub fn capture_trace(consumer: &Consumer, trace: TraceStruct) {
    match consumer {
        Consumer::StdoutConsumer(stdout_consumer) => {
            stdout_consumer.capture(trace);
        }
        Consumer::FileConsumer(file_consumer) => {
            file_consumer.capture(trace);
        }
    }
}

pub fn create_consumer(
    consumer_type: &str,
    verbosity: VerbosityLevel,
    output_format: &str,
    file_path: String,
) -> Consumer {
    let formatter: Box<dyn MessageFormatter> = match output_format.to_lowercase().as_str() {
        "json" => Box::new(JsonFormatter),
        "human" => Box::new(HumanFormatter),
        other => panic!("Unknown format type: {}", other),
    };

    match consumer_type.to_lowercase().as_str() {
        "stdout" => Consumer::StdoutConsumer(StdoutConsumer {
            formatter,
            verbosity,
        }),
        "file" => {
            let path = PathBuf::from(&file_path);
            if path.exists() {
                fs::remove_file(&path).unwrap();
            }

            Consumer::FileConsumer(FileConsumer {
                formatter,
                verbosity,
                file_path,
            })
        }
        other => panic!("Unknown consumer type: {}", other),
    }
}

pub fn specify_trace_file(
    essence_file: String,
    passed_file: Option<String>,
    output_format: &str,
) -> String {
    match passed_file {
        Some(trace_file) => trace_file,
        None => {
            let new_extension = match output_format.to_lowercase().as_str() {
                "json" => "json",
                "human" => "txt",
                _ => panic!("Unknown output format: {}", output_format),
            };

            let mut path = PathBuf::from(essence_file);

            if let Some(stem) = path.file_stem() {
                let mut new_name = stem.to_string_lossy().into_owned();
                new_name.push_str("_protrace"); // Append `_protrace`

                path.set_file_name(new_name);
                path.set_extension(new_extension); // Set new extension
            }

            path.to_string_lossy().into_owned()
        }
    }
}
