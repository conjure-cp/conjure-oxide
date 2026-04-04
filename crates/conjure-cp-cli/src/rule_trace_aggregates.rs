use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Write as _;
use std::path::{Path, PathBuf};
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
    tmp_path: PathBuf,
    total_rule_applications: usize,
    counts: BTreeMap<String, usize>,
}

#[derive(Default)]
struct RuleNameVisitor {
    rule_name: Option<String>,
}

impl RuleTraceAggregatesHandle {
    pub fn new(path: PathBuf) -> anyhow::Result<Self> {
        let state = RuleTraceAggregatesState::new(path);
        state.write_snapshot()?;

        Ok(Self {
            state: Arc::new(Mutex::new(state)),
        })
    }

    pub fn layer(&self) -> RuleTraceAggregatesLayer {
        RuleTraceAggregatesLayer {
            state: Arc::clone(&self.state),
        }
    }

    pub fn flush(&self) -> anyhow::Result<()> {
        self.state
            .lock()
            .expect("rule trace aggregate state lock poisoned")
            .write_snapshot()
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

        if let Ok(mut state) = self.state.lock() {
            let _ = state.record_rule(rule_name);
        }
    }
}

impl RuleTraceAggregatesState {
    fn new(path: PathBuf) -> Self {
        Self {
            tmp_path: temporary_output_path(&path),
            path,
            total_rule_applications: 0,
            counts: BTreeMap::new(),
        }
    }

    fn record_rule(&mut self, rule_name: String) -> anyhow::Result<()> {
        self.total_rule_applications += 1;
        *self.counts.entry(rule_name).or_insert(0) += 1;
        self.write_snapshot()
    }

    fn write_snapshot(&self) -> anyhow::Result<()> {
        let mut rows: Vec<_> = self.counts.iter().collect();
        rows.sort_by(|(rule_name_a, count_a), (rule_name_b, count_b)| {
            count_b.cmp(count_a).then_with(|| rule_name_a.cmp(rule_name_b))
        });

        let mut file = File::create(&self.tmp_path).with_context(|| {
            format!(
                "Unable to create temporary aggregate trace file {}",
                self.tmp_path.display()
            )
        })?;

        writeln!(
            file,
            "total_rule_applications: {}",
            self.total_rule_applications
        )?;
        for (rule_name, count) in rows {
            writeln!(file, "{count:6} {rule_name}")?;
        }
        file.flush()?;

        fs::rename(&self.tmp_path, &self.path).with_context(|| {
            format!(
                "Unable to move aggregate trace file into place at {}",
                self.path.display()
            )
        })?;

        Ok(())
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

fn temporary_output_path(path: &Path) -> PathBuf {
    let filename = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "rule-trace-aggregates".to_owned());

    path.with_file_name(format!(".{filename}.tmp"))
}
