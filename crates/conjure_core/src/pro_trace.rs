use crate::ast::Expression;
use serde_json;
use std::any::Any;
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
    fn capture(&self, trace: TraceStruct);
}

pub trait MessageFormatter: Any + Send + Sync {
    fn format(&self, trace: TraceStruct) -> String;
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
    fn format(&self, trace: TraceStruct) -> String {
        match trace {
            TraceStruct::RuleTrace(rule_trace) => {
                if rule_trace.transformed_expression.is_some() {
                    format!("Successful Tranformation: \n{}", rule_trace)
                } else {
                    format!("Unsuccessful Tranformation: \n{}", rule_trace)
                }
            }
            _ => String::from("Unknown trace"),
        }
    }
}

// add a collection of traces
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
    BothConsumer(BothConsumer),
}

pub struct StdoutConsumer {
    pub formatter: Box<dyn MessageFormatter>,
    pub verbosity: VerbosityLevel,
}

pub struct FileConsumer {
    pub formatter: Box<dyn MessageFormatter>,
    pub formatter_type: FormatterType, // trust me again, it's for the json formtting
    pub verbosity: VerbosityLevel,
    pub file_path: String, // path to file where the trace will be written
    pub is_first: std::cell::Cell<bool>, // for json formatting, trust me
}

pub struct BothConsumer {
    pub formatter: Box<dyn MessageFormatter>,
    pub formatter_type: FormatterType, // trust me again, it's for the json formtting
    pub verbosity: VerbosityLevel,
    pub file_path: String, // path to file where the trace will be written
    pub is_first: std::cell::Cell<bool>, // for json formatting, trust me
}

impl Trace for StdoutConsumer {
    fn capture(&self, trace: TraceStruct) {
        let formatted_output = self.formatter.format(trace);
        println!("{}", formatted_output);
    }
}

impl Trace for FileConsumer {
    fn capture(&self, trace: TraceStruct) {
        let formatted_output = self.formatter.format(trace);
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.file_path)
            .unwrap();
        if self.formatter_type == FormatterType::Json {
            if self.is_first.get() {
                writeln!(file, "[").unwrap();
                writeln!(file, "{}", formatted_output).unwrap();
                self.is_first.set(false);
            } else {
                writeln!(file, ",\n{}", formatted_output).unwrap();
            }
        } else {
            writeln!(file, "{}", formatted_output).unwrap();
        }
    }
}

pub fn finalise_trace_file(path: &str) {
    println!("{}", &path);

    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .unwrap();
    writeln!(file, "\n]").unwrap();
}

impl Trace for BothConsumer {
    fn capture(&self, trace: TraceStruct) {
        let formatted_output = self.formatter.format(trace);
        println!("{}", formatted_output);
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.file_path)
            .unwrap();
        if self.formatter_type == FormatterType::Json {
            if self.is_first.get() {
                writeln!(file, "[").unwrap();
                writeln!(file, "{}", formatted_output).unwrap();
                self.is_first.set(false);
            } else {
                writeln!(file, ",\n{}", formatted_output).unwrap();
            }
        } else {
            writeln!(file, "{}", formatted_output).unwrap();
        }
    }
}

// which returns the verbosity level of the consumer
pub fn check_verbosity_level(consumer: &Consumer) -> VerbosityLevel {
    match consumer {
        Consumer::StdoutConsumer(stdout_consumer) => stdout_consumer.verbosity.clone(),
        Consumer::FileConsumer(file_consumer) => file_consumer.verbosity.clone(),
        Consumer::BothConsumer(both_consumer) => both_consumer.verbosity.clone(),
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
        Consumer::BothConsumer(both_consumer) => both_consumer.capture(trace),
    }
}

pub fn create_consumer(
    consumer_type: &str,
    verbosity: VerbosityLevel,
    output_format: &str,
    file_path: String,
) -> Consumer {
    let (formatter, formatter_type): (Box<dyn MessageFormatter>, FormatterType) =
        match output_format.to_lowercase().as_str() {
            "json" => (Box::new(JsonFormatter), FormatterType::Json),
            "human" => (Box::new(HumanFormatter), FormatterType::Human),
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
                formatter,
                formatter_type,
                verbosity,
                file_path,
                is_first: std::cell::Cell::new(true), // for json formatting, trust me
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
                new_name.push_str("_protrace");

                path.set_file_name(new_name);
                path.set_extension(new_extension);
            }

            path.to_string_lossy().into_owned()
        }
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
            formatter: Box::new(HumanFormatter),
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
