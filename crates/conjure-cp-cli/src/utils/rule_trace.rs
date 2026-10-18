//! Shared in-memory rule application counts for CLI and integration traces.
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::{Layer, layer::Context};

/// A clonable trace layer whose counts can be read without file I/O.
#[derive(Clone, Default)]
pub struct RuleApplicationCounter {
    counts: Arc<Mutex<BTreeMap<String, u64>>>,
}

impl RuleApplicationCounter {
    /// Snapshot the counts collected by every clone of this layer.
    pub fn snapshot(&self) -> BTreeMap<String, u64> {
        self.counts
            .lock()
            .expect("rule trace aggregate state lock poisoned")
            .clone()
    }
}

impl<S: Subscriber> Layer<S> for RuleApplicationCounter {
    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        let mut visitor = RuleNameVisitor::default();
        event.record(&mut visitor);
        if let Some(rule) = visitor.rule_name {
            *self
                .counts
                .lock()
                .expect("rule trace aggregate state lock poisoned")
                .entry(rule)
                .or_default() += 1;
        }
    }
}

#[derive(Default)]
struct RuleNameVisitor {
    rule_name: Option<String>,
}

impl Visit for RuleNameVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "rule_name" {
            self.rule_name = Some(value.to_owned());
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "rule_name" && self.rule_name.is_none() {
            self.rule_name = Some(format!("{value:?}").trim_matches('"').to_owned());
        }
    }
}
