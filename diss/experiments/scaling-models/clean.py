#!/usr/bin/env python3
"""Remove generated params, results, and caches from all scaling-model experiments."""

import shutil
from pathlib import Path

HERE = Path(__file__).resolve().parent
MODELS = [d for d in HERE.iterdir() if d.is_dir() and (d / "run.py").exists()]

for model_dir in sorted(MODELS):
    name = model_dir.name
    for target in ["params", "__pycache__", "results.csv", "results-no-solver.csv"]:
        p = model_dir / target
        if p.is_dir():
            shutil.rmtree(p)
            print(f"  rm -r {name}/{target}")
        elif p.is_file():
            p.unlink()
            print(f"  rm    {name}/{target}")

print("done")

