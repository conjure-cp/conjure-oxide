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

- HTML report path: `parser-benchmark-table/parser_benchmark_table.html`

To view the HTML outpt:

```bash
open parser-benchmark-table/parser_benchmark_table.html
```

## Usage

Run from the repository root:

```bash
cargo run -p parser-benchmark-table
```

### Options

- `--parser native|via-conjure`
  - `native`: run only native parser
  - `via-conjure`: run only via-conjure parser
  - if omitted, both parsers run

- `--repos conjure-oxide,conjure,essencecatalog`
  - comma-separated list of repositories to include
  - if omitted, all three are included

