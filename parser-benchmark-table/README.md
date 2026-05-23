# parser-benchmark-table

CLI tool to benchmark parser behavior across Essence inputs and generate an HTML report.

## What it does

- Discovers input groups from selected repositories.
- Runs one or both parser paths on each group:
  - native parser
  - via-conjure parser
- Produces an HTML table report with pass/fail results and per-test details.

## Scan sources

- `conjure-oxide`: scans `tests-integration/tests`
- `Conjure`: scans `parser-benchmark-table/.cache/repos/conjure`
- `EssenceCatalog`: scans `parser-benchmark-table/.cache/repos/EssenceCatalog`

If `Conjure` or `EssenceCatalog` are selected, their repositories are cloned/pulled into the cache automatically.

## Output

- By default, the HTML report is written to `parser-benchmark-table/parser_benchmark_table.html`
- Use `--output-html <path>` to write it somewhere else

To view the HTML output:

```bash
open parser-benchmark-table/parser_benchmark_table.html
```

## Usage

Run from the repository root:

```bash
cargo run -p parser-benchmark-table
```

For the full CLI and flag descriptions, run:

```bash
cargo run -p parser-benchmark-table -- --help
```
