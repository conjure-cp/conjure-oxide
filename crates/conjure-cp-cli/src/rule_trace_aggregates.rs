//! Writes the final in-memory rule counts to the CLI aggregate trace file.
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use anyhow::Context as _;

/// Write a snapshot of the counts, ordered by descending count and then rule name.
pub fn write_rule_trace_aggregates(
    path: &Path,
    counts: &BTreeMap<String, u64>,
) -> anyhow::Result<()> {
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

    fs::write(path, contents)
        .with_context(|| format!("Unable to write aggregate trace file {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_total_and_sorts_counts_with_alphabetical_ties() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("aggregates.txt");
        let counts = BTreeMap::from([
            ("alpha".to_owned(), 2),
            ("beta".to_owned(), 4),
            ("gamma".to_owned(), 2),
        ]);

        write_rule_trace_aggregates(&path, &counts).unwrap();

        assert_eq!(
            fs::read_to_string(path).unwrap(),
            "total_rule_applications: 8\n     4 beta\n     2 alpha\n     2 gamma\n",
        );
    }
}
