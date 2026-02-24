[//]: # (Author: Soph Morgulchik, Yi Xin Chong)
[//]: # (Last Updated: 05/05/2025)

# Overview

ProTrace is a tracing module created to trace rule applications on expressions when an Essence file is parsed and rewritten by Conjure Oxide. The purpose of the module is to visualise the rules applied on the expressions, and simplify the identification of errors for debugging. The module supports multiple output formats such as **JSON** or **human** readable, along with different verbosity levels to show only what the user would like to see.

### Verbosity Level

The different verbosity levels include:

* **`High`**: All rule applications.
* **`Medium`** (default): Successful rule applications.
* **`Low`**: Only errors are shown (to be implemented).

Outputs of the trace can be saved in a JSON or text file (depending on the format of trace) when specified by the user, and if the file path is not given, the output will be stored in the location of the input Essence file by default.

The module also provides filtering functionalities for displaying specific rule or rule set applications.
***

# Trace type

The module is capable of tracing two types of objects: rule and model. A trace can be created using `capture_trace`, which takes a consumer and a trace type ([RuleTrace](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#rule-trace) or [ModelTrace](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#model-trace)).

``` rust
pub fn capture_trace(consumer: &Consumer, trace: TraceType)
```

``` rust
pub enum TraceType<'a> {
    RuleTrace(RuleTrace),
    ModelTrace(&'a ModelTrace),
}
```

## Rule Trace

Rules are applied to a given expression to rewrite it into the syntax that Conjure understands. These rule applications can be traced by the module to show users what rules have been tried and successfully applied during parsing. A rule trace consists of:

* An **initial expression**
* The **name** of the rule being applied
* The **priority** of the rule being applied
* The **rule set** that the rule belongs to
* A **transformed expression** as a result of rule application
* An optional **new variable** created during the rule application
* An optional **new constraint** added to the top of the expression tree created during the rule application

``` rust
pub struct RuleTrace {
    pub initial_expression: Expression,
    pub rule_name: String,
    pub rule_priority: u16,
    pub rule_set_name: String,
    pub transformed_expression: Option<Expression>,
    pub new_variables_str: Option<String>,
    pub top_level_str: Option<String>,
}
```

## Model Trace

Models of the problem contain expressions along with some constraints. A model trace consists of:

* An **initial model**
* A **rewritten model** after rule application

``` rust
pub struct ModelTrace {
    pub initial_model: Model,
    pub rewritten_model: Option<Model>,
}
```

***

# Formatter

Formatters are responsible for converting trace information into a **human-readable** or **JSON** string format before it is output by a consumer.

At the core of this system is the `MessageFormatter` trait, which defines a common interface for all formatters. There are two built-in formatter implementations provided: [HumanFormatter](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#human-formatter) and [JsonFormatter](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#json-formatter).

### Message Formatter Trait

```rust
pub trait MessageFormatter: Any + Send + Sync {
    fn format(&self, trace: TraceType) -> String;
}
```

## Human Formatter

```rust
pub struct HumanFormatter
```

The `HumanFormatter` implements the [MessageFormatter](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#message-formatter-trait) trait by matching on [TraceType](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#trace-type) and returning a formatted String.
All [TraceType](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#trace-type) variants implement the `Display` trait, which allows for easy string conversion.

**Behaviour:**

* **Successful Rule Applications**

  * A message "Successful Transformation" followed by the full formatted [RuleTrace](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#rule-trace), displaying all of its fields.

* **Unsuccessful Rule Applications**

  * A message like "Unsuccessful Transformation" followed by a partial display showing only:

    * **initial expression**
    * **rule name**
    * **rule priority**
    * **rule set name**

  * Only shown if the [VerbosityLevel](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#verbosity-level) is set to `High`.

* **Model Traces**
  * For [ModelTrace](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#model-trace), it simply displays the initial and rewritten model using its Display implementation.

**Filtering**:
Before formatting, the `HumanFormatter` checks if a [rule name filter](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#rule-filters) or [rule set name filter](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#rule-filters) has been applied.

## JSON Formatter

```rust
pub struct JsonFormatter
```

The `JsonFormatter` implements the [MessageFormatter](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#message-formatter-trait) trait by matching on [TraceType](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#trace-type) and returning a JSON-formatted String.
It uses the `serde_json` library to serialize data into a **pretty-printed JSON structure**.

**Behaviour**:

* **Rule Transformations**

  * For [RuleTrace](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#rule-trace), it serializes the entire RuleTrace object into a pretty-printed JSON string using `serde_json::to_string_pretty`.

  * Similar to [HumanFormatter](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#human-formatter) displays both successful and unsuccessful rule applications when the verbosity is high.

* **Model Traces**

  * For [ModelTrace](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#model-trace), no JSON output is generated. No need since the parsed and rewritten models are produced in JSON already.

**Filtering:**
Before formatting, the `JsonFormatter` checks if a [rule name filter](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#rule-filters) or [rule set name filter](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#rule-filters) has been applied.
***

# Consumer

The **Consumer** enum represents different types of endpoints that receive, format, and output [TraceType](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#trace-type) data.

Consumers control where the trace data goes to and which format.

* [StdoutConsumer](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#stdout-consumer) prints out the trace to the console
* [FileConsumer](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#file-consumer) writes the trace data to a file in one of three ways: as human-readable text, as JSON-formatted text, or to both file types simultaneously.
* [BothConsumer](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#both-consumer) writes to both stdout and file(s).

```rust
pub enum Consumer {
    StdoutConsumer(StdoutConsumer),
    FileConsumer(FileConsumer),
    BothConsumer(BothConsumer),
}
```

### Trace Trait

Each variant implements the `Trace` trait, meaning it can capture traces and send them to the appropriate destination.

```rust
pub trait Trace {
    fn capture(&self, trace: TraceType);
}
```

## Stdout Consumer

A `StdoutConsumer` outputs the formatted trace data to standard output.

Holds:

* A reference-counted (Arc) **formatter** that implements the [MessageFormatter](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#message-formatter-trait) trait.
  * `Arc` (Atomic Reference Counted pointer) allows safe shared ownership across multiple consumers.
  * Multiple Consumers might want to share the same formatter without each owning and duplicating it.
* A **verbosity** [VerbosityLevel](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#verbosity-level) that can influence what gets printed.

```rust
pub struct StdoutConsumer {
    pub formatter: Arc<dyn MessageFormatter>,
    pub verbosity: VerbosityLevel,
}
```

`dyn` = dynamic dispatch. At runtime, it calls the correct format method for the actual formatter.
When capturing a trace:
It formats the trace using its assigned formatter.

## File Consumer

A `FileConsumer` writes the formatted trace data to one or more files.

Holds:

* A reference-counted (Arc) **formatter**.
* A **formatter type** to determine whether to write in Human, JSON, or both formats.
* **verbosity**
* Path to the **file for the JSON trace**. (Optional)
* Path to the **file for the human-readable trace**. (Optional)
* **is first** flag used for managing JSON array formatting when appending traces.

```rust
pub struct FileConsumer {
    pub formatter: Arc<dyn MessageFormatter>,
    pub formatter_type: FormatterType,
    pub verbosity: VerbosityLevel,
    pub json_file_path: Option<String>, 
    pub human_file_path: Option<String>, 
    pub is_first: std::cell::Cell<bool>, 
}

```

When capturing a trace:

Based on the selected `FormatterType`:
**Human**: Writes human-readable output to the human file.
**Json**: Writes JSON output to the JSON file.
**Both**: Writes to both files using respective formatters ([HumanFormatter](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#human-formatter) and [JsonFormatter](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#json-formatter) directly).

## Both Consumer

A `BothConsumer`sends the formatted trace data to both:

* Standard output (via an internal [StdoutConsumer](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#stdout-consumer))
* Files (via an internal [FileConsumer](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#file-consumer))

```rust
pub struct BothConsumer {
    stdout_consumer: StdoutConsumer,
    file_consumer: FileConsumer,
}

```

It combines the behaviours of [StdoutConsumer](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#stdout-consumer) and [FileConsumer](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#file-consumer) by calling method [capture](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#trace) on both its fields.
***

# Rule Filters

If users would like to only see specific rules or rule sets during rule application tracing, rule filters can be applied using the command-line argument `--filter-rule-name` or `--filter-rule-set` followed by comma-separated rule names or sets. The output will only show rules that is **in the rule filters**, otherwise the rule application will be completely skipped. This feature was added for the abstraction of unimportant rule applications during tracing, allowing users to only see the rules they require.

When both rule name and rule set filters are used in tandem, any rules that pass either the rule name or the rule set filter will be displayed. For example, if the rule name filter is `normalise_associative_commutative` and the rule set filter is `Minion`, any rule that has the rule name `normalise_associative_commutative` or is in the rule set `Minion` will be displayed.

Rule filters will be applied to both [FileConsumer](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#file-consumer) and [StdoutConsumer](https://github.com/conjure-cp/conjure-oxide/wiki/ProTrace-module/_edit#stdout-consumer).

Rule filters can also be hard-coded into the program or accessed using the following functions:

```rust
pub fn set_rule_filter(rule_name: Option<Vec<String>>)

pub fn get_rule_filter() -> Option<Vec<String>>

pub fn set_rule_set_filter(rule_set: Option<Vec<String>>)

pub fn get_rule_set_filter() -> Option<Vec<String>>
```

# Capturing general messages (independent of the main tracing functionality set by)

This feature was motivated by the need for a unified interface that allows developers to capture messages—whether to stdout or a file—with the flexibility of a typical debug print statement, but with added control.

`Default` messages are always printed out and additional message types can be filtered and viewed using the command-line argument `--get-info-about`, followed by the desired message `Kind`.

For example, running `--get-info-about rules` will display all the enabled rule sets, individual rules, their assigned priorities, and the sets they belong to. To improve readability, output is color-coded: error messages appear in red, while all other information is shown in green.

```rust
pub enum Kind {
    Parser,
    Rules,
    Error,
    Solver,
    Model,
    Default,
}
fn display_message(message: String, file_path: Option<String>, kind: Kind)


```

# Tracing in integration tests

Similar to how a `Consumer` is created in solve.rs for `cargo run solve`, a `combined_consumer` is manually constructed for each integration test, with its fields explicitly set rather than being derived from command-line arguments. This consumer is configured to log successful rule applications to two separate files—one in a human-readable format and the other in JSON. It is then passed to the `rewrite_naive` function. By establishing a uniform internal interface for tracing in both `cargo run` and `cargo test`, we were able to significantly reduce code duplication and streamline the overall tracing logic.

```rust
   let combined_consumer = create_consumer(
            "file",
            VerbosityLevel::Medium,
            "both",
            Some(format!("{path}/{essence_base}.generated-rule-trace.json")),
            Some(format!("{path}/{essence_base}.generated-rule-trace.txt")),
        );
```

The rule traces is verified using the functions `read_human_rule_trace` and `read_json_rule_trace` defined in `testing.rs` which compare the generated trace output to the expected one. `read_json_rule_trace` ignores the `id` fields since they can change from one run to another.  

# Command line arguments and flags

Enable rule tracing

    -T, --tracing

Select output location for trace result: stdout or file [default: stdout]

    -L, --trace-output <TRACE_OUTPUT>

Select verbosity level for trace [default: medium] [possible values: low, medium, high]

    --verbosity <VERBOSITY>

Select the format of the trace output: human or json [default: human]

    -F, --formatter <FORMATTER>

Optionally save traces to specific files: first for JSON, second for human format

    -f, --trace-file [<JSON_TRACE> <HUMAN_TRACE>]

Filter messages by given kind [possible values: parser, rules, error, solver, model, default]

    --get-info-about <KIND_FILTER>

Filter rule trace to only show given rule names (comma separated)

    --filter-rule-name <RULE_NAME_FILTER>

Filter rule trace to only show given rule set names (comma separated)

    --filter-rule-set <RULE_SET_FILTER>
