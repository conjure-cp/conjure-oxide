//! Counts rule applications in memory and writes the totals once, when the CLI exits.
use std::fmt::Write as _;
use std::fs::{self, File};
use std::path::PathBuf;

use anyhow::Context as _;
use conjure_cp_cli::utils::rule_trace::RuleApplicationCounter;

/// Owns the CLI output path and shared rule application counter.
#[derive(Clone)]
pub struct RuleTraceAggregatesHandle {
    path: PathBuf,
    counter: RuleApplicationCounter,
}

impl RuleTraceAggregatesHandle {
    /// Create the output file before rewriting, so an unusable path fails early.
    pub fn new(path: PathBuf) -> anyhow::Result<Self> {
        File::create(&path)
            .with_context(|| format!("Unable to create aggregate trace file {}", path.display()))?;
        Ok(Self {
            path,
            counter: RuleApplicationCounter::default(),
        })
    }

    /// A layer sharing the counts written by this handle.
    pub fn layer(&self) -> RuleApplicationCounter {
        self.counter.clone()
    }

    /// Write the counts collected so far.
    pub fn flush(&self) {
        self.write().expect("failed to write rule trace aggregates")
    }
}

impl RuleTraceAggregatesHandle {
    fn write(&self) -> anyhow::Result<()> {
        let counts = self.counter.snapshot();
        let mut rows: Vec<_> = counts.iter().collect();
        rows.sort_by(|(rule_name_a, count_a), (rule_name_b, count_b)| {
            count_b
                .cmp(count_a)
                .then_with(|| rule_name_a.cmp(rule_name_b))
        });

        let mut contents = format!(
            "total_rule_applications: {}\n",
            counts.values().sum::<u64>()
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
