use crate::ast::Expression;
use crate::Model;
use clap::ValueEnum;
use colored::Colorize;
use serde_json;
use std::any::Any;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::{fmt, fs::OpenOptions, io::Write};

/// Different kinds of messages to be registered by `display_message`.
#[derive(PartialEq, Clone, Debug, ValueEnum)]
pub enum Kind {
    Parser,
    Rules,
    Error,
    Solver,
    Model,
    Default,
}

/// Create kind_filter for message as a global variable.
pub static KIND_FILTER: Mutex<Option<Kind>> = Mutex::new(None);

/// Set the kind_filter. If no Kind specified, set as Default.
pub fn set_kind_filter(kind: Option<Kind>) {
    let mut filter = KIND_FILTER.lock().unwrap();
    *filter = Some(kind.unwrap_or(Kind::Default));
}

/// Get the kind_filter.
pub fn get_kind_filter() -> Option<Kind> {
    let filter = KIND_FILTER.lock().unwrap();
    filter.clone()
}

/// Create rule_filter for rule filtering as a global variable.
pub static RULE_FILTER: Mutex<Option<Vec<String>>> = Mutex::new(None);

/// Create rule_set_filter for rule set filtering as a global variable.
pub static RULE_SET_FILTER: Mutex<Option<Vec<String>>> = Mutex::new(None);

/// Set the rule_filter.
pub fn set_rule_filter(rule_name: Option<Vec<String>>) {
    let mut filter = RULE_FILTER.lock().unwrap();
    *filter = rule_name;
}

/// Get the rule_filter.
pub fn get_rule_filter() -> Option<Vec<String>> {
    let filter = RULE_FILTER.lock().unwrap();
    filter.clone()
}

/// Set the rule_set_filter.
pub fn set_rule_set_filter(rule_set: Option<Vec<String>>) {
    let mut filter = RULE_SET_FILTER.lock().unwrap();
    *filter = rule_set;
}

/// Get the rule_set_filter.
pub fn get_rule_set_filter() -> Option<Vec<String>> {
    let filter = RULE_SET_FILTER.lock().unwrap();
    filter.clone()
}

#[derive(serde::Serialize, Clone)]

/// Representation of a rule trace.
pub struct RuleTrace {
    pub initial_expression: Expression,
    pub rule_name: String,
    pub rule_priority: u16,
    pub rule_set_name: String,
    pub transformed_expression: Option<Expression>,
    pub new_variables_str: Option<String>,
    pub top_level_str: Option<String>,
}

/// Representation of the [Model] trace.
pub struct ModelTrace {
    pub initial_model: Model,
    pub rewritten_model: Option<Model>,
}

/// Different types of traces.
#[derive(Clone)]
pub enum TraceType<'a> {
    RuleTrace(RuleTrace),
    ModelTrace(&'a ModelTrace),
}

/// Provides a human readable representation of the [RuleTrace] struct.
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
            write!(f, "\n{}", vars)?;
        }
        if let Some(top) = &self.top_level_str {
            write!(f, "\n{}", top)?;
        }

        write!(f, "\n--\n")?;
        Ok(())
    }
}

/// Human-readable representation of [ModelTrace].
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

#[derive(clap::ValueEnum, serde::Serialize, Clone, PartialEq, Default, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum VerbosityLevel {
    #[default]
    Low,
    Medium,
    High,
}

/// Trait for capturing traces of a specific type.
///
/// Implementors of this trait can define how to store or process traces
pub trait Trace {
    fn capture(&self, trace: TraceType);
}

/// Trait for formatting trace output into a string representation.
pub trait MessageFormatter: Any + Send + Sync {
    fn format(&self, trace: TraceType) -> String;
}

/// Helper enum to check the formatter type later on.
#[derive(PartialEq)]
pub enum FormatterType {
    Json,
    Human,
    Both,
}

pub struct HumanFormatter;

/// Human-readable formatter implementing the [MessageFormatter] trait.
impl MessageFormatter for HumanFormatter {
    fn format(&self, trace: TraceType) -> String {
        match trace {
            TraceType::RuleTrace(rule_trace) => {
                // check if there are any filters applied and if the rule matches the filters
                let no_filter = get_rule_filter().is_none() && get_rule_set_filter().is_none();
                let rule_filter_matches =
                    get_rule_filter().is_some_and(|filter| filter.contains(&rule_trace.rule_name));
                let rule_set_filter_matches = get_rule_set_filter()
                    .is_some_and(|filter| filter.contains(&rule_trace.rule_set_name));

                // return an empty string if the rule does not match the filters given
                if !(no_filter || rule_filter_matches || rule_set_filter_matches) {
                    return String::new();
                }

                if rule_trace.transformed_expression.is_some() {
                    format!("Successful Tranformation: \n{}", rule_trace)
                } else {
                    format!("Unsuccessful Tranformation: \n{}", rule_trace)
                }
            }
            TraceType::ModelTrace(model) => format!("{}", model),
        }
    }
}

pub struct JsonFormatter;

/// JSON formatter implementing the [MessageFormatter] trait.
impl MessageFormatter for JsonFormatter {
    fn format(&self, trace: TraceType) -> String {
        match trace {
            TraceType::RuleTrace(rule_trace) => {
                // check if there are any filters applied and if the rule matches the filters
                let no_filter = get_rule_filter().is_none() && get_rule_set_filter().is_none();
                let rule_filter_matches =
                    get_rule_filter().is_some_and(|filter| filter.contains(&rule_trace.rule_name));

                let rule_set_filter_matches = get_rule_set_filter()
                    .is_some_and(|filter| filter.contains(&rule_trace.rule_set_name));

                // return an empty string if the rule does not match the filters given
                if !(no_filter || rule_filter_matches || rule_set_filter_matches) {
                    return String::new();
                }

                let json_str = serde_json::to_string_pretty(&rule_trace).unwrap();

                json_str.to_string()
            }
            TraceType::ModelTrace(_) => String::new(),
        }
    }
}

/// Different types of consumers.
///
/// A [Consumer] recieves a [TraceType], processes its data according to the consumer type, and sends it to the appropriate destination.
pub enum Consumer {
    StdoutConsumer(StdoutConsumer),
    FileConsumer(FileConsumer),
    BothConsumer(BothConsumer),
}

/// A consumer that writes trace data to standard output.
pub struct StdoutConsumer {
    pub formatter: Arc<dyn MessageFormatter>,
    pub verbosity: VerbosityLevel,
}

/// A consumer that writes trace data to file(s).
///
/// Supports outputting in JSON, plain text, or both formats depending on configuration.
pub struct FileConsumer {
    pub formatter: Arc<dyn MessageFormatter>,
    pub formatter_type: FormatterType,
    pub verbosity: VerbosityLevel,
    pub json_file_path: Option<String>, // path to file where the json trace will be written
    pub human_file_path: Option<String>, // path to file where the human trace will be written
    pub is_first: std::cell::Cell<bool>, // for json formatting, trust me
}

/// A consumer that sends data to both standard output and file.
pub struct BothConsumer {
    stdout_consumer: StdoutConsumer,
    file_consumer: FileConsumer,
}

impl Trace for StdoutConsumer {
    fn capture(&self, trace: TraceType) {
        let formatted_output = self.formatter.format(trace);
        if formatted_output == String::new() {
            return;
        }
        println!("{}", formatted_output);
    }
}

impl Trace for FileConsumer {
    fn capture(&self, trace: TraceType) {
        match self.formatter_type {
            FormatterType::Json => {
                if let Some(ref json_path) = self.json_file_path {
                    let formatted_output = self.formatter.format(trace.clone());

                    // if rule does not match filter, it will not be outputted
                    if formatted_output == String::new() {
                        return;
                    }

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

/// Helper function to write JSON trace data to a file.
///
/// This function handles inserting commas appropriately when writing to a JSON array stored in a file.
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

/// Returns the [VerbosityLevel] of a [Consumer].
pub fn check_verbosity_level(consumer: &Consumer) -> VerbosityLevel {
    match consumer {
        Consumer::StdoutConsumer(stdout_consumer) => stdout_consumer.verbosity.clone(),
        Consumer::FileConsumer(file_consumer) => file_consumer.verbosity.clone(),
        Consumer::BothConsumer(both_consumer) => both_consumer.stdout_consumer.verbosity.clone(),
    }
}

/// Provides an implementation for the `capture` method, which sends the trace to the appropriate [Consumer].
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

/// Creates a [Consumer] instance based on the specified type, verbosity, format, and output paths.
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

    // helper function to clean and create a file if it exists
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

/// Generates filenames for JSON format and Human-readable format.
/// If the user passes filenames as command-line arguments, the function just returns them
/// If not, the filenames for the trace output are generated based on the problem file.
pub fn specify_trace_files(
    essence_file: String,
    files: Option<Vec<String>>,
    output_format: &str,
) -> (Option<String>, Option<String>) {
    let path = PathBuf::from(&essence_file);

    // Extract the file name
    let stem = path
        .file_stem()
        .expect("Essence file must have a stem")
        .to_string_lossy();

    // Generates  default trace filenames (e.g., "input_protrace.json" or "input_protrace.txt") located in the appropriate test folder
    let make_default = |ext: &str| {
        let mut p = path.clone();
        p.set_file_name(format!("{}_protrace", stem));
        p.set_extension(ext);
        p.to_string_lossy().into_owned()
    };

    // Helper to extract a file path from the `files` vector or fall back to the default
    let get_or_default = |idx: usize, default: String| {
        files
            .as_ref()
            .and_then(|f| f.get(idx).cloned())
            .unwrap_or(default)
    };

    // File paths based on output format
    match output_format.to_lowercase().as_str() {
        "json" => (Some(get_or_default(0, make_default("json"))), None),
        "human" => (None, Some(get_or_default(1, make_default("txt")))),
        "both" => (
            Some(get_or_default(0, make_default("json"))),
            Some(get_or_default(1, make_default("txt"))),
        ),
        other => panic!("Unknown output format: {}", other),
    }
}

/// Closing the JSON array of rules for valid JSON format
pub fn json_trace_close(json_path: Option<String>) {
    if let Some(path) = json_path {
        let path_ref = Path::new(&path);
        // Only proceed if the file exists and is not empty
        if path_ref.exists() && path_ref.metadata().map(|m| m.len()).unwrap_or(0) > 2 {
            let mut file = OpenOptions::new()
                .append(true)
                .open(path_ref)
                .expect("Failed to open JSON trace file for closing");
            writeln!(file, "]").expect("Failed to write closing bracket to JSON file");
        }
    }
}

/// General function to capture desired messages.
///
/// If a file is specified, the message will be written there, otherwise it will be printed out to the terminal
pub fn display_message(message: String, file_path: Option<String>, kind: Kind) {
    if let Some(filter) = get_kind_filter() {
        if kind != filter && kind != Kind::Default {
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
        let colored_message = match kind {
            Kind::Error => message.red(),
            _ => message.green(),
        };
        println!("{}", colored_message);
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
        let file_path = "test_trace_output_json_only.json".to_string();

        std::fs::File::create(&file_path).expect("Failed to create test file");

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
        let file_path = "test_trace_output_human_only.txt".to_string();

        std::fs::File::create(&file_path).expect("Failed to create test file");
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
        let json_file_path = "test_trace_output_both.json".to_string();
        let human_file_path = "test_trace_output_both.txt".to_string();

        std::fs::File::create(&json_file_path).expect("Failed to create test file");
        std::fs::File::create(&human_file_path).expect("Failed to create test file");
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
        let output_file = specify_trace_files("example.essence".into(), None, "json");
        assert_eq!(output_file.0.unwrap_or_default(), "example_protrace.json");
    }

    #[test]
    fn test_specify_trace_file_human() {
        let output_file = specify_trace_files("example.essence".into(), None, "human");
        assert_eq!(output_file.1.unwrap_or_default(), "example_protrace.txt");
    }

    #[test]
    fn test_specify_trace_file_both() {
        let output_file = specify_trace_files("example.essence".into(), None, "both");
        assert_eq!(output_file.0.unwrap_or_default(), "example_protrace.json");
        assert_eq!(output_file.1.unwrap_or_default(), "example_protrace.txt");
    }

    #[test]
    fn test_specify_trace_file_passed() {
        let output_file1 = specify_trace_files(
            "example.essence".into(),
            Some(vec![
                "".to_string(),
                "example_essence_trace.txt".to_string(),
            ]),
            "human",
        );

        assert_eq!(
            output_file1.1.unwrap_or_default(),
            "example_essence_trace.txt"
        );

        let output_file2 = specify_trace_files(
            "example.essence".into(),
            Some(vec!["example_essence_trace.json".to_string()]),
            "json",
        );

        assert_eq!(
            output_file2.0.unwrap_or_default(),
            "example_essence_trace.json"
        );

        let output_file3 = specify_trace_files(
            "example.essence".into(),
            Some(vec![
                "example_essence_trace.json".to_string(),
                "example_essence_trace.txt".to_string(),
            ]),
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
