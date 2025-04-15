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

// Values for different kinds of trace messages
#[derive(PartialEq, Clone, Debug, ValueEnum)]
pub enum Kind {
    Parser,
    RuleAttempt,
    RuleSuccess,
    Error,
    Solver,
    Model,
    Default,
}

/// Create kind_filter as a global variable
pub static KIND_FILTER: Mutex<Option<Kind>> = Mutex::new(None);

/// Set the kind_filter. If no Kind specified, set as Default.
pub fn set_kind_filter(kind: Option<Kind>) {
    let mut filter = KIND_FILTER.lock().unwrap();
    *filter = Some(kind.unwrap_or(Kind::Default));
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
    Both,
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
    pub json_file_path: Option<String>, // path to file where the json trace will be written
    pub human_file_path: Option<String>, // path to file where the human trace will be written
    pub is_first: std::cell::Cell<bool>, // for json formatting, trust me
}

pub struct BothConsumer {
    stdout_consumer: StdoutConsumer,
    file_consumer: FileConsumer,
}

impl Trace for StdoutConsumer {
    fn capture(&self, trace: TraceType) {
        let formatted_output = self.formatter.format(trace);
        println!("{}", formatted_output);
    }
}

impl Trace for FileConsumer {
    fn capture(&self, trace: TraceType) {
        match self.formatter_type {
            FormatterType::Json => {
                if let Some(ref json_path) = self.json_file_path {
                    let formatted_output = self.formatter.format(trace.clone());
                    let json_file = OpenOptions::new()
                        .append(true)
                        .create(true)
                        .open(json_path)
                        .unwrap();

                    write_to_json_trace_file(self, json_file, formatted_output, trace);
                }
            }
            FormatterType::Human => {
                if let Some(ref human_path) = self.human_file_path {
                    let formatted_output = self.formatter.format(trace.clone());
                    let mut human_file = OpenOptions::new()
                        .append(true)
                        .create(true)
                        .open(human_path)
                        .unwrap();

                    writeln!(human_file, "{}", formatted_output).unwrap();
                }
            }
            FormatterType::Both => {
                // For JSON output, use a JsonFormatter explicitly
                if let Some(ref json_path) = self.json_file_path {
                    let json_formatter = JsonFormatter;
                    let formatted_output = json_formatter.format(trace.clone());

                    let json_file = OpenOptions::new()
                        .append(true)
                        .create(true)
                        .open(json_path)
                        .unwrap();

                    write_to_json_trace_file(self, json_file, formatted_output, trace.clone());
                }

                // For human output, use a HumanFormatter explicitly
                if let Some(ref human_path) = self.human_file_path {
                    let human_formatter = HumanFormatter;
                    let formatted_output = human_formatter.format(trace.clone());

                    let mut human_file = OpenOptions::new()
                        .append(true)
                        .create(true)
                        .open(human_path)
                        .unwrap();

                    writeln!(human_file, "{}", formatted_output).unwrap();
                }
            }
        }
    }
}

pub fn write_to_json_trace_file(
    consumer: &FileConsumer,
    mut json_file: File,
    formatted_output: String,
    trace: TraceType,
) {
    match trace {
        // If the trace is a RuleTrace, handle the comma insertion
        TraceType::RuleTrace(_) => {
            if consumer.is_first.get() {
                writeln!(json_file, "[").unwrap(); // Start JSON array
                writeln!(json_file, "{}", formatted_output).unwrap();
                consumer.is_first.set(false);
            } else {
                writeln!(json_file, ",{}", formatted_output).unwrap(); // Append comma and object
            }
        }
        // If it's any other type, just write the formatted output without commas
        _ => {
            writeln!(json_file, "{}", formatted_output).unwrap();
        }
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
// pub fn create_consumer(
//     consumer_type: &str,
//     verbosity: VerbosityLevel,
//     output_format: &str,
//     json_file_path: String,
//     human_file_path: String,
// ) -> Consumer {
//     let (formatter, formatter_type): (Arc<dyn MessageFormatter>, FormatterType) =
//         match output_format.to_lowercase().as_str() {
//             "json" => (Arc::new(JsonFormatter), FormatterType::Json),
//             "human" => (Arc::new(HumanFormatter), FormatterType::Human),
//             other => panic!("Unknown format type: {}", other),
//         };

//     match consumer_type.to_lowercase().as_str() {
//         "stdout" => Consumer::StdoutConsumer(StdoutConsumer {
//             formatter,
//             verbosity,
//         }),
//         "file" => {
//             let path = PathBuf::from(&file_path);
//             if path.exists() {
//                 fs::remove_file(&path).unwrap();
//             }

//             // Create the file if it doesn't exist
//             File::create(&path).unwrap();

//             Consumer::FileConsumer(FileConsumer {
//                 formatter,
//                 formatter_type,
//                 verbosity,
//                 file_path,
//                 is_first: std::cell::Cell::new(true), // for json formatting, trust me
//             })
//         }
//         "both" => {
//             let path = PathBuf::from(&file_path);
//             if path.exists() {
//                 fs::remove_file(&path).unwrap();
//             }

//             Consumer::BothConsumer(BothConsumer {
//                 stdout_consumer: StdoutConsumer {
//                     formatter: formatter.clone(),
//                     verbosity: verbosity.clone(),
//                 },
//                 file_consumer: FileConsumer {
//                     formatter,
//                     formatter_type,
//                     verbosity,
//                     file_path,
//                     is_first: std::cell::Cell::new(true), // for JSON formatting
//                 },
//             })
//         }
//         other => panic!("Unknown consumer type: {}", other),
//     }
// }
pub fn create_consumer(
    consumer_type: &str,
    verbosity: VerbosityLevel,
    output_format: &str,
    json_file_path: Option<String>,
    human_file_path: Option<String>,
) -> Consumer {
    let (formatter, formatter_type): (Arc<dyn MessageFormatter>, FormatterType) =
        match output_format.to_lowercase().as_str() {
            "json" => (Arc::new(JsonFormatter), FormatterType::Json),
            "human" => (Arc::new(HumanFormatter), FormatterType::Human),
            "both" => (Arc::new(JsonFormatter), FormatterType::Both),
            other => panic!("Unknown format type: {}", other),
        };

    // Helper function to clean and create a file if it exists
    fn init_file(path_str: &String) {
        let path = PathBuf::from(path_str);
        if path.exists() {
            fs::remove_file(&path).unwrap();
        }
        File::create(&path).unwrap();
    }

    match consumer_type.to_lowercase().as_str() {
        "stdout" => Consumer::StdoutConsumer(StdoutConsumer {
            formatter,
            verbosity,
        }),

        "file" => {
            if let Some(ref json_path) = json_file_path {
                init_file(json_path);
            }
            if let Some(ref human_path) = human_file_path {
                init_file(human_path);
            }

            Consumer::FileConsumer(FileConsumer {
                formatter,
                formatter_type,
                verbosity,
                json_file_path,
                human_file_path,
                is_first: std::cell::Cell::new(true),
            })
        }

        "both" => {
            if let Some(ref json_path) = json_file_path {
                init_file(json_path);
            }
            if let Some(ref human_path) = human_file_path {
                init_file(human_path);
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
                    json_file_path,
                    human_file_path,
                    is_first: std::cell::Cell::new(true),
                },
            })
        }

        other => panic!("Unknown consumer type: {}", other),
    }
}
pub fn specify_trace_files(
    essence_file: String,
    json_file: Option<String>,
    human_file: Option<String>,
    output_format: &str,
) -> (Option<String>, Option<String>) {
    let path = PathBuf::from(&essence_file);
    let base_stem = path
        .file_stem()
        .expect("Essence file must have a stem")
        .to_string_lossy()
        .into_owned();

    let default_json = {
        let mut p = path.clone();
        p.set_file_name(format!("{}_protrace", base_stem));
        p.set_extension("json");
        p.to_string_lossy().into_owned()
    };

    let default_human = {
        let mut p = path.clone();
        p.set_file_name(format!("{}_protrace", base_stem));
        p.set_extension("txt");
        p.to_string_lossy().into_owned()
    };

    match output_format.to_lowercase().as_str() {
        "json" => {
            let json = Some(json_file.unwrap_or(default_json));
            (json, None)
        }
        "human" => {
            let human = Some(human_file.unwrap_or(default_human));
            (None, human)
        }
        "both" => {
            let json = Some(json_file.unwrap_or(default_json));
            let human = Some(human_file.unwrap_or(default_human));
            (json, human)
        }
        other => panic!("Unknown output format: {}", other),
    }
}

pub fn json_trace_close(json_path: Option<String>) {
    if let Some(path) = json_path {
        let mut file = OpenOptions::new()
            .append(true)
            .open(path)
            .expect("Failed to open JSON trace file");
        writeln!(file, "]").expect("Failed to write to file");
    } else {
        println!("No JSON file path provided.");
    }
}

/// General function to capture desired messages
/// If a file is specified, the message will be written there, otherwise it will be printed out to the terminal
pub fn display_message(message: String, file_path: Option<String>, kind: Kind) {
    if let Some(filter) = get_kind_filter() {
        if kind != filter {
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
        let consumer = create_consumer("stdout", VerbosityLevel::High, "human", None, None);
        assert!(matches!(consumer, Consumer::StdoutConsumer(_)));
    }

    #[test]
    fn test_create_file_consumer_for_json() {
        let file_path = "test_trace_output.json".to_string();
        let consumer = create_consumer(
            "file",
            VerbosityLevel::Medium,
            "json",
            Some(file_path.clone()),
            None,
        );
        if let Consumer::FileConsumer(file_consumer) = &consumer {
            assert_eq!(file_consumer.json_file_path.as_ref().unwrap(), &file_path);
        } else {
            panic!("Expected a Json FileConsumer");
        }

        assert!(matches!(consumer, Consumer::FileConsumer(_)));
        fs::remove_file(file_path).ok();
    }

    #[test]
    fn test_create_file_consumer_for_human() {
        let file_path = "test_trace_output.txt".to_string();
        let consumer = create_consumer(
            "file",
            VerbosityLevel::Medium,
            "human",
            None,
            Some(file_path.clone()),
        );
        if let Consumer::FileConsumer(file_consumer) = &consumer {
            assert_eq!(file_consumer.human_file_path.as_ref().unwrap(), &file_path);
        } else {
            panic!("Expected a Human FileConsumer");
        }

        assert!(matches!(consumer, Consumer::FileConsumer(_)));
        fs::remove_file(file_path).ok();
    }

    #[test]
    fn test_create_file_consumer_for_both() {
        let json_file_path = "test_trace_output.json".to_string();
        let human_file_path = "test_trace_output.txt".to_string();
        let consumer = create_consumer(
            "file",
            VerbosityLevel::Medium,
            "both",
            Some(json_file_path.clone()),
            Some(human_file_path.clone()),
        );
        if let Consumer::FileConsumer(file_consumer) = &consumer {
            assert_eq!(
                file_consumer.human_file_path.as_ref().unwrap(),
                &human_file_path
            );
            assert_eq!(
                file_consumer.json_file_path.as_ref().unwrap(),
                &json_file_path
            );
        } else {
            panic!("Expected a Joint FileConsumer");
        }

        assert!(matches!(consumer, Consumer::FileConsumer(_)));
        fs::remove_file(json_file_path).ok();
        fs::remove_file(human_file_path).ok();
    }

    #[test]
    fn test_check_verbosity_level() {
        let consumer = Consumer::StdoutConsumer(StdoutConsumer {
            formatter: Arc::new(HumanFormatter),
            verbosity: VerbosityLevel::High,
        });
        assert_eq!(check_verbosity_level(&consumer), VerbosityLevel::High);

        let consumer_2 = create_consumer("stdout", VerbosityLevel::Medium, "human", None, None);
        assert_eq!(check_verbosity_level(&consumer_2), VerbosityLevel::Medium);
    }

    #[test]
    fn test_specify_trace_file_json() {
        let output_file = specify_trace_files("example.essence".into(), None, None, "json");
        assert_eq!(output_file.0.unwrap_or_default(), "example_protrace.json");
    }

    #[test]
    fn test_specify_trace_file_human() {
        let output_file = specify_trace_files("example.essence".into(), None, None, "human");
        assert_eq!(output_file.1.unwrap_or_default(), "example_protrace.txt");
    }

    #[test]
    fn test_specify_trace_file_both() {
        let output_file = specify_trace_files("example.essence".into(), None, None, "both");
        assert_eq!(output_file.0.unwrap_or_default(), "example_protrace.json");
        assert_eq!(output_file.1.unwrap_or_default(), "example_protrace.txt");
    }

    #[test]
    fn test_specify_trace_file_passed() {
        let output_file1 = specify_trace_files(
            "example.essence".into(),
            None,
            Some("example_essence_trace.txt".to_string()),
            "human",
        );

        assert_eq!(
            output_file1.1.unwrap_or_default(),
            "example_essence_trace.txt"
        );

        let output_file2 = specify_trace_files(
            "example.essence".into(),
            Some("example_essence_trace.json".to_string()),
            None,
            "json",
        );

        assert_eq!(
            output_file2.0.unwrap_or_default(),
            "example_essence_trace.json"
        );

        let output_file3 = specify_trace_files(
            "example.essence".into(),
            Some("example_essence_trace.json".to_string()),
            Some("example_essence_trace.txt".to_string()),
            "both",
        );
        assert_eq!(
            output_file3.0.unwrap_or_default(),
            "example_essence_trace.json"
        );
        assert_eq!(
            output_file3.1.unwrap_or_default(),
            "example_essence_trace.txt"
        );
    }
}
