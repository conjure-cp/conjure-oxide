#!/usr/bin/env python3
"""Run all scaling-model experiments sequentially, forwarding CLI flags."""

import argparse
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
MODELS = sorted(d for d in HERE.iterdir() if d.is_dir() and (d / "run.py").exists())

# Define top-level flags for documentation / validation,
# then forward everything (including unknown args) to each run.py.
ap = argparse.ArgumentParser(
    description="Run all scaling-model experiments, forwarding flags to each run.py.",
)
ap.add_argument("--oxide-only", action="store_true")
ap.add_argument("--conjure-only", action="store_true")
ap.add_argument("--threads", type=int, default=None)
ap.add_argument("--runs", type=int, default=None)
ap.add_argument("--min-n", type=int, default=None,
                help="smallest N (forwarded to each experiment)")
ap.add_argument("--max-n", type=int, default=None,
                help="largest N (forwarded to each experiment)")
ap.add_argument("--no-solver", action="store_true",
                help="skip solver; record rewrite/translation time only")
ap.parse_known_args()  # validate known flags; ignore experiment-specific ones

# forward all flags verbatim
extra_args = sys.argv[1:]

failed = []
for model_dir in MODELS:
    name = model_dir.name
    print(f"\n{'=' * 60}")
    print(f"  {name}")
    print(f"{'=' * 60}\n")

    rc = subprocess.call(
        [sys.executable, "run.py", *extra_args],
        cwd=model_dir,
    )
    if rc != 0:
        failed.append(name)
        print(f"\n*** {name} exited with code {rc} ***\n", file=sys.stderr)

print(f"\n{'=' * 60}")
if failed:
    print(f"FAILED: {', '.join(failed)}", file=sys.stderr)
    sys.exit(1)
else:
    print("all experiments finished")
