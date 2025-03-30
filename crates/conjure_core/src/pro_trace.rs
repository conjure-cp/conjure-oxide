use crate::ast::Expression;
use crate::Model;
// use once_cell::sync::Lazy;
use clap::ValueEnum;
use serde_json;
use std::any::Any;
use std::fmt::write;
use std::fs::{self, File};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::{fmt, fs::OpenOptions, io::Write};

#[derive(PartialEq, Clone, Debug, ValueEnum)]
pub enum Kind {
    Parser,
    RuleAttempt,
    RuleSuccess,
    Error,
}

// Create kind_filter as a global variable
// pub static KIND_FILTER: Option<Kind> = None;
pub static KIND_FILTER: Mutex<Option<Kind>> = Mutex::new(None);

// Set the kind_filter
pub fn set_kind_filter(kind: Option<Kind>) {
    // let mut filter = &KIND_FILTER;
    let mut filter = KIND_FILTER.lock().unwrap();
    *filter = kind;
}

// Get the kind_filter
pub fn get_kind_filter() -> Option<Kind> {
    let filter = KIND_FILTER.lock().unwrap();
    filter.clone()
}

#[derive(serde::Serialize)] // added for serialisation to JSON using serde
#[derive(Clone)]
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

pub struct ModelTrace {
    pub initial_model: Model,
    pub rewritten_model: Option<Model>,
}

/// Different types of traces
#[derive(Clone)]
pub enum TraceType<'a> {
    RuleTrace(RuleTrace),
    ModelTrace(&'a ModelTrace),
}

/// Provides a human readable representation of the RuleTrace struct
impl fmt::Display for RuleTrace {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}, \n~~> {} [{}; {}]",
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

// Human -readable representation of ModelTrace
impl fmt::Display for ModelTrace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(model) = &self.rewritten_model {
            write!(f, "\nFinal model:\n\n{}", model)?;
        } else {
            write!(
                f,
                "\nModel before rewriting:\n\n{}\n--\n",
                self.initial_model
            )?;
        }

        Ok(())
    }
}

/// represents the level of detail in the trace
#[derive(clap::ValueEnum, serde::Serialize, Clone, PartialEq, Default, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum VerbosityLevel {
    #[default]
    Low,
    Medium,
    High,
}
/// defines trait for formatting traces
pub trait Trace {
    fn capture(&self, trace: TraceType);
}

pub trait MessageFormatter: Any + Send + Sync {
    fn format(&self, trace: TraceType) -> String;
}

// it's here to be able to check the formatter type later on
#[derive(PartialEq)]
pub enum FormatterType {
    Json,
    Human,
}

pub struct HumanFormatter;

// human-readable formatter implementing the MessageFormatter trait
impl MessageFormatter for HumanFormatter {
    fn format(&self, trace: TraceType) -> String {
        match trace {
            TraceType::RuleTrace(rule_trace) => {
                if (rule_trace.transformed_expression.is_some()) {
                    format!("Successful Tranformation: \n{}", rule_trace)
                } else {
                    format!("Unsuccessful Tranformation: \n{}", rule_trace)
                }
            }
            TraceType::ModelTrace(model) => format!("{}", model),
        }
    }
}

// add a collection of traces
pub struct JsonFormatter;

// JSON formatter implementing the MessageFormatter trait
impl MessageFormatter for JsonFormatter {
    fn format(&self, trace: TraceType) -> String {
        match trace {
            TraceType::RuleTrace(rule_trace) => {
                let json_str = serde_json::to_string_pretty(&rule_trace).unwrap();

                format!("{}", json_str)
            }
            TraceType::ModelTrace(_) => String::new(),
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
    BothConsumer(BothConsumer),
}

pub struct StdoutConsumer {
    pub formatter: Arc<dyn MessageFormatter>,
    pub verbosity: VerbosityLevel,
}

pub struct FileConsumer {
    pub formatter: Arc<dyn MessageFormatter>,
    pub formatter_type: FormatterType, // trust me again, it's for the json formtting
    pub verbosity: VerbosityLevel,
    pub file_path: String, // path to file where the trace will be written
    pub is_first: std::cell::Cell<bool>, // for json formatting, trust me
}

pub struct BothConsumer {
    stdout_consumer: StdoutConsumer,
    file_consumer: FileConsumer,
    // pub formatter: Box<dyn MessageFormatter>,
    // pub formatter_type: FormatterType, // trust me again, it's for the json formtting
    // pub verbosity: VerbosityLevel,
    // pub file_path: String, // path to file where the trace will be written
    // pub is_first: std::cell::Cell<bool>, // for json formatting, trust me
}

impl Trace for StdoutConsumer {
    fn capture(&self, trace: TraceType) {
        let formatted_output = self.formatter.format(trace);
        println!("{}", formatted_output);
    }
}

impl Trace for FileConsumer {
    fn capture(&self, trace: TraceType) {
        let formatted_output = self.formatter.format(trace.clone());
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.file_path)
            .unwrap();

        if self.formatter_type == FormatterType::Json {
            match trace {
                // If the trace is a RuleTrace, handle the comma insertion
                TraceType::RuleTrace(_) => {
                    if self.is_first.get() {
                        writeln!(file, "[").unwrap(); // Start JSON array
                        writeln!(file, "{}", formatted_output).unwrap();
                        self.is_first.set(false);
                    } else {
                        writeln!(file, ",{}", formatted_output).unwrap(); // Append comma and object
                    }
                }
                // If it's any other type, just write the formatted output without commas
                _ => {
                    writeln!(file, "{}", formatted_output).unwrap();
                }
            }
        } else {
            // For non-JSON formatting, just write the formatted output
            writeln!(file, "{}", formatted_output).unwrap();
        }

        // if self.formatter_type == FormatterType::Json {
        //     if self.is_first.get() {
        //         writeln!(file, "[").unwrap(); // Start JSON array
        //         writeln!(file, "{}", formatted_output).unwrap();
        //         self.is_first.set(false);
        //     } else {
        //         writeln!(file, ",{}", formatted_output).unwrap(); // Append comma and object
        //     }
        // } else {
        //     writeln!(file, "{}", formatted_output).unwrap();
        // }
    }
}

impl Trace for BothConsumer {
    fn capture(&self, trace: TraceType) {
        self.stdout_consumer.capture(trace.clone());
        self.file_consumer.capture(trace);
    }
}

// which returns the verbosity level of the consumer
pub fn check_verbosity_level(consumer: &Consumer) -> VerbosityLevel {
    match consumer {
        Consumer::StdoutConsumer(stdout_consumer) => stdout_consumer.verbosity.clone(),
        Consumer::FileConsumer(file_consumer) => file_consumer.verbosity.clone(),
        Consumer::BothConsumer(both_consumer) => both_consumer.stdout_consumer.verbosity.clone(),
    }
}

// provides an implementation for the capture method, which
// sends the trace to the appropriate consumer
pub fn capture_trace(consumer: &Consumer, trace: TraceType) {
    match consumer {
        Consumer::StdoutConsumer(stdout_consumer) => {
            stdout_consumer.capture(trace);
        }
        Consumer::FileConsumer(file_consumer) => {
            file_consumer.capture(trace);
        }
        Consumer::BothConsumer(both_consumer) => both_consumer.capture(trace),
    }
}

/// Creates a consumer for tracing functionality based on the desired type(file/stdout), format (human/json) and verbosity of the output
pub fn create_consumer(
    consumer_type: &str,
    verbosity: VerbosityLevel,
    output_format: &str,
    file_path: String,
) -> Consumer {
    let (formatter, formatter_type): (Arc<dyn MessageFormatter>, FormatterType) =
        match output_format.to_lowercase().as_str() {
            "json" => (Arc::new(JsonFormatter), FormatterType::Json),
            "human" => (Arc::new(HumanFormatter), FormatterType::Human),
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

            // Create the file if it doesn't exist
            File::create(&path).unwrap();

            Consumer::FileConsumer(FileConsumer {
                formatter,
                formatter_type,
                verbosity,
                file_path,
                is_first: std::cell::Cell::new(true), // for json formatting, trust me
            })
        }
        "both" => {
            let path = PathBuf::from(&file_path);
            if path.exists() {
                fs::remove_file(&path).unwrap();
            }

            Consumer::BothConsumer(BothConsumer {
                stdout_consumer: StdoutConsumer {
                    formatter: formatter.clone(),
                    verbosity: verbosity.clone(),
                },
                file_consumer: FileConsumer {
                    formatter,
                    formatter_type,
                    verbosity,
                    file_path,
                    is_first: std::cell::Cell::new(true), // for JSON formatting
                },
            })

            // Consumer::BothConsumer(BothConsumer {
            //     formatter,
            //     formatter_type,
            //     verbosity,
            //     file_path,
            //     is_first: std::cell::Cell::new(true), // for json formatting, trust me
            // })
        }
        other => panic!("Unknown consumer type: {}", other),
    }
}

/// Creates a dedicated file name for the tracing output
/// If the user did not specify an output file, the file name is constructed based on the input essence file path and the type of trace expected
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
                new_name.push_str("_protrace");

                path.set_file_name(new_name);
                path.set_extension(new_extension);
            }

            path.to_string_lossy().into_owned()
        }
    }
}

/// General function to capture desired messages
/// If a file is specified, the message will be written there, otherwise it will be printed out to the terminal
pub fn display_message(message: String, file_path: Option<String>, kind: Kind) {
    if let Some(filter) = get_kind_filter() {
        if filter != kind {
            return;
        }
    }
    if let Some(file_path) = file_path {
        let mut file = match OpenOptions::new().append(true).create(true).open(file_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error opening file: {}", e);
                return;
            }
        };
        if let Err(e) = writeln!(file, "{}", message) {
            eprintln!("Error writing to file: {}", e);
        }
    } else {
        println!("{}", message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_stdout_consumer() {
        let consumer = create_consumer("stdout", VerbosityLevel::High, "human", "".into());
        assert!(matches!(consumer, Consumer::StdoutConsumer(_)));
    }

    #[test]
    fn test_create_file_consumer() {
        let file_path = "test_trace_output.json".to_string();
        let consumer = create_consumer("file", VerbosityLevel::Medium, "json", file_path.clone());
        if let Consumer::FileConsumer(file_consumer) = &consumer {
            assert_eq!(file_consumer.file_path, file_path);
        } else {
            panic!("Expected a FileConsumer");
        }

        assert!(matches!(consumer, Consumer::FileConsumer(_)));
        fs::remove_file(file_path).ok();
    }

    #[test]
    fn test_check_verbosity_level() {
        let consumer = Consumer::StdoutConsumer(StdoutConsumer {
            formatter: Arc::new(HumanFormatter),
            verbosity: VerbosityLevel::High,
        });
        assert_eq!(check_verbosity_level(&consumer), VerbosityLevel::High);

        let consumer_2 = create_consumer("stdout", VerbosityLevel::Medium, "human", "".into());
        assert_eq!(check_verbosity_level(&consumer_2), VerbosityLevel::Medium);
    }

    #[test]
    fn test_specify_trace_file_json() {
        let output_file = specify_trace_file("example.essence".into(), None, "json");
        assert_eq!(output_file, "example_protrace.json");
    }

    #[test]
    fn test_specify_trace_file_human() {
        let output_file = specify_trace_file("example.essence".into(), None, "human");
        assert_eq!(output_file, "example_protrace.txt");
    }

    #[test]
    fn test_specify_trace_file_passed() {
        let output_file = specify_trace_file(
            "example.essence".into(),
            Some("example.essence_trace".to_string()),
            "human",
        );
        assert_eq!(output_file, "example.essence_trace");
    }
}
