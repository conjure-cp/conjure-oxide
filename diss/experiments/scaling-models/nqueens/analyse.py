#!/usr/bin/env python3
"""
Analyse N-Queens experiment results and produce plots.

Reads results.csv and generates:
  1. N vs average e2e time (Oxide vs Conjure)
  2. N vs solution translation time (log scale)
  3. N vs rewrite time (log scale)
  4. N vs number of solutions
  5. Stacked bar chart: time breakdown per N (selected values)
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
CSV_PATH = SCRIPT_DIR / "results.csv"
PLOTS_DIR = SCRIPT_DIR / "plots"

# -- font setup --------------------------------------------------------------

# Cormorant Garamond Bold for titles, sans-serif for everything else.
# We need the static Bold TTF because matplotlib ignores variable font
# weight axes. Generate it with fonttools if only the variable font exists.

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
            font, {"wght": 700}, inplace=True,
            overlap=instancer.OverlapMode.KEEP_AND_SET_FLAGS,
        )
        font.save(str(_bold_ttf))
    except Exception as exc:
        print(f"Warning: could not generate bold font: {exc}", file=sys.stderr)

if _bold_ttf.exists():
    fm.fontManager.addfont(str(_bold_ttf))
    _TITLE_FP = fm.FontProperties(fname=str(_bold_ttf), size=14)
else:
    print("Warning: Cormorant Garamond Bold not found, titles will use sans-serif",
          file=sys.stderr)

plt.rcParams.update({
    "font.family": "sans-serif",
    "font.size": 10,
    "axes.labelsize": 10,
    "xtick.labelsize": 9,
    "ytick.labelsize": 9,
})

_LEGEND_PROPS = {"family": "sans-serif", "size": 8}


def _set_title(ax, text: str) -> None:
    if _TITLE_FP is not None:
        ax.set_title(text, fontproperties=_TITLE_FP)
    else:
        ax.set_title(text, fontweight="bold", fontsize=14)

# -- colour scheme -----------------------------------------------------------
# Matching the LaTeX pastel palette.

G_CONJURE   = (0.75,  1.0,   1.0)    # cyan!25
G_SAVILEROW = (0.9375, 1.0,  0.75)   # lime!25
G_SOLVER    = (0.89,  0.78,  1.0)    # violet!22
G_OXIDE     = (1.0,   0.975, 0.79)   # Goldenrod!25
G_OXIDE_1   = (1.0,   0.925, 0.895)  # Peach!15
G_OXIDE_2   = (0.958, 0.862, 0.856)  # BrickRed!15

# Darker versions for line plots.
G_CONJURE_LINE = (0.0, 0.7, 0.7)
G_OXIDE_LINE   = (0.8, 0.65, 0.0)

# Seaborn palette and marker dicts keyed by display name.
_PALETTE = {"Oxide": G_OXIDE_LINE, "Conjure": G_CONJURE_LINE}
_MARKERS = {"Oxide": "o", "Conjure": "s"}

# -- helpers -----------------------------------------------------------------

def load_data() -> pd.DataFrame:
    df = pd.read_csv(CSV_PATH)
    # For oxide rows, e2e = sum of components.
    # For conjure rows, e2e = the conjure_solve_e2e_ms column.
    oxide_mask = df["system"] == "oxide"
    df.loc[oxide_mask, "e2e_ms"] = (
        df.loc[oxide_mask, "rewrite_time_ms"]
        + df.loc[oxide_mask, "solver_time_ms"]
        + df.loc[oxide_mask, "solution_translation_time_ms"]
    )
    conjure_mask = df["system"] == "conjure"
    df.loc[conjure_mask, "e2e_ms"] = df.loc[conjure_mask, "conjure_solve_e2e_ms"]

    # Derived columns for plotting.
    df["System"] = df["system"].str.capitalize()
    df["e2e_s"] = df["e2e_ms"] / 1000.0
    return df


def savefig(fig, name: str) -> None:
    PLOTS_DIR.mkdir(exist_ok=True)
    path = PLOTS_DIR / name
    fig.savefig(path, dpi=200, bbox_inches="tight")
    print(f"Saved {path}")


# -- plot 1: N vs average e2e time ------------------------------------------

def plot_e2e(df: pd.DataFrame) -> None:
    fig, ax = plt.subplots(figsize=(7, 4.5))
    sns.lineplot(
        data=df, x="N", y="e2e_s",
        hue="System", style="System",
        markers=_MARKERS, dashes=False,
        palette=_PALETTE, linewidth=2, markersize=6,
        errorbar="sd", ax=ax,
    )
    ax.set_xlabel("Board size N")
    ax.set_ylabel("Average e2e time (s)")
    _set_title(ax, "N-Queens: End-to-End Time")
    ax.legend(prop=_LEGEND_PROPS)
    ax.grid(True, alpha=0.3)
    sns.despine(ax=ax)
    savefig(fig, "e2e_time.png")
    plt.close(fig)


# -- plot 2: N vs solution translation time (log scale) ---------------------

def plot_translation_time(df: pd.DataFrame) -> None:
    fig, ax = plt.subplots(figsize=(7, 4.5))
    sns.lineplot(
        data=df, x="N", y="solution_translation_time_ms",
        hue="System", style="System",
        markers=_MARKERS, dashes=False,
        palette=_PALETTE, linewidth=2, markersize=6,
        errorbar="sd", ax=ax,
    )
    ax.set_yscale("log")
    ax.set_xlabel("Board size N")
    ax.set_ylabel("Solution translation time (ms, log)")
    _set_title(ax, "N-Queens: Solution Translation Time")
    ax.legend(prop=_LEGEND_PROPS)
    ax.grid(True, alpha=0.3, which="both")
    sns.despine(ax=ax)
    savefig(fig, "translation_time_log.png")
    plt.close(fig)


# -- plot 3: N vs rewrite time (log scale) ----------------------------------

def plot_rewrite_time(df: pd.DataFrame) -> None:
    fig, ax = plt.subplots(figsize=(7, 4.5))
    sns.lineplot(
        data=df, x="N", y="rewrite_time_ms",
        hue="System", style="System",
        markers=_MARKERS, dashes=False,
        palette=_PALETTE, linewidth=2, markersize=6,
        errorbar="sd", ax=ax,
    )
    ax.set_yscale("log")
    ax.set_xlabel("Board size N")
    ax.set_ylabel("Rewrite time (ms, log)")
    _set_title(ax, "N-Queens: Rewrite Time")
    ax.legend(prop=_LEGEND_PROPS)
    ax.grid(True, alpha=0.3, which="both")
    sns.despine(ax=ax)
    savefig(fig, "rewrite_time_log.png")
    plt.close(fig)


# -- plot 4: N vs number of solutions --------------------------------------

def plot_num_solutions(df: pd.DataFrame) -> None:
    oxide = df[df["System"] == "Oxide"]
    fig, ax = plt.subplots(figsize=(7, 4.5))
    sns.barplot(
        data=oxide, x="N", y="num_solutions",
        color=G_SOLVER, edgecolor="grey", linewidth=0.5,
        errorbar=None, ax=ax,
    )
    ax.set_yscale("log")
    ax.set_xlabel("Board size N")
    ax.set_ylabel("Number of solutions (log)")
    _set_title(ax, "N-Queens: Problem Scale")
    ax.grid(True, axis="y", alpha=0.3, which="both")
    sns.despine(ax=ax)
    savefig(fig, "num_solutions.png")
    plt.close(fig)


# -- plot 5: time breakdown --------------------------------------------------

_PHASE_COLS = {
    "rewrite_time_ms": "Rewrite",
    "solver_time_ms": "Solve",
    "solution_translation_time_ms": "Translate",
}
_PHASE_PALETTE = {
    "Rewrite":   G_SOLVER,   # light violet
    "Solve":     G_SAVILEROW,   # green
    "Translate": G_CONJURE,   # blue
}


def plot_breakdown(df: pd.DataFrame) -> None:
    """Dodged bar chart: phase breakdown per N, faceted by System, log scale."""
    melted = df.melt(
        id_vars=["System", "N"],
        value_vars=list(_PHASE_COLS.keys()),
        var_name="Phase", value_name="time_ms",
    )
    melted["Phase"] = melted["Phase"].map(_PHASE_COLS)
    melted["time_s"] = melted["time_ms"] / 1000.0

    g = sns.catplot(
        data=melted, x="N", y="time_s",
        hue="Phase", col="System",
        kind="bar", errorbar="sd",
        hue_order=["Solve", "Rewrite", "Translate"],
        palette=_PHASE_PALETTE,
        edgecolor="grey", linewidth=0.5,
        height=5, aspect=1.3,
        legend="auto",
        facet_kws={"gridspec_kws": {"wspace": 0.1}},
    )
    g.set(yscale="log")
    g.set_axis_labels("Board size N", "Time (s, log scale)")
    g.figure.suptitle(
        "N-Queens: Phase Breakdown",
        fontproperties=_TITLE_FP,
        y=1.02,
    )
    sns.move_legend(g, "upper left", bbox_to_anchor=(0.08, 1.05),
                    prop=_LEGEND_PROPS, title=None)
    savefig(g.figure, "breakdown.png")
    plt.close(g.figure)


# -- main --------------------------------------------------------------------

def main() -> None:
    if not CSV_PATH.exists():
        print(f"Error: {CSV_PATH} not found. Run the experiment first.",
              file=sys.stderr)
        sys.exit(1)

    df = load_data()

    plot_e2e(df)
    plot_translation_time(df)
    plot_rewrite_time(df)
    plot_num_solutions(df)
    plot_breakdown(df)

    print("Done.")


if __name__ == "__main__":
    main()


