#!/usr/bin/env python3
"""
Run the old Conjure pipeline and extract per-phase timings.

Phase 1 - conjure modelling (Essence -> Essence')
  Measured via wall-clock time of `conjure modelling`.

Phase 2 - conjure solve end-to-end
  Parses stats.json for SavileRowTotalTime and SolverTotalTime
  (both reported in seconds as strings).

Derived timings (all in ms):
  rewrite_time = conjure_modelling_wall + SavileRowTotalTime
  solver_time  = SolverTotalTime
  solution_translation_time ~= e2e_wall - rewrite_time - solver_time

Usage:
    python3 conjure-stats.py MODEL.essence [--param PARAM.essence] [-n all|N]
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from glob import glob


# -- helpers -----------------------------------------------------------------


def _which(name: str) -> str:
    path = shutil.which(name)
    if path is None:
        print(f"ERROR: '{name}' not found on PATH", file=sys.stderr)
        sys.exit(1)
    return path


def _run(cmd: list[str], label: str, *, quiet: bool = False) -> dict:
    """Run cmd, return {cmd, wall_time_ms, returncode, stdout, stderr}."""
    cmd_str = " ".join(cmd)
    if not quiet:
        print(f"  [{label}] {cmd_str}", file=sys.stderr)

    t0 = time.monotonic()
    proc = subprocess.run(cmd, capture_output=True, text=True)
    wall_ms = (time.monotonic() - t0) * 1000.0

    if proc.returncode != 0 and not quiet:
        print(f"  [{label}] FAILED (rc={proc.returncode})", file=sys.stderr)
        print(f"    stdout: {proc.stdout[:400]}", file=sys.stderr)
        print(f"    stderr: {proc.stderr[:400]}", file=sys.stderr)

    return {
        "cmd": cmd_str,
        "label": label,
        "wall_time_ms": round(wall_ms, 4),
        "returncode": proc.returncode,
        "stdout": proc.stdout,
        "stderr": proc.stderr,
    }


# -- main pipeline -----------------------------------------------------------


def run_pipeline(
    model_path: str,
    *,
    param_path: str | None = None,
    number_of_solutions: str = "all",
    no_solver: bool = False,
    quiet: bool = False,
) -> dict:
    conjure = _which("conjure")
    steps: list[dict] = []

    with tempfile.TemporaryDirectory(prefix="conjure_stats_") as tmpdir:
        conjure_out = os.path.join(tmpdir, "conjure-out")

        # 1. conjure modelling (Essence -> Essence')
        # Run separately so we can measure this phase's wall-clock time,
        # since conjure's stats.json doesn't report it.
        cmd = [conjure, "modelling", "-o", conjure_out, model_path]
        step = _run(cmd, "conjure-modelling", quiet=quiet)
        steps.append(step)
        if step["returncode"] != 0:
            return _make_error_result(steps, "conjure modelling failed")

        conjure_modelling_ms = step["wall_time_ms"]

        # Find the eprime file produced by conjure modelling.
        eprime_files = glob(os.path.join(conjure_out, "*.eprime"))
        if not eprime_files:
            return _make_error_result(steps, "no .eprime from conjure modelling")
        eprime_path = eprime_files[0]

        if no_solver:
            # No-solver mode: run savilerow directly on the eprime
            # (Essence' -> Minion flat) without -run-solver.
            # This measures conjure modelling + savilerow translation only.
            savilerow_bin = _which("savilerow")
            sr_cmd = [savilerow_bin, eprime_path]
            if param_path:
                sr_cmd += ["-in-param", param_path]
            step = _run(sr_cmd, "savilerow", quiet=quiet)
            steps.append(step)
            if step["returncode"] != 0:
                return _make_error_result(steps, "savilerow failed")

            savilerow_ms = step["wall_time_ms"]
            rewrite_time_ms = conjure_modelling_ms + savilerow_ms

            return {
                "rewrite_time_ms": round(rewrite_time_ms, 4),
                "solver_time_ms": 0.0,
                "solution_translation_time_ms": 0.0,
                "num_solutions": 0,
                "conjure_solve_e2e_ms": 0.0,
                "status": "OK",
                "breakdown": {
                    "conjure_modelling_ms": round(conjure_modelling_ms, 4),
                    "savilerow_ms": round(savilerow_ms, 4),
                    "solver_ms": 0.0,
                },
                "steps": [
                    {
                        "label": s["label"],
                        "cmd": s["cmd"],
                        "wall_time_ms": s["wall_time_ms"],
                        "returncode": s["returncode"],
                    }
                    for s in steps
                ],
            }

        # 2. conjure solve end-to-end
        # We extract SavileRow and solver timings from stats.json, then
        # approximate solution-translation time by subtraction.
        conjure_solve_out = os.path.join(tmpdir, "conjure-solve-out")
        conjure_solve_cmd = [
            conjure,
            "solve",
            f"--number-of-solutions={number_of_solutions}",
            "--copy-solutions=no",
            "-o",
            conjure_solve_out,
            model_path,
        ]
        if param_path:
            conjure_solve_cmd.append(param_path)

        step = _run(conjure_solve_cmd, "conjure-solve-e2e", quiet=quiet)
        steps.append(step)
        conjure_solve_e2e_ms = step["wall_time_ms"]

        if step["returncode"] != 0:
            return _make_error_result(steps, "conjure solve failed")

        # 3. parse stats.json from the e2e run
        stats_files = glob(os.path.join(conjure_solve_out, "*.stats.json"))
        if not stats_files:
            return _make_error_result(steps, "no stats.json from conjure solve")

        with open(stats_files[0]) as f:
            e2e_stats = json.load(f)

        sr_info = e2e_stats.get("savilerowInfo", {})

        # SavileRow reports times in seconds (as strings)
        savilerow_total_s = float(sr_info.get("SavileRowTotalTime", 0))
        solver_total_s = float(sr_info.get("SolverTotalTime", 0))

        savilerow_ms = savilerow_total_s * 1000.0
        solver_ms = solver_total_s * 1000.0

        num_solutions = int(sr_info.get("SolverSolutionsFound", 0))
        if num_solutions == 0:
            num_solutions = len(glob(os.path.join(conjure_solve_out, "*.solution")))

        # aggregate timings
        rewrite_time_ms = conjure_modelling_ms + savilerow_ms
        solver_time_ms = solver_ms

        # solution translation ~= e2e wall - modelling - savilerow - solver
        # (modelling time comes from a separate run so this is approximate)
        solution_translation_time_ms = max(
            0.0, conjure_solve_e2e_ms - rewrite_time_ms - solver_time_ms
        )

        return {
            "rewrite_time_ms": round(rewrite_time_ms, 4),
            "solver_time_ms": round(solver_time_ms, 4),
            "solution_translation_time_ms": round(solution_translation_time_ms, 4),
            "num_solutions": num_solutions,
            "conjure_solve_e2e_ms": round(conjure_solve_e2e_ms, 4),
            "status": "OK",
            "breakdown": {
                "conjure_modelling_ms": round(conjure_modelling_ms, 4),
                "savilerow_ms": round(savilerow_ms, 4),
                "solver_ms": round(solver_ms, 4),
            },
            "steps": [
                {
                    "label": s["label"],
                    "cmd": s["cmd"],
                    "wall_time_ms": s["wall_time_ms"],
                    "returncode": s["returncode"],
                }
                for s in steps
            ],
        }


def _make_error_result(steps: list[dict], msg: str) -> dict:
    return {
        "rewrite_time_ms": -1,
        "solver_time_ms": -1,
        "solution_translation_time_ms": -1,
        "num_solutions": -1,
        "conjure_solve_e2e_ms": -1,
        "status": msg,
        "breakdown": {},
        "steps": [
            {
                "label": s["label"],
                "cmd": s["cmd"],
                "wall_time_ms": s["wall_time_ms"],
                "returncode": s["returncode"],
            }
            for s in steps
        ],
    }


# -- CLI ---------------------------------------------------------------------


def main():
    parser = argparse.ArgumentParser(
        description="Run old Conjure pipeline and report timing stats as JSON."
    )
    parser.add_argument("model", help="Path to the Essence model file")
    parser.add_argument("--param", help="Path to a parameter file", default=None)
    parser.add_argument(
        "-n",
        "--number-of-solutions",
        default="all",
        help="Number of solutions (default: all)",
    )
    parser.add_argument(
        "--no-solver",
        action="store_true",
        help="Run modelling + SavileRow translation only (no solver).",
    )

    args = parser.parse_args()

    result = run_pipeline(
        args.model,
        param_path=args.param,
        number_of_solutions=args.number_of_solutions,
        no_solver=args.no_solver,
    )

    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
