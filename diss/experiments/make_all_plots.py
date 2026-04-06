#!/usr/bin/env python3
"""
Generate all plots for all experiments.

Runs analysis scripts for:
  - linear (analyse.py)
  - nested (analyse.py)
  - scaling-models/nqueens (analyse.py, analyse_no_solver.py)
  - scaling-models/pythagorean-triples (analyse.py, analyse_no_solver.py)
  - scaling-models/bin_packing (analyse.py)
"""

import subprocess
import sys
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
SCALING_MODELS_DIR = SCRIPT_DIR / "scaling-models"

# List of (directory, script) pairs to run
ANALYSIS_SCRIPTS = [
    # Top-level experiments
    (SCRIPT_DIR / "linear", "analyse.py"),
    (SCRIPT_DIR / "nested", "analyse.py"),
    # Scaling models
    (SCALING_MODELS_DIR / "nqueens", "analyse.py"),
    (SCALING_MODELS_DIR / "nqueens", "analyse_no_solver.py"),
    (SCALING_MODELS_DIR / "pythagorean-triples", "analyse.py"),
    (SCALING_MODELS_DIR / "pythagorean-triples", "analyse_no_solver.py"),
    (SCALING_MODELS_DIR / "bin_packing", "analyse.py"),
]


def run_script(directory: Path, script_name: str) -> bool:
    """Run a Python script in the given directory. Returns True on success."""
    script_path = directory / script_name
    if not script_path.exists():
        print(f"  [SKIP] {script_path} does not exist")
        return True  # Not a failure, just skip

    print(f"  Running {script_path.relative_to(SCRIPT_DIR)}...")
    result = subprocess.run(
        [sys.executable, str(script_path)],
        cwd=str(directory),
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        print(f"  [FAIL] {script_name} failed:")
        if result.stderr:
            for line in result.stderr.strip().split("\n"):
                print(f"    {line}")
        return False

    # Print output (saved files, etc.)
    if result.stdout:
        for line in result.stdout.strip().split("\n"):
            print(f"    {line}")

    return True


def main():
    print("Generating all experiment plots...\n")

    failed = []
    for directory, script_name in ANALYSIS_SCRIPTS:
        if not run_script(directory, script_name):
            failed.append(f"{directory.name}/{script_name}")
        print()

    if failed:
        print(f"Failed scripts: {', '.join(failed)}")
        sys.exit(1)

    print("All plots generated successfully.")


if __name__ == "__main__":
    main()

