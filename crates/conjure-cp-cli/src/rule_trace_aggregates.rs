//! Counts rule applications in memory and writes the totals once, when the CLI exits.
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::Context as _;
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;

#[derive(Clone)]
pub struct RuleTraceAggregatesHandle {
    state: Arc<Mutex<RuleTraceAggregatesState>>,
}

pub struct RuleTraceAggregatesLayer {
    state: Arc<Mutex<RuleTraceAggregatesState>>,
}

struct RuleTraceAggregatesState {
    path: PathBuf,
    total_rule_applications: usize,
    counts: BTreeMap<String, usize>,
}

#[derive(Default)]
struct RuleNameVisitor {
    rule_name: Option<String>,
}

impl RuleTraceAggregatesHandle {
    /// Creates the output file now, so an unusable path fails before any rewriting.
    pub fn new(path: PathBuf) -> anyhow::Result<Self> {
        File::create(&path)
            .with_context(|| format!("Unable to create aggregate trace file {}", path.display()))?;

        Ok(Self {
            state: Arc::new(Mutex::new(RuleTraceAggregatesState {
                path,
                total_rule_applications: 0,
                counts: BTreeMap::new(),
            })),
        })
    }

    pub fn layer(&self) -> RuleTraceAggregatesLayer {
        RuleTraceAggregatesLayer {
            state: Arc::clone(&self.state),
        }
    }

    /// Writes the counts collected so far.
    pub fn flush(&self) {
        self.state
            .lock()
            .expect("rule trace aggregate state lock poisoned")
            .write()
            .expect("failed to write rule trace aggregates")
    }
}

impl<S> Layer<S> for RuleTraceAggregatesLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut visitor = RuleNameVisitor::default();
        event.record(&mut visitor);

        let Some(rule_name) = visitor.rule_name else {
            return;
        };

        let mut state = self
            .state
            .lock()
            .expect("rule trace aggregate state lock poisoned");
        state.total_rule_applications += 1;
        *state.counts.entry(rule_name).or_insert(0) += 1;
    }
}

impl RuleTraceAggregatesState {
    fn write(&self) -> anyhow::Result<()> {
        let mut rows: Vec<_> = self.counts.iter().collect();
        rows.sort_by(|(rule_name_a, count_a), (rule_name_b, count_b)| {
            count_b
                .cmp(count_a)
                .then_with(|| rule_name_a.cmp(rule_name_b))
        });

        let mut contents = format!(
            "total_rule_applications: {}\n",
            self.total_rule_applications
        );
        for (rule_name, count) in rows {
            writeln!(contents, "{count:6} {rule_name}")?;
        }

        fs::write(&self.path, contents).with_context(|| {
            format!(
                "Unable to write aggregate trace file {}",
                self.path.display()
            )
        })
    }
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
