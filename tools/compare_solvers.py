#!/usr/bin/env python3
"""
Compare translation and solving performance between Minion and OR-Tools CP-SAT
from conjure-oxide integration test stats.json files.
"""

from __future__ import annotations

import argparse
import csv
import glob
import json
import math
import os
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Compare Minion and OR-Tools CP-SAT performance from integration test stats."
    )
    parser.add_argument(
        "--tests-dir",
        type=str,
        default=None,
        help="Path to test-suite/tests directory. If omitted, auto-detected from current repo or WSL.",
    )
    parser.add_argument(
        "--output-csv",
        type=str,
        default="target/solver-comparison.csv",
        help="Path to write the CSV report (default: target/solver-comparison.csv).",
    )
    parser.add_argument(
        "--output-md",
        type=str,
        default="target/solver-comparison.md",
        help="Path to write the Markdown report (default: target/solver-comparison.md).",
    )
    parser.add_argument(
        "--top-n",
        type=int,
        default=30,
        help="Number of rows to show in the terminal preview table (default: 30).",
    )
    parser.add_argument(
        "--category",
        type=str,
        default=None,
        help="Filter results by category substring (e.g. 'savilerow', 'smt', 'basic').",
    )
    parser.add_argument(
        "--sort-by",
        choices=["ortools-solve", "minion-solve", "solve-speedup", "ortools-trans", "max-solve", "name"],
        default="max-solve",
        help="Metric to sort the table by (default: 'max-solve').",
    )
    return parser.parse_args()


def find_tests_dir(user_path: Optional[str]) -> Path:
    if user_path:
        p = Path(user_path)
        if p.exists() and p.is_dir():
            return p.resolve()
        raise FileNotFoundError(f"Specified tests directory not found: {user_path}")

    # Check common WSL repo path if running under Linux
    wsl_candidate = Path("/home/matteo/conjure-oxide/test-suite/tests")
    if wsl_candidate.exists() and wsl_candidate.is_dir():
        return wsl_candidate.resolve()

    # Check UNC path from Windows accessing WSL Ubuntu
    unc_wsl_candidates = [
        Path(r"\\wsl.localhost\Ubuntu\home\matteo\conjure-oxide\test-suite\tests"),
        Path(r"\\wsl$\Ubuntu\home\matteo\conjure-oxide\test-suite\tests"),
    ]
    for unc in unc_wsl_candidates:
        if unc.exists() and unc.is_dir():
            return unc

    # Check local relative repo path
    local_candidates = [
        Path("test-suite/tests"),
        Path("../test-suite/tests"),
        Path("../../test-suite/tests"),
    ]
    for c in local_candidates:
        if c.exists() and c.is_dir():
            return c.resolve()

    raise FileNotFoundError(
        "Could not auto-detect test-suite/tests. Please specify via --tests-dir."
    )


def extract_stats(filepath: str) -> Dict[str, Any]:
    with open(filepath, "r", encoding="utf-8") as f:
        data = json.load(f)

    stats = data.get("stats", {})

    # Translation time (rewriter runtime sum)
    rewriter_runs = stats.get("rewriterRuns", [])
    trans_time_s = sum(
        run.get("rewriterRunTime", {}).get("secs", 0)
        + run.get("rewriterRunTime", {}).get("nanos", 0) / 1e9
        for run in rewriter_runs
    )
    rule_attempts = sum(
        run.get("rewriterRuleApplicationAttempts", 0) for run in rewriter_runs
    )

    # Solver time
    solver_runs = stats.get("solverRuns", [])
    solve_wall_time_s = sum(
        run.get("conjureSolverWallTime_s") or 0.0 for run in solver_runs
    )
    solve_internal_time_s = sum(
        run.get("solverTime_s") or 0.0 for run in solver_runs
    )
    satisfiable = any(run.get("satisfiable", False) for run in solver_runs)
    nodes = sum(run.get("nodes") or 0 for run in solver_runs)

    return {
        "trans_time_s": trans_time_s,
        "rule_attempts": rule_attempts,
        "solve_wall_time_s": solve_wall_time_s,
        "solve_internal_time_s": solve_internal_time_s,
        "satisfiable": satisfiable,
        "nodes": nodes,
    }


def main() -> int:
    args = parse_args()
    tests_dir = find_tests_dir(args.tests_dir)
    print(f"Scanning stats files in: {tests_dir}")

    # Search for ortools-cpsat stats
    pattern = str(tests_dir / "**" / "*-ortools-cpsat-stats.json")
    ortools_files = glob.glob(pattern, recursive=True)

    records: List[Dict[str, Any]] = []

    for ort_file in ortools_files:
        min_file = ort_file.replace("-ortools-cpsat-stats.json", "-minion-stats.json")
        if not os.path.exists(min_file):
            continue

        rel_path = os.path.relpath(os.path.dirname(ort_file), tests_dir)
        variant = os.path.basename(ort_file).replace("-ortools-cpsat-stats.json", "")

        if args.category and args.category.lower() not in rel_path.lower():
            continue

        try:
            ort_stat = extract_stats(ort_file)
            min_stat = extract_stats(min_file)
        except Exception as e:
            print(f"Warning: error reading {rel_path} ({variant}): {e}", file=sys.stderr)
            continue

        m_trans = min_stat["trans_time_s"]
        o_trans = ort_stat["trans_time_s"]
        m_solve = min_stat["solve_wall_time_s"]
        o_solve = ort_stat["solve_wall_time_s"]

        m_total = m_trans + m_solve
        o_total = o_trans + o_solve

        # Solve speedup: minion / ortools (> 1 means ortools is faster)
        if o_solve > 0 and m_solve > 0:
            solve_speedup = m_solve / o_solve
        elif o_solve == 0 and m_solve == 0:
            solve_speedup = 1.0
        elif o_solve == 0:
            solve_speedup = float("inf")
        else:
            solve_speedup = 0.0

        # Total speedup
        if o_total > 0 and m_total > 0:
            total_speedup = m_total / o_total
        else:
            total_speedup = 1.0

        records.append({
            "category": rel_path.split(os.sep)[0] if os.sep in rel_path else rel_path,
            "test_path": rel_path,
            "variant": variant,
            "minion_trans_s": m_trans,
            "ortools_trans_s": o_trans,
            "minion_solve_s": m_solve,
            "ortools_solve_s": o_solve,
            "minion_total_s": m_total,
            "ortools_total_s": o_total,
            "solve_speedup": solve_speedup,
            "total_speedup": total_speedup,
            "minion_nodes": min_stat["nodes"],
            "ortools_nodes": ort_stat["nodes"],
            "rule_attempts_minion": min_stat["rule_attempts"],
            "rule_attempts_ortools": ort_stat["rule_attempts"],
        })

    if not records:
        print("No matching test pairs found.")
        return 1

    total_pairs = len(records)
    print(f"Found {total_pairs} paired test runs comparing Minion and OR-Tools CP-SAT.\n")

    # Sorting
    if args.sort_by == "max-solve":
        records.sort(key=lambda r: max(r["minion_solve_s"], r["ortools_solve_s"]), reverse=True)
    elif args.sort_by == "ortools-solve":
        records.sort(key=lambda r: r["ortools_solve_s"], reverse=True)
    elif args.sort_by == "minion-solve":
        records.sort(key=lambda r: r["minion_solve_s"], reverse=True)
    elif args.sort_by == "solve-speedup":
        records.sort(key=lambda r: r["solve_speedup"], reverse=True)
    elif args.sort_by == "ortools-trans":
        records.sort(key=lambda r: r["ortools_trans_s"], reverse=True)
    elif args.sort_by == "name":
        records.sort(key=lambda r: (r["test_path"], r["variant"]))

    # Summary calculations
    ortools_faster = [r for r in records if r["ortools_solve_s"] < r["minion_solve_s"]]
    minion_faster = [r for r in records if r["minion_solve_s"] < r["ortools_solve_s"]]
    equal = [r for r in records if r["minion_solve_s"] == r["ortools_solve_s"]]

    total_minion_trans = sum(r["minion_trans_s"] for r in records)
    total_ortools_trans = sum(r["ortools_trans_s"] for r in records)
    total_minion_solve = sum(r["minion_solve_s"] for r in records)
    total_ortools_solve = sum(r["ortools_solve_s"] for r in records)
    total_minion_time = sum(r["minion_total_s"] for r in records)
    total_ortools_time = sum(r["ortools_total_s"] for r in records)

    valid_speedups = [r["solve_speedup"] for r in records if 0 < r["solve_speedup"] < float("inf")]
    geomean_speedup = (
        math.exp(sum(math.log(s) for s in valid_speedups) / len(valid_speedups))
        if valid_speedups
        else 1.0
    )
    median_speedup = sorted(valid_speedups)[len(valid_speedups) // 2] if valid_speedups else 1.0

    print("=" * 80)
    print(" RIEPILOGO GENERALE COMPARAZIONE SOLVER (Minion vs OR-Tools CP-SAT)")
    print("=" * 80)
    print(f"Test totali analizzati con dati appaiati: {total_pairs}")
    print(f"  - OR-Tools più veloce nel risolvere (solve time): {len(ortools_faster)} test")
    print(f"  - Minion più veloce nel risolvere (solve time):   {len(minion_faster)} test")
    print(f"  - Tempi equivalenti:                              {len(equal)} test\n")
    print(f"Tempo totale di Traduzione (AST -> Solver):")
    print(f"  - Minion:   {total_minion_trans:>10.2f} s")
    print(f"  - OR-Tools: {total_ortools_trans:>10.2f} s  (Differenza: {total_ortools_trans - total_minion_trans:+.2f} s)\n")
    print(f"Tempo totale di Risoluzione (Solve Wall Time):")
    print(f"  - Minion:   {total_minion_solve:>10.2f} s")
    print(f"  - OR-Tools: {total_ortools_solve:>10.2f} s\n")
    print(f"Tempo Totale Complessivo (Traduzione + Risoluzione):")
    print(f"  - Minion:   {total_minion_time:>10.2f} s")
    print(f"  - OR-Tools: {total_ortools_time:>10.2f} s\n")
    print(f"Speedup di Risoluzione (Minion / OR-Tools):")
    print(f"  - Media Geometrica: {geomean_speedup:.3f}x")
    print(f"  - Mediana:          {median_speedup:.3f}x")
    print("=" * 80)
    print()

    # Print top N table preview
    top_n = min(args.top_n, len(records))
    print(f"Anteprima dei primi {top_n} test per tempo di risoluzione:")
    header = f"{'Test / Istanza':<42} | {'Minion Tr (s)':<13} | {'ORT Tr (s)':<11} | {'Minion Sol (s)':<14} | {'ORT Sol (s)':<12} | {'Speedup':<9}"
    sep = "-" * len(header)
    print(header)
    print(sep)

    for r in records[:top_n]:
        display_name = f"{r['test_path']} ({r['variant']})"
        if len(display_name) > 42:
            display_name = display_name[:39] + "..."
        sp_str = f"{r['solve_speedup']:.2f}x" if r["solve_speedup"] < 1000 else ">1000x"
        print(
            f"{display_name:<42} | "
            f"{r['minion_trans_s']:<13.4f} | "
            f"{r['ortools_trans_s']:<11.4f} | "
            f"{r['minion_solve_s']:<14.4f} | "
            f"{r['ortools_solve_s']:<12.4f} | "
            f"{sp_str:<9}"
        )
    print(sep)
    print()

    # Write CSV
    output_csv = Path(args.output_csv)
    output_csv.parent.mkdir(parents=True, exist_ok=True)
    with open(output_csv, "w", newline="", encoding="utf-8") as f:
        fieldnames = [
            "category",
            "test_path",
            "variant",
            "minion_trans_s",
            "ortools_trans_s",
            "minion_solve_s",
            "ortools_solve_s",
            "minion_total_s",
            "ortools_total_s",
            "solve_speedup_factor",
            "total_speedup_factor",
            "minion_nodes",
            "ortools_nodes",
            "rule_attempts_minion",
            "rule_attempts_ortools",
        ]
        writer = csv.DictWriter(f, fieldnames=fieldnames)
        writer.writeheader()
        for r in records:
            writer.writerow({
                "category": r["category"],
                "test_path": r["test_path"],
                "variant": r["variant"],
                "minion_trans_s": f"{r['minion_trans_s']:.6f}",
                "ortools_trans_s": f"{r['ortools_trans_s']:.6f}",
                "minion_solve_s": f"{r['minion_solve_s']:.6f}",
                "ortools_solve_s": f"{r['ortools_solve_s']:.6f}",
                "minion_total_s": f"{r['minion_total_s']:.6f}",
                "ortools_total_s": f"{r['ortools_total_s']:.6f}",
                "solve_speedup_factor": f"{r['solve_speedup']:.4f}",
                "total_speedup_factor": f"{r['total_speedup']:.4f}",
                "minion_nodes": r["minion_nodes"],
                "ortools_nodes": r["ortools_nodes"],
                "rule_attempts_minion": r["rule_attempts_minion"],
                "rule_attempts_ortools": r["rule_attempts_ortools"],
            })
    print(f"Report CSV completo scritto con successo in: {output_csv.resolve()}")

    # Write Markdown Report
    output_md = Path(args.output_md)
    output_md.parent.mkdir(parents=True, exist_ok=True)
    with open(output_md, "w", encoding="utf-8") as f:
        f.write("# Confronto Prestazionale: Minion vs Google OR-Tools CP-SAT\n\n")
        f.write("Questo documento mette a confronto i tempi di traduzione (AST rewriting in Conjure Oxide) e i tempi effettivi di risoluzione (solver wall time) registrati sui test di integrazione eseguiti con entrambi i backend.\n\n")
        f.write("## 1. Riepilogo Generale\n\n")
        f.write(f"- **Test totali comparati (coppie corrispondenti):** `{total_pairs}`\n")
        f.write(f"- **Test in cui OR-Tools è più veloce a risolvere:** `{len(ortools_faster)}`\n")
        f.write(f"- **Test in cui Minion è più veloce a risolvere:** `{len(minion_faster)}`\n")
        f.write(f"- **Speedup medio di risoluzione (media geometrica):** `{geomean_speedup:.3f}x`\n")
        f.write(f"- **Speedup mediano di risoluzione:** `{median_speedup:.3f}x`\n\n")
        f.write("| Metrica Complessiva | Minion | OR-Tools CP-SAT | Differenza |\n")
        f.write("| :--- | :--- | :--- | :--- |\n")
        f.write(f"| **Tempo Totale Traduzione** | {total_minion_trans:.2f} s | {total_ortools_trans:.2f} s | {total_ortools_trans - total_minion_trans:+.2f} s |\n")
        f.write(f"| **Tempo Totale Risoluzione** | {total_minion_solve:.2f} s | {total_ortools_solve:.2f} s | {total_ortools_solve - total_minion_solve:+.2f} s |\n")
        f.write(f"| **Tempo Totale Complessivo** | {total_minion_time:.2f} s | {total_ortools_time:.2f} s | {total_ortools_time - total_minion_time:+.2f} s |\n\n")

        f.write("## 2. Test Rilevanti in cui OR-Tools Supera Minion\n\n")
        f.write("Nei problemi combinatori complessi con domini non banali, OR-Tools sfrutta le euristiche SAT e la propagazione di no-goods per superare Minion:\n\n")
        f.write("| Test | Minion Solve (s) | OR-Tools Solve (s) | Speedup Risoluzione |\n")
        f.write("| :--- | :--- | :--- | :--- |\n")
        top_ortools_faster = sorted(ortools_faster, key=lambda r: r["minion_solve_s"] - r["ortools_solve_s"], reverse=True)
        for r in top_ortools_faster[:15]:
            f.write(f"| `{r['test_path']}` ({r['variant']}) | {r['minion_solve_s']:.4f} s | {r['ortools_solve_s']:.4f} s | **{r['solve_speedup']:.2f}x** |\n")

        f.write("\n## 3. Test con Tempi Significativi di Risoluzione\n\n")
        f.write("| Test | Minion Trans (s) | OR-Tools Trans (s) | Minion Solve (s) | OR-Tools Solve (s) | Speedup |\n")
        f.write("| :--- | :--- | :--- | :--- | :--- | :--- |\n")
        for r in records[:50]:
            sp_str = f"{r['solve_speedup']:.2f}x" if r["solve_speedup"] < 1000 else ">1000x"
            f.write(
                f"| `{r['test_path']}` ({r['variant']}) | {r['minion_trans_s']:.4f} | {r['ortools_trans_s']:.4f} | {r['minion_solve_s']:.4f} | {r['ortools_solve_s']:.4f} | {sp_str} |\n"
            )
        f.write("\n")

    print(f"Report Markdown scritto con successo in: {output_md.resolve()}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
