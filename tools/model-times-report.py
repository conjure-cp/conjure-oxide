#!/usr/bin/env python3
"""Export accepted per-model integration timings as CSV, JSON and Markdown."""

from __future__ import annotations

import argparse
import csv
import json
import math
import sys
import tomllib
from pathlib import Path
from typing import Any


CONFIG_FIELDS = (
    "parser", "rewriter", "comprehension-expander", "heuristic", "channelling",
    "seed", "solver-seed", "solver",
)
TIME_FIELDS = (
    "translation-time", "backend-time", "search-time",
    "solution-processing-time", "solve-time",
)
ROOT = Path(__file__).resolve().parent.parent
INTEGRATION = ROOT / "test-suite/tests/integration"


def config_key(row: dict[str, Any]) -> tuple[Any, ...]:
    return tuple(row.get(key, 0 if key in ("seed", "solver-seed") else "")
                 for key in CONFIG_FIELDS)


def validate(stats: dict[str, Any], fixture: str) -> None:
    """Check recorded measurements without rejecting partial or failed runs."""
    models = stats.get("model-runs", [])
    for row in models:
        label = f"{fixture}: {row['solver']} model {row['model-index']}"
        if len(row.get("choice-path", [])) != len(row.get("choices", [])):
            raise ValueError(f"{label}: choice indices and labels differ in length")
        for key in TIME_FIELDS:
            if key in row and (not math.isfinite(row[key]) or row[key] < 0):
                raise ValueError(f"{label}: invalid {key}")
        parts = TIME_FIELDS[1:4]
        if "solve-time" in row and sum(row.get(key, 0) for key in parts) > row["solve-time"] + 1e-6:
            raise ValueError(f"{label}: collection phases exceed solve-time")
    for run in stats.get("runs", []):
        matches = [row for row in models if config_key(row) == config_key(run)]
        if not matches:
            continue
        for key in ("translation-time", "solve-time"):
            # Allow the rounding used by the canonical stats writer.
            tolerance = 1e-6 * (len(matches) + 1)
            if key in run and abs(sum(row.get(key, 0) for row in matches) - run[key]) > tolerance:
                raise ValueError(f"{fixture}: {config_key(run)}: model totals differ for {key}")


def markdown(rows: list[dict[str, Any]], top: int) -> str:
    def cell(value: Any) -> str:
        return str(value).replace("|", "\\|").replace("\n", " ")

    def table_row(values: list[Any]) -> str:
        return "| " + " | ".join(cell(value) for value in values) + " |"

    lines = [
        "# Integration model timings", "",
        "Wall-clock seconds from accepted stats. Each model measures its whole combination "
        "of choices. Search includes callbacks and solver-time rewrites. Missing measurements "
        "are shown as '-' rather than zero.", "",
        "| Fixture | Solver / heuristic | Parser / rewriter / expander | Channelling | "
        "Seeds (model / solver) | Models | Translation | Backend | Search | Processing | Collection total |",
        "|---|---|---|---|---|---:|---:|---:|---:|---:|---:|",
    ]
    groups: dict[tuple[Any, ...], list[dict[str, Any]]] = {}
    for row in rows:
        groups.setdefault((row["fixture"], *config_key(row)), []).append(row)
    for group in groups.values():
        row = group[0]
        totals = [f"{sum(item.get(key, 0) for item in group):.3f}"
                  if any(key in item for item in group) else "-" for key in TIME_FIELDS]
        lines.append(table_row([
            row["fixture"], f"{row['solver']} / {row['heuristic']}",
            " / ".join(str(row.get(key, "")) for key in CONFIG_FIELDS[:3]),
            row.get("channelling", ""), f"{row.get('seed', 0)} / {row.get('solver-seed', 0)}",
            len(group), *totals,
        ]))
    lines += [
        "", "## Slowest individual compiled models", "",
        "| Fixture / model | Solver | Status | Translation | Backend | Search | Representations | SAT families |",
        "|---|---|---|---:|---:|---:|---|---|",
    ]
    for row in sorted(rows, key=lambda item: item.get("translation-time", 0)
                      + item.get("solve-time", 0), reverse=True)[:top]:
        options = "; ".join(key.removeprefix("sat-encoding-") + ": " + ", ".join(value)
                            for key, value in row.items() if key.startswith("sat-encoding-") and value)
        times = [f"{row[key]:.3f}" if key in row else "-"
                 for key in ("translation-time", "backend-time", "search-time")]
        lines.append(table_row([
            f"{row['fixture']} / {row['model-index']}", row["solver"], row.get("status", ""),
            *times, ", ".join(row.get("representations", [])), options,
        ]))
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("fixtures", nargs="*", help="Fixture paths relative to test-suite/tests/integration; "
                        "also accepts directory or stats.toml paths relative to the current directory")
    parser.add_argument("--output-dir", type=Path, default=ROOT / "target/model-timings",
                        help="Output directory (default: target/model-timings in the repository)")
    parser.add_argument("--top", type=int, default=20, help="Slowest models shown in Markdown (default: 20)")
    args = parser.parse_args()
    if args.top < 0:
        parser.error("--top must be non-negative")
    try:
        if args.fixtures:
            paths = []
            for fixture in args.fixtures:
                path = Path(fixture)
                if not path.exists():
                    path = INTEGRATION / fixture
                paths.append(path / "stats.toml" if path.is_dir() else path)
        else:
            paths = list(INTEGRATION.rglob("stats.toml"))
        rows = []
        skipped = 0
        fixture_count = 0
        for path in sorted({path.resolve() for path in paths}):
            fixture = path.parent.relative_to(INTEGRATION).as_posix()
            stats = tomllib.loads(path.read_text())
            models = stats.get("model-runs", [])
            if not models:
                if args.fixtures:
                    raise ValueError(f"{fixture}: no model-runs; record with ACCEPT=true first")
                skipped += 1
                continue
            validate(stats, fixture)
            rows.extend({"fixture": fixture, **row} for row in models)
            fixture_count += 1
        if not rows:
            raise ValueError("No model timing rows found; record with ACCEPT=true first")
        output = args.output_dir
        output.mkdir(parents=True, exist_ok=True)
        columns = list(dict.fromkeys(key for row in rows for key in row))
        with (output / "model-runs.csv").open("w", newline="") as handle:
            writer = csv.DictWriter(handle, fieldnames=columns)
            writer.writeheader()
            for row in rows:
                writer.writerow({key: json.dumps(value) if isinstance(value, list) else value
                                 for key, value in row.items()})
        (output / "model-runs.json").write_text(json.dumps(rows, indent=2) + "\n")
        (output / "model-runs.md").write_text(markdown(rows, args.top))
        print(f"Wrote {len(rows)} model rows across {fixture_count} fixtures to {output}; "
              f"skipped {skipped} fixtures without model-runs")
        return 0
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
