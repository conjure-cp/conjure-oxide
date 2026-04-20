#!/usr/bin/env python3
"""
Benchmark deeply nested matrix and record models.

Scales nesting *depth* N (1..MAX_N) for both matrix and record types.

Matrix (N levels):
  N=1: find x: matrix indexed by [int(1..M)] of int(1..M)
  N=2: find x: matrix indexed by [int(1..M)] of matrix indexed by [int(1..M)] of int(1..M)

Record (N levels, M fields each):
  N=1, M=2: find x: record {d1_f1: int(1..2), d1_f2: int(1..2)}
  N=2, M=2: find x: record {d2_f1: record {d1_f1: int(1..2), ...}, d2_f2: ...}

Field names are prefixed with depth (d1_, d2_, ...) because old Conjure
treats record field names as globally scoped.
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

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
conjure_stats = importlib.import_module("conjure-stats")

# -- config ------------------------------------------------------------------

M = 2       # leaf domain int(1..M), also fields per record
MAX_N = 4   # max nesting depth

N_RUNS = 1
N_THREADS = 1
CONJURE_OXIDE_BIN: str | None = None
EXTRA_FLAGS: list[str] = []
OUTPUT_CSV = "results.csv"
RUN_CONJURE = True

# -- paths -------------------------------------------------------------------

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parents[2]


def find_conjure_oxide() -> str:
    if CONJURE_OXIDE_BIN is not None:
        return CONJURE_OXIDE_BIN
    candidate = REPO_ROOT / "target" / "release" / "conjure-oxide"
    print("candidate: ", candidate)
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


# -- model generators -------------------------------------------------------

def gen_matrix_model(n: int, m: int) -> str:
    """N levels of matrix nesting over int(1..m)."""
    inner = f"int(1..{m})"
    for _ in range(n):
        inner = f"matrix indexed by [int(1..{m})] of {inner}"
    return f"find x: {inner}\n"


def gen_record_model(n: int, m: int) -> str:
    """N levels of record nesting over int(1..m), M fields per level."""
    inner = f"int(1..{m})"
    for depth in range(1, n + 1):
        fields = ", ".join(f"d{depth}_f{i}: {inner}" for i in range(1, m + 1))
        inner = f"record {{{fields}}}"
    return f"find x: {inner}\n"


# -- oxide runner ------------------------------------------------------------

def run_one_oxide(kind: str, n: int, run: int, m: int, oxide_bin: str) -> dict:
    model_text = gen_matrix_model(n, m) if kind == "matrix" else gen_record_model(n, m)

    with tempfile.TemporaryDirectory() as tmpdir:
        model_path = os.path.join(tmpdir, "model.essence")
        stats_path = os.path.join(tmpdir, "stats.json")
        solutions_path = os.path.join(tmpdir, "solutions.json")

        with open(model_path, "w") as f:
            f.write(model_text)

        cmd = [
            oxide_bin,
            "--parser", "tree-sitter",
            "--comprehension-expander", "via-solver-ac",
            "solve", model_path,
            "-n", "all",
            "--info-json-path", stats_path,
            "-o", solutions_path,
            *EXTRA_FLAGS,
        ]
        print(f"[oxide {kind} N={n} run={run}] {' '.join(cmd)}", file=sys.stderr)
        proc = subprocess.run(cmd, capture_output=True, text=True)

        if proc.returncode != 0:
            print(f"[oxide {kind} N={n} run={run}] FAILED rc={proc.returncode}",
                  file=sys.stderr)
            print(f"  stderr: {proc.stderr[:500]}", file=sys.stderr)
            return _error_row("oxide", kind, n, run)

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

        print(f"[oxide {kind} N={n} run={run}] {num_solutions} solutions",
              file=sys.stderr)
        return {
            "system": "oxide", "kind": kind, "M": m,
            "N": n, "run": run,
            "num_solutions": num_solutions,
            "rewrite_time_ms": round(rewrite_ms, 4),
            "solver_time_ms": round(solver_ms, 4),
            "solution_translation_time_ms": round(translate_ms, 4),
        }


# -- conjure runner ----------------------------------------------------------

def run_one_conjure(kind: str, n: int, run: int, m: int) -> dict:
    model_text = gen_matrix_model(n, m) if kind == "matrix" else gen_record_model(n, m)

    with tempfile.TemporaryDirectory() as tmpdir:
        model_path = os.path.join(tmpdir, "model.essence")
        with open(model_path, "w") as f:
            f.write(model_text)

        print(f"[conjure {kind} N={n} run={run}] running...", file=sys.stderr)
        stats = conjure_stats.run_pipeline(
            model_path, number_of_solutions="all", quiet=True,
        )
        if stats["status"] != "OK":
            print(f"[conjure {kind} N={n} run={run}] FAILED: {stats['status']}",
                  file=sys.stderr)

        print(f"[conjure {kind} N={n} run={run}] {stats['num_solutions']} solutions",
              file=sys.stderr)
        return {
            "system": "conjure", "kind": kind, "M": m,
            "N": n, "run": run,
            "num_solutions": stats["num_solutions"],
            "rewrite_time_ms": stats["rewrite_time_ms"],
            "solver_time_ms": stats["solver_time_ms"],
            "solution_translation_time_ms": stats["solution_translation_time_ms"],
            "conjure_solve_e2e_ms": stats.get("conjure_solve_e2e_ms", -1),
        }


def _error_row(system: str, kind: str, n: int, run: int) -> dict:
    return {"system": system, "kind": kind, "M": M,
            "N": n, "run": run, "num_solutions": -1,
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
                    help="repeat each (system, kind, N) combo this many times")
    args = ap.parse_args()

    n_threads = args.threads
    n_runs = args.runs
    do_oxide = not args.conjure_only
    do_conjure = RUN_CONJURE and not args.oxide_only

    oxide_bin = None
    if do_oxide:
        oxide_bin = find_conjure_oxide()
        print(f"Using: {oxide_bin}", file=sys.stderr)

    jobs: list[tuple[str, int]] = []
    for n in range(1, MAX_N + 1):
        jobs.append(("matrix", n))
        jobs.append(("record", n))

    print(f"{len(jobs)} models, {n_runs} run(s) each, {n_threads} thread(s)",
          file=sys.stderr)

    results: list[dict] = []

    if do_oxide:
        with ThreadPoolExecutor(max_workers=n_threads) as pool:
            futs = {}
            for kind, n in jobs:
                for run in range(1, n_runs + 1):
                    f = pool.submit(run_one_oxide, kind, n, run, M, oxide_bin)
                    futs[f] = (kind, n, run)
            for f in as_completed(futs):
                kind, n, run = futs[f]
                try:
                    results.append(f.result())
                except Exception as exc:
                    print(f"[oxide {kind} N={n} run={run}] {exc!r}", file=sys.stderr)
                    results.append(_error_row("oxide", kind, n, run))

    if do_conjure:
        with ThreadPoolExecutor(max_workers=n_threads) as pool:
            futs = {}
            for kind, n in jobs:
                for run in range(1, n_runs + 1):
                    f = pool.submit(run_one_conjure, kind, n, run, M)
                    futs[f] = (kind, n, run)
            for f in as_completed(futs):
                kind, n, run = futs[f]
                try:
                    results.append(f.result())
                except Exception as exc:
                    print(f"[conjure {kind} N={n} run={run}] {exc!r}", file=sys.stderr)
                    results.append(_error_row("conjure", kind, n, run))

    results.sort(key=lambda r: (r["system"], r["kind"], r["N"], r["run"]))

    fieldnames = ["system", "kind", "M", "N", "run", "num_solutions",
                  "rewrite_time_ms", "solver_time_ms",
                  "solution_translation_time_ms", "conjure_solve_e2e_ms"]
    out_path = SCRIPT_DIR / OUTPUT_CSV
    with open(out_path, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=fieldnames, extrasaction="ignore")
        w.writeheader()
        w.writerows(results)

    print(f"\nWrote {out_path}", file=sys.stderr)
    with open(out_path) as f:
        print(f.read())


if __name__ == "__main__":
    main()

