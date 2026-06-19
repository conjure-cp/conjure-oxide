#!/usr/bin/env python3
"""Write a CSV comparing accepted timing stats against the Git baseline."""

from __future__ import annotations

import argparse
import csv
import math
import subprocess
import sys
import tomllib
from pathlib import Path
from typing import Any


CONFIG_PATHSPEC = ":(glob)test-suite/tests/**/config.toml"

FIELDNAMES = [
    "test",
    "old_status",
    "new_status",
    "old_conjure_status",
    "new_conjure_status",
    "old_oxide_status",
    "new_oxide_status",
    "old_conjure_translation_time_s",
    "new_conjure_translation_time_s",
    "old_conjure_solve_time_s",
    "new_conjure_solve_time_s",
    "old_conjure_total_time_s",
    "new_conjure_total_time_s",
    "old_oxide_translation_time_s",
    "new_oxide_translation_time_s",
    "old_oxide_solve_time_s",
    "new_oxide_solve_time_s",
    "old_oxide_total_time_s",
    "new_oxide_total_time_s",
    "old_vs_new_conjure_total_speedup",
    "old_vs_new_oxide_total_speedup",
    "new_conjure_vs_new_oxide_total_speedup",
    "summary",
]


def main() -> int:
    args = parse_args()
    root = git_root()
    output = Path(args.output)
    if not output.is_absolute():
        output = root / output

    rows = [
        build_row(root, args.base, path)
        for path in changed_config_paths(root, args.base)
    ]

    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=FIELDNAMES)
        writer.writeheader()
        writer.writerows(rows)

    print(f"Wrote {len(rows)} timing comparison rows to {output}")
    return 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Compare changed test config.toml timing stats against a Git baseline "
            "and write a CSV report."
        )
    )
    parser.add_argument(
        "--base",
        default="HEAD",
        help="Git revision to compare the working tree against (default: HEAD).",
    )
    parser.add_argument(
        "--output",
        default="target/accept-times-diff.csv",
        help="CSV output path (default: target/accept-times-diff.csv).",
    )
    return parser.parse_args()


def git_root() -> Path:
    return Path(run_git(None, ["rev-parse", "--show-toplevel"]).strip())


def changed_config_paths(root: Path, base: str) -> list[str]:
    diff_output = run_git(
        root,
        [
            "diff",
            "--name-only",
            "--diff-filter=ACMR",
            base,
            "--",
            CONFIG_PATHSPEC,
        ],
    )
    paths = {line.strip() for line in diff_output.splitlines() if line.strip()}

    untracked_output = run_git(
        root,
        ["ls-files", "--others", "--exclude-standard", "--", "test-suite/tests"],
    )
    paths.update(
        line.strip()
        for line in untracked_output.splitlines()
        if line.strip().endswith("/config.toml")
    )

    return sorted(paths)


def build_row(root: Path, base: str, path: str) -> dict[str, str]:
    old_config = read_git_config(root, base, path)
    new_config = read_worktree_config(root / path)

    old_conjure = tool_stats(old_config, "conjure")
    new_conjure = tool_stats(new_config, "conjure")
    old_oxide = tool_stats(old_config, "oxide")
    new_oxide = tool_stats(new_config, "oxide")

    old_conjure_total = total_time(old_conjure)
    new_conjure_total = total_time(new_conjure)
    old_oxide_total = total_time(old_oxide)
    new_oxide_total = total_time(new_oxide)

    old_status = status_from_config(old_config)
    new_status = status_from_config(new_config)
    old_conjure_status = status_from_tool(old_conjure)
    new_conjure_status = status_from_tool(new_conjure)
    old_oxide_status = status_from_tool(old_oxide)
    new_oxide_status = status_from_tool(new_oxide)
    old_effective_status = effective_status(
        old_status, old_oxide_status, old_conjure_status
    )
    new_effective_status = effective_status(
        new_status, new_oxide_status, new_conjure_status
    )

    return {
        "test": path.removesuffix("/config.toml"),
        "old_status": value(old_status),
        "new_status": value(new_status),
        "old_conjure_status": value(old_conjure_status),
        "new_conjure_status": value(new_conjure_status),
        "old_oxide_status": value(old_oxide_status),
        "new_oxide_status": value(new_oxide_status),
        "old_conjure_translation_time_s": decimal(old_conjure["translation-time"]),
        "new_conjure_translation_time_s": decimal(new_conjure["translation-time"]),
        "old_conjure_solve_time_s": decimal(old_conjure["solve-time"]),
        "new_conjure_solve_time_s": decimal(new_conjure["solve-time"]),
        "old_conjure_total_time_s": decimal(old_conjure_total),
        "new_conjure_total_time_s": decimal(new_conjure_total),
        "old_oxide_translation_time_s": decimal(old_oxide["translation-time"]),
        "new_oxide_translation_time_s": decimal(new_oxide["translation-time"]),
        "old_oxide_solve_time_s": decimal(old_oxide["solve-time"]),
        "new_oxide_solve_time_s": decimal(new_oxide["solve-time"]),
        "old_oxide_total_time_s": decimal(old_oxide_total),
        "new_oxide_total_time_s": decimal(new_oxide_total),
        "old_vs_new_conjure_total_speedup": speedup_when_ok(
            old_conjure_total,
            new_conjure_total,
            old_effective_status,
            new_effective_status,
            old_conjure_status,
            new_conjure_status,
        ),
        "old_vs_new_oxide_total_speedup": speedup_when_ok(
            old_oxide_total,
            new_oxide_total,
            old_effective_status,
            new_effective_status,
            old_oxide_status,
            new_oxide_status,
        ),
        "new_conjure_vs_new_oxide_total_speedup": speedup_when_ok(
            new_conjure_total,
            new_oxide_total,
            new_effective_status,
            new_effective_status,
            new_conjure_status,
            new_oxide_status,
        ),
        "summary": summarize(
            old_status=old_status,
            new_status=new_status,
            old_conjure_status=old_conjure_status,
            new_conjure_status=new_conjure_status,
            old_oxide_status=old_oxide_status,
            new_oxide_status=new_oxide_status,
            old_oxide_total=old_oxide_total,
            new_oxide_total=new_oxide_total,
            old_conjure_total=old_conjure_total,
            new_conjure_total=new_conjure_total,
        ),
    }


def read_git_config(root: Path, base: str, path: str) -> dict[str, Any]:
    result = subprocess.run(
        ["git", "show", f"{base}:{path}"],
        cwd=root,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        return {}
    return parse_toml(result.stdout, f"{base}:{path}")


def read_worktree_config(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {}
    return parse_toml(path.read_text(), str(path))


def parse_toml(contents: str, source: str) -> dict[str, Any]:
    if not contents.strip():
        return {}
    try:
        data = tomllib.loads(contents)
    except tomllib.TOMLDecodeError as err:
        raise SystemExit(f"Could not parse {source}: {err}") from err
    if not isinstance(data, dict):
        return {}
    return data


def tool_stats(config: dict[str, Any], tool: str) -> dict[str, float | str | None]:
    stats = config.get("stats")
    if not isinstance(stats, dict):
        stats = {}
    table = stats.get(tool)
    if not isinstance(table, dict):
        table = {}

    return {
        "status": string_value(table.get("status")),
        "translation-time": numeric_value(table.get("translation-time")),
        "solve-time": numeric_value(table.get("solve-time")),
    }


def status_from_config(config: dict[str, Any]) -> str | None:
    return string_value(config.get("status"))


def status_from_tool(stats: dict[str, float | str | None]) -> str | None:
    status = stats["status"]
    return status if isinstance(status, str) else None


def total_time(stats: dict[str, float | str | None]) -> float | None:
    translation_time = stats["translation-time"]
    solve_time = stats["solve-time"]
    if isinstance(translation_time, float) and isinstance(solve_time, float):
        return translation_time + solve_time
    return None


def summarize(
    *,
    old_status: str | None,
    new_status: str | None,
    old_conjure_status: str | None,
    new_conjure_status: str | None,
    old_oxide_status: str | None,
    new_oxide_status: str | None,
    old_oxide_total: float | None,
    new_oxide_total: float | None,
    old_conjure_total: float | None,
    new_conjure_total: float | None,
) -> str:
    old_effective = effective_status(old_status, old_oxide_status, old_conjure_status)
    new_effective = effective_status(new_status, new_oxide_status, new_conjure_status)
    old_kind = status_kind(old_effective)
    new_kind = status_kind(new_effective)

    if old_effective is None and new_effective is not None:
        return "new test"
    if old_kind == "timeout" and new_kind == "timeout":
        return "still timeout"
    if old_kind == "fail" and new_kind == "fail":
        return "still fail"
    if new_kind == "timeout":
        return "new timeout"
    if old_kind == "timeout":
        return "no longer timeout"
    if new_kind == "fail":
        return "new fail"
    if old_kind == "fail":
        return "fixed"
    if old_kind != new_kind:
        return "status changed"

    old_total = old_oxide_total
    new_total = new_oxide_total
    if old_total is None or new_total is None:
        old_total = old_conjure_total
        new_total = new_conjure_total

    if old_total is None or new_total is None:
        return "stayed same"
    if math.isclose(old_total, new_total, rel_tol=1e-9, abs_tol=1e-12):
        return "stayed same"
    if new_total < old_total:
        return "got faster"
    return "got slower"


def status_kind(status: str | None) -> str | None:
    if status is None:
        return None
    status = status.lower()
    if status.startswith("timeout"):
        return "timeout"
    if status == "fail":
        return "fail"
    if status == "ok":
        return "ok"
    return status


def effective_status(
    test_status: str | None, oxide_status: str | None, conjure_status: str | None
) -> str | None:
    return test_status or oxide_status or conjure_status


def numeric_value(raw: Any) -> float | None:
    if isinstance(raw, bool):
        return None
    if isinstance(raw, int | float):
        return float(raw)
    return None


def string_value(raw: Any) -> str | None:
    if isinstance(raw, str):
        return raw
    return None


def speedup(baseline: float | None, comparison: float | None) -> str:
    if baseline is None or comparison is None or comparison == 0:
        return ""
    return decimal(baseline / comparison)


def speedup_when_ok(
    baseline: float | None,
    comparison: float | None,
    baseline_overall_status: str | None,
    comparison_overall_status: str | None,
    baseline_tool_status: str | None,
    comparison_tool_status: str | None,
) -> str:
    statuses = [
        baseline_overall_status,
        comparison_overall_status,
        baseline_tool_status,
        comparison_tool_status,
    ]
    if any(status_kind(status) != "ok" for status in statuses):
        return ""
    return speedup(baseline, comparison)


def decimal(raw: float | str | None) -> str:
    if raw is None:
        return ""
    if isinstance(raw, str):
        return raw
    return f"{raw:.12g}"


def value(raw: str | None) -> str:
    return raw or ""


def run_git(root: Path | None, args: list[str]) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=root,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        command = " ".join(["git", *args])
        raise SystemExit(f"{command} failed:\n{result.stderr.strip()}")
    return result.stdout


if __name__ == "__main__":
    sys.exit(main())
