#!/usr/bin/env python3
"""
Scale the N-Queens model (board size N from MIN_N to MAX_N) and compare
conjure-oxide vs old Conjure. Writes results.csv for plotting.
"""

import csv
import json
import os
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
import importlib

sys.path.insert(0, str(Path(__file__).resolve().parent.parent.parent))
conjure_stats = importlib.import_module("conjure-stats")

# -- config ------------------------------------------------------------------

MIN_N = 4
MAX_N = 12
N_RUNS = 1
N_THREADS = 1

CONJURE_OXIDE_BIN: str | None = None
EXTRA_FLAGS: list[str] = []
OUTPUT_CSV = "results.csv"
RUN_CONJURE = True

# -- paths -------------------------------------------------------------------

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parents[3]
MODEL_PATH = SCRIPT_DIR / "input.essence"
PARAMS_DIR = SCRIPT_DIR / "params"


def find_conjure_oxide() -> str:
    if CONJURE_OXIDE_BIN is not None:
        return CONJURE_OXIDE_BIN
    candidate = REPO_ROOT / "target" / "release" / "conjure-oxide"
    if candidate.exists():
        return str(candidate)
    print("conjure-oxide not found, building...", file=sys.stderr)
    subprocess.check_call(
        ["cargo", "build", "--release", "-p", "conjure-cp-cli", "--bin", "conjure-oxide"],
        cwd=REPO_ROOT,
    )
    assert candidate.exists(), f"build succeeded but {candidate} not found"
    return str(candidate)


def duration_ms(d: dict | None) -> float:
    """Rust Duration {secs, nanos} -> milliseconds."""
    if d is None:
        return 0.0
    return d.get("secs", 0) * 1000.0 + d.get("nanos", 0) / 1e6


# -- param files -------------------------------------------------------------

def generate_params(min_n: int, max_n: int) -> list[tuple[int, Path]]:
    PARAMS_DIR.mkdir(exist_ok=True)
    jobs = []
    for n in range(min_n, max_n + 1):
        p = PARAMS_DIR / f"n_{n}.param"
        p.write_text(f"language ESSENCE' 1.0\n\nletting n be {n}\n")
        jobs.append((n, p))
    return jobs


# -- oxide runner ------------------------------------------------------------

def run_one_oxide(n: int, run: int, param_path: Path, oxide_bin: str,
                  no_solver: bool = False) -> dict:
    with tempfile.TemporaryDirectory() as tmpdir:
        stats_path = os.path.join(tmpdir, "stats.json")
        solutions_path = os.path.join(tmpdir, "solutions.json")

        cmd = [
            oxide_bin,
            "--parser", "tree-sitter",
            "--comprehension-expander", "via-solver-ac",
            "solve", str(MODEL_PATH), str(param_path),
            "-n", "all",
            "--info-json-path", stats_path,
            "-o", solutions_path,
            *EXTRA_FLAGS,
        ]
        if no_solver:
            cmd.append("--no-run-solver")
        print(f"[oxide N={n} run={run}] {' '.join(cmd)}", file=sys.stderr)
        proc = subprocess.run(cmd, capture_output=True, text=True)

        if proc.returncode != 0:
            print(f"[oxide N={n} run={run}] FAILED rc={proc.returncode}", file=sys.stderr)
            print(f"  stderr: {proc.stderr[:500]}", file=sys.stderr)
            return _error_row("oxide", n, run)

        with open(stats_path) as f:
            stats = json.load(f)

        s = stats.get("stats", {})
        rewrite_ms = sum(duration_ms(rw.get("rewriterRunTime"))
                         for rw in s.get("rewriterRuns", []))
        solver_ms = sum(sr.get("conjureSolverWallTime_s", 0.0) * 1000.0
                        for sr in s.get("solverRuns", []))
        translate_ms = duration_ms(s.get("solutionTranslationTime"))

        try:
            with open(solutions_path) as f:
                sols = json.load(f)
            num_solutions = len(sols) if isinstance(sols, list) else -1
        except (FileNotFoundError, json.JSONDecodeError):
            num_solutions = -1

        print(f"[oxide N={n} run={run}] {num_solutions} solutions", file=sys.stderr)
        return {
            "system": "oxide", "N": n, "run": run,
            "num_solutions": num_solutions,
            "rewrite_time_ms": round(rewrite_ms, 4),
            "solver_time_ms": round(solver_ms, 4),
            "solution_translation_time_ms": round(translate_ms, 4),
        }


# -- conjure runner ----------------------------------------------------------

def run_one_conjure(n: int, run: int, param_path: Path,
                    no_solver: bool = False) -> dict:
    print(f"[conjure N={n} run={run}] running...", file=sys.stderr)
    stats = conjure_stats.run_pipeline(
        str(MODEL_PATH), param_path=str(param_path),
        number_of_solutions="all", no_solver=no_solver, quiet=True,
    )
    if stats["status"] != "OK":
        print(f"[conjure N={n} run={run}] FAILED: {stats['status']}", file=sys.stderr)
    print(f"[conjure N={n} run={run}] {stats['num_solutions']} solutions", file=sys.stderr)
    return {
        "system": "conjure", "N": n, "run": run,
        "num_solutions": stats["num_solutions"],
        "rewrite_time_ms": stats["rewrite_time_ms"],
        "solver_time_ms": stats["solver_time_ms"],
        "solution_translation_time_ms": stats["solution_translation_time_ms"],
        "conjure_solve_e2e_ms": stats.get("conjure_solve_e2e_ms", -1),
    }


def _error_row(system: str, n: int, run: int) -> dict:
    return {"system": system, "N": n, "run": run, "num_solutions": -1,
            "rewrite_time_ms": -1, "solver_time_ms": -1,
            "solution_translation_time_ms": -1}


# -- main --------------------------------------------------------------------

def main() -> None:
    import argparse
    ap = argparse.ArgumentParser()
    ap.add_argument("--oxide-only", action="store_true")
    ap.add_argument("--conjure-only", action="store_true")
    ap.add_argument("--threads", type=int, default=N_THREADS)
    ap.add_argument("--runs", type=int, default=N_RUNS,
                    help="repeat each (system, N) pair this many times")
    ap.add_argument("--min-n", type=int, default=MIN_N,
                    help=f"smallest N to test (default {MIN_N})")
    ap.add_argument("--max-n", type=int, default=MAX_N,
                    help=f"largest N to test (default {MAX_N})")
    ap.add_argument("--no-solver", action="store_true",
                    help="skip the solver; record rewrite/translation time only")
    args = ap.parse_args()

    n_threads = args.threads
    n_runs = args.runs
    no_solver = args.no_solver
    do_oxide = not args.conjure_only
    do_conjure = RUN_CONJURE and not args.oxide_only

    oxide_bin = None
    if do_oxide:
        oxide_bin = find_conjure_oxide()
        print(f"Using: {oxide_bin}", file=sys.stderr)

    jobs = generate_params(args.min_n, args.max_n)
    print(f"{len(jobs)} param files, {n_runs} run(s) each, {n_threads} thread(s)",
          file=sys.stderr)

    results: list[dict] = []

    if do_oxide:
        with ThreadPoolExecutor(max_workers=n_threads) as pool:
            futs = {}
            for n, p in jobs:
                for run in range(1, n_runs + 1):
                    f = pool.submit(run_one_oxide, n, run, p, oxide_bin,
                                    no_solver=no_solver)
                    futs[f] = (n, run)
            for f in as_completed(futs):
                n, run = futs[f]
                try:
                    results.append(f.result())
                except Exception as exc:
                    print(f"[oxide N={n} run={run}] {exc!r}", file=sys.stderr)
                    results.append(_error_row("oxide", n, run))

    if do_conjure:
        with ThreadPoolExecutor(max_workers=n_threads) as pool:
            futs = {}
            for n, p in jobs:
                for run in range(1, n_runs + 1):
                    f = pool.submit(run_one_conjure, n, run, p,
                                    no_solver=no_solver)
                    futs[f] = (n, run)
            for f in as_completed(futs):
                n, run = futs[f]
                try:
                    results.append(f.result())
                except Exception as exc:
                    print(f"[conjure N={n} run={run}] {exc!r}", file=sys.stderr)
                    results.append(_error_row("conjure", n, run))

    results.sort(key=lambda r: (r["system"], r["N"], r["run"]))

    fieldnames = ["system", "N", "run", "num_solutions", "rewrite_time_ms",
                  "solver_time_ms", "solution_translation_time_ms",
                  "conjure_solve_e2e_ms"]
    csv_name = "results-no-solver.csv" if no_solver else OUTPUT_CSV
    out_path = SCRIPT_DIR / csv_name
    with open(out_path, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=fieldnames, extrasaction="ignore")
        w.writeheader()
        w.writerows(results)

    print(f"\nWrote {out_path}", file=sys.stderr)
    with open(out_path) as f:
        print(f.read())


if __name__ == "__main__":
    main()

