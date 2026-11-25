#!/usr/bin/env python3
"""Run nextest with priorities derived from expected-time (requires Python 3.11+)."""

from collections import defaultdict
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib


REPO_ROOT = Path(__file__).resolve().parent.parent


def priority_config(repo_root: Path) -> str:
    """Mirror build.rs discovery and assign higher priorities to longer tests."""
    tests_root = repo_root / "test-suite" / "tests"
    buckets = defaultdict(list)
    for suite, metadata in (("exhaustive", "stats.toml"), ("custom", "config.toml")):
        for path in sorted((tests_root / suite).rglob(metadata)):
            directory = path.parent
            if suite == "exhaustive":
                if len(list(directory.glob("*.essence"))) != 1:
                    continue
            elif not (directory / "run.sh").is_file():
                continue
            with path.open("rb") as source:
                expected_time = tomllib.load(source).get("expected-time")
            if expected_time is None:
                continue
            if type(expected_time) is not int or not 0 <= expected_time <= 2**64 - 1:
                raise ValueError(f"{path}: expected-time must be a non-negative u64 integer")
            name = directory.relative_to(tests_root.parent).as_posix()
            name = name.replace("./", "").replace("/", "_").replace("-", "_")
            buckets[expected_time].append(name)

    lines = [
        "# Generated from expected-time; do not edit.",
        'nextest-version = "0.9.91"',
        "",
    ]
    durations = sorted(buckets)
    for index, duration in enumerate(durations):
        # Nextest permits only -100..100. Preserve strict bucket order up to 100
        # distinct durations, then combine adjacent buckets monotonically.
        priority = 1 + index * 99 // max(1, len(durations) - 1)
        filters = " | ".join(f"test(={name})" for name in sorted(buckets[duration]))
        lines.extend([
            f"# expected-time = {duration} seconds",
            "[[profile.default.overrides]]",
            f"filter = {json.dumps(filters)}",
            f"priority = {priority}",
            "",
        ])
    return "\n".join(lines)


def main() -> int:
    args = sys.argv[1:]
    cargo = ["cargo"]
    if args and args[0].startswith("+"):
        cargo.append(args.pop(0))
    # A unique temporary config avoids stale priorities and concurrent-run races.
    with tempfile.TemporaryDirectory(prefix="conjure-nextest-") as temporary:
        config = Path(temporary) / "nextest.toml"
        config.write_text(priority_config(REPO_ROOT), encoding="utf-8")
        return subprocess.call(
            cargo + ["nextest", "run", "--tool-config-file", f"conjure-oxide:{config}"] + args,
            cwd=REPO_ROOT,
        )


if __name__ == "__main__":
    sys.exit(main())
