#!/usr/bin/env python3
"""
Analyse Pythagorean Triples no-solver results and produce plots.

Reads results-no-solver.csv and generates:
  1. N vs rewrite time (Oxide vs Conjure)
  2. N vs rewrite time, log scale (Oxide vs Conjure)

"Rewrite time" here means the full model translation pipeline
without running the solver:
  - Oxide: conjure-oxide rewrite pass
  - Conjure: conjure modelling (Essence -> Essence') + savilerow (Essence' -> Minion)
"""

import sys
from pathlib import Path

import matplotlib
import matplotlib.pyplot as plt
import matplotlib.font_manager as fm
import numpy as np
import pandas as pd
import seaborn as sns

# -- paths -------------------------------------------------------------------

SCRIPT_DIR = Path(__file__).resolve().parent
CSV_PATH = SCRIPT_DIR / "results-no-solver.csv"
PLOTS_DIR = SCRIPT_DIR / "plots"

# -- font setup --------------------------------------------------------------

# Cormorant Garamond Bold for titles, sans-serif for everything else.

_TITLE_FP = None
_font_dir = Path.home() / ".local" / "share" / "fonts"
_bold_ttf = _font_dir / "CormorantGaramond-Bold.ttf"
_var_ttf = _font_dir / "CormorantGaramond-VariableFont_wght.ttf"

if not _bold_ttf.exists() and _var_ttf.exists():
    try:
        from fontTools.ttLib import TTFont
        from fontTools.varLib import instancer

        font = TTFont(str(_var_ttf))
        instancer.instantiateVariableFont(
            font,
            {"wght": 700},
            inplace=True,
            overlap=instancer.OverlapMode.KEEP_AND_SET_FLAGS,
        )
        font.save(str(_bold_ttf))
    except Exception as exc:
        print(f"Warning: could not generate bold font: {exc}", file=sys.stderr)

if _bold_ttf.exists():
    fm.fontManager.addfont(str(_bold_ttf))
    _TITLE_FP = fm.FontProperties(fname=str(_bold_ttf), size=14)
else:
    print(
        "Warning: Cormorant Garamond Bold not found, titles will use sans-serif",
        file=sys.stderr,
    )

plt.rcParams.update(
    {
        "font.family": "sans-serif",
        "font.size": 10,
        "axes.labelsize": 10,
        "xtick.labelsize": 9,
        "ytick.labelsize": 9,
    }
)

_LEGEND_PROPS = {"family": "sans-serif", "size": 8}


def _set_title(ax, text):
    if _TITLE_FP is not None:
        ax.set_title(text, fontproperties=_TITLE_FP)
    else:
        ax.set_title(text, fontweight="bold", fontsize=14)


# -- colour scheme -----------------------------------------------------------

G_CONJURE_LINE = (0.0, 0.7, 0.7)
G_OXIDE_LINE = (0.8, 0.65, 0.0)

_PALETTE = {"Oxide": G_OXIDE_LINE, "Conjure": G_CONJURE_LINE}
_MARKERS = {"Oxide": "o", "Conjure": "s"}

# -- helpers -----------------------------------------------------------------


def load_data():
    df = pd.read_csv(CSV_PATH)

    # Drop rows where rewrite_time_ms is -1 (failed runs).
    df = df[df["rewrite_time_ms"] >= 0].copy()

    df["System"] = df["system"].str.capitalize()
    df["rewrite_s"] = df["rewrite_time_ms"] / 1000.0
    return df


def savefig(fig, name):
    PLOTS_DIR.mkdir(exist_ok=True)
    path = PLOTS_DIR / name
    fig.savefig(path, dpi=200, bbox_inches="tight")
    print(f"Saved {path}")


# -- plot 1: rewrite time (linear scale) ------------------------------------


def plot_rewrite_time(df):
    fig, ax = plt.subplots(figsize=(7, 4.5))
    sns.lineplot(
        data=df,
        x="N",
        y="rewrite_s",
        hue="System",
        style="System",
        markers=_MARKERS,
        dashes=False,
        palette=_PALETTE,
        linewidth=2,
        markersize=6,
        errorbar="sd",
        ax=ax,
    )
    ax.set_xlabel("Partition size N")
    ax.set_ylabel("Rewrite time (s)")
    _set_title(ax, "Pythagorean Triples (No Solver): Rewrite Time")
    ax.legend(prop=_LEGEND_PROPS)
    ax.grid(True, alpha=0.3)
    sns.despine(ax=ax)
    savefig(fig, "no_solver_rewrite_time.png")
    plt.close(fig)


# -- plot 2: rewrite time (log scale) ---------------------------------------


def plot_rewrite_time_log(df):
    fig, ax = plt.subplots(figsize=(7, 4.5))
    sns.lineplot(
        data=df,
        x="N",
        y="rewrite_time_ms",
        hue="System",
        style="System",
        markers=_MARKERS,
        dashes=False,
        palette=_PALETTE,
        linewidth=2,
        markersize=6,
        errorbar="sd",
        ax=ax,
    )
    ax.set_yscale("log")
    ax.set_xlabel("Partition size N")
    ax.set_ylabel("Rewrite time (ms, log)")
    _set_title(ax, "Pythagorean Triples (No Solver): Rewrite Time")
    ax.legend(prop=_LEGEND_PROPS)
    ax.grid(True, alpha=0.3, which="both")
    sns.despine(ax=ax)
    savefig(fig, "no_solver_rewrite_time_log.png")
    plt.close(fig)


# -- main --------------------------------------------------------------------


def main():
    if not CSV_PATH.exists():
        print(
            f"Error: {CSV_PATH} not found. Run the experiment first.", file=sys.stderr
        )
        sys.exit(1)

    df = load_data()

    if df.empty:
        print("Error: no valid data rows in CSV.", file=sys.stderr)
        sys.exit(1)

    plot_rewrite_time(df)
    plot_rewrite_time_log(df)

    print("Done.")


if __name__ == "__main__":
    main()

